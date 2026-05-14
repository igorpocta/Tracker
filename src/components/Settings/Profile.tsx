/**
 * Settings → Profile.
 *
 * Reference: `screens/SCR-20260514-rjgv-2.png`.
 *
 * The avatar + name + email row + Log out button. We also show the
 * subscription preview below — the underlying data isn't wired to a real
 * billing backend, but the visual surface is part of the Trcker identity.
 */
import { Check, LogOut, Pencil } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import { getCurrentConfig, signOut } from "../../api/commands";
import type { JiraConfig } from "../../api/types";

export default function Profile() {
  const navigate = useNavigate();
  const [config, setConfig] = useState<JiraConfig | null>(null);

  useEffect(() => {
    let cancelled = false;
    getCurrentConfig()
      .then((c) => {
        if (!cancelled) setConfig(c);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const handleSignOut = async () => {
    try {
      await signOut();
      navigate("/setup", { replace: true });
    } catch {
      /* swallow — error path keeps the user on the panel */
    }
  };

  const initials =
    (config?.email?.[0] ?? "T").toString().toUpperCase().slice(0, 1);
  const name = displayNameFromEmail(config?.email ?? "");

  return (
    <div className="flex flex-col gap-8 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Profile
        </h2>
      </header>

      <section className="flex items-center gap-3">
        <div
          className="w-12 h-12 rounded-full flex items-center justify-center
                     text-base font-semibold"
          style={{
            background: "var(--bg-active)",
            color: "var(--text-primary)",
          }}
        >
          {initials}
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-semibold text-[var(--text-primary)]">
            {name}
          </div>
          <div className="text-xs text-[var(--text-tertiary)] truncate">
            {config?.email ?? "Not connected"}
          </div>
        </div>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 px-3 h-8 rounded-[var(--radius-md)]
                     border border-[var(--border-subtle)] text-xs
                     text-[var(--text-secondary)]
                     hover:bg-[var(--bg-hover)] transition-colors duration-150"
        >
          <Pencil className="w-3.5 h-3.5" aria-hidden />
          Edit
        </button>
      </section>

      <section className="border-t border-[var(--border-subtle)] pt-4">
        <button
          type="button"
          onClick={handleSignOut}
          className="inline-flex items-center gap-1.5 px-3 h-8 rounded-[var(--radius-md)]
                     border border-[var(--border-subtle)] text-xs
                     text-[var(--text-secondary)]
                     hover:bg-[var(--bg-hover)] transition-colors duration-150"
        >
          <LogOut className="w-3.5 h-3.5" aria-hidden />
          Log out
        </button>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          Signing out clears all local data. Any running timer will be saved first.
        </p>
      </section>

      <section className="border-t border-[var(--border-subtle)] pt-6">
        <h3 className="text-base font-semibold text-[var(--text-primary)] mb-4">
          Subscription
        </h3>
        <div className="grid grid-cols-2 gap-3">
          <SubscriptionTile
            label="Poor"
            features={["Unlimited tracking", "Goals", "Up to 20 indexed tickets"]}
            tone="muted"
          />
          <SubscriptionTile
            label="Pro"
            features={["Premium themes", "More simultaneous connections", "Unlimited indexed tickets"]}
            tone="muted"
          />
        </div>
        <div className="mt-3">
          <SubscriptionTile
            label="Team"
            badge="Current"
            features={["Theme customized to your company", "Unlimited indexed tickets", "Priority support"]}
            tone="active"
          />
        </div>
      </section>
    </div>
  );
}

function SubscriptionTile({
  label,
  features,
  tone,
  badge,
}: {
  label: string;
  features: string[];
  tone: "muted" | "active";
  badge?: string;
}) {
  return (
    <div
      className="rounded-[var(--radius-md)] p-4 border"
      style={{
        background: tone === "active" ? "var(--accent-soft)" : "var(--bg-surface)",
        borderColor:
          tone === "active" ? "var(--accent)" : "var(--border-subtle)",
      }}
    >
      <div className="flex items-center gap-2 mb-2">
        <span
          className="text-sm font-semibold"
          style={{
            color: tone === "active" ? "var(--accent)" : "var(--text-primary)",
          }}
        >
          {label}
        </span>
        {badge && (
          <span
            className="text-[10px] uppercase tracking-[0.1em] px-1.5 py-0.5 rounded-full"
            style={{
              color: "var(--accent)",
              border: "1px solid var(--accent)",
            }}
          >
            {badge}
          </span>
        )}
      </div>
      <ul className="flex flex-col gap-1">
        {features.map((f) => (
          <li key={f} className="text-[11px] text-[var(--text-secondary)] flex items-start gap-1.5">
            <Check
              className="w-3 h-3 mt-0.5 shrink-0"
              style={{
                color: tone === "active" ? "var(--accent)" : "var(--text-tertiary)",
              }}
              aria-hidden
            />
            {f}
          </li>
        ))}
      </ul>
    </div>
  );
}

function displayNameFromEmail(email: string): string {
  if (!email) return "Not signed in";
  const [local] = email.split("@");
  if (!local) return email;
  return local
    .split(/[._-]/)
    .filter(Boolean)
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}
