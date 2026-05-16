/**
 * Shared wrapper for top-level route pages.
 *
 * Every route in `src/routes/` was carrying the same
 * `px-6 pb-6 pt-2 flex flex-col gap-* w-full max-w-[*] mx-auto`
 * class string. Different routes pick a different `max-w` (Audit
 * is narrower, JiraDashboard is wider) and a different vertical
 * `gap`, so those are exposed as props with the most-common values
 * as defaults — Calendar / Reports / TimeLog can use the bare
 * `<PageContainer>` and get the same shell back.
 */
import type { ReactNode } from "react";

export interface PageContainerProps {
  children: ReactNode;
  /**
   * Tailwind max-width class. Defaults to `max-w-[1100px]` which
   * matches Calendar, Goals, Reports and TimeLog. Audit overrides
   * to `max-w-[920px]`; JiraDashboard to `max-w-[1400px]`.
   */
  maxWidth?: string;
  /**
   * Vertical gap between direct children. Defaults to `gap-5`.
   * Audit + Goals override to `gap-4`.
   */
  gap?: string;
  /** Extra classes appended to the wrapper. */
  className?: string;
}

export function PageContainer({
  children,
  maxWidth = "max-w-[1100px]",
  gap = "gap-5",
  className,
}: PageContainerProps) {
  return (
    <div
      className={`px-6 pb-6 pt-2 flex flex-col w-full ${maxWidth} ${gap} mx-auto${
        className ? ` ${className}` : ""
      }`}
    >
      {children}
    </div>
  );
}
