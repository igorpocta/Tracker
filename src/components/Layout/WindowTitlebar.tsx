/**
 * Custom title bar for Windows.
 *
 * On Windows the native OS title bar clashes with Tracker's dark chrome, so the
 * backend disables window decorations (`set_decorations(false)`) and we render
 * this slim bar instead: a full-width drag region with minimize / maximize /
 * close controls on the right. macOS keeps its native `Overlay` title bar (with
 * the traffic-light buttons) and uses `DragStrip` instead — this component is
 * only mounted on Windows.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

export function WindowTitlebar() {
  const [maximized, setMaximized] = useState(false);

  // Keep the maximize/restore affordance in sync with the actual window state.
  useEffect(() => {
    const w = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    w.isMaximized()
      .then((m) => !cancelled && setMaximized(m))
      .catch(() => {});
    w.onResized(() => {
      w.isMaximized()
        .then((m) => !cancelled && setMaximized(m))
        .catch(() => {});
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const onMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }, []);

  const minimize = () => void getCurrentWindow().minimize().catch(() => {});
  const toggleMax = () => void getCurrentWindow().toggleMaximize().catch(() => {});
  const close = () => void getCurrentWindow().close().catch(() => {});

  return (
    <div
      aria-label="Titulní lišta okna"
      onMouseDown={onMouseDown}
      data-tauri-drag-region
      className="fixed top-0 left-0 right-0 h-8 z-[9999] flex items-center
                 justify-between select-none border-b border-[var(--border-subtle)]"
      style={
        {
          background: "var(--bg-app)",
          WebkitAppRegion: "drag",
          appRegion: "drag",
        } as React.CSSProperties
      }
    >
      <span className="pl-3 text-[11px] font-medium text-[var(--text-tertiary)] pointer-events-none">
        Tracker
      </span>
      <div
        className="flex items-stretch h-full"
        style={
          { WebkitAppRegion: "no-drag", appRegion: "no-drag" } as React.CSSProperties
        }
      >
        <TitlebarButton label="Minimalizovat" onClick={minimize}>
          <Minus className="w-3.5 h-3.5" aria-hidden />
        </TitlebarButton>
        <TitlebarButton
          label={maximized ? "Obnovit" : "Maximalizovat"}
          onClick={toggleMax}
        >
          <Square className="w-3 h-3" aria-hidden />
        </TitlebarButton>
        <TitlebarButton label="Zavřít" danger onClick={close}>
          <X className="w-4 h-4" aria-hidden />
        </TitlebarButton>
      </div>
    </div>
  );
}

function TitlebarButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={
        "w-11 h-full flex items-center justify-center text-[var(--text-secondary)] " +
        "transition-colors duration-150 " +
        (danger
          ? "hover:bg-[var(--danger)] hover:text-white"
          : "hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]")
      }
    >
      {children}
    </button>
  );
}
