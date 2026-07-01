/**
 * Favorite-issue toggle button — Phase 18B Item 26.
 *
 * Renders a `Star` icon (lucide) that switches between filled/outline based
 * on whether the given issue is currently favorited. Clicking toggles the
 * state via `add_favorite` / `remove_favorite` and emits a
 * `favorites-changed` event the rest of the UI can listen to.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Star } from "lucide-react";

import {
  addFavorite,
  isFavorite as isFavoriteCmd,
  removeFavorite,
} from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";
import { useT } from "../../i18n";

export interface FavoriteStarProps {
  issueKey: string;
  /**
   * Connection that owns this issue. Favorites are keyed by
   * `(connectionId, issueKey)` so the same key in two tenants toggles
   * independently. `null`/`undefined` means "connection unknown" (legacy).
   */
  connectionId?: number | null;
  /**
   * Source-of-truth override. When provided, the component renders in
   * "controlled" mode: it trusts the parent and does NOT issue its own
   * `isFavorite` IPC call. The parent is expected to keep this value in
   * sync (e.g. by reading from a `favorites` cache that gets invalidated
   * on toggle — the StartTrackingBar dropdown does exactly that).
   *
   * When `undefined`, the component falls back to "uncontrolled" mode and
   * runs a per-issue query against the backend.
   */
  initial?: boolean;
  /** Optional size override (in px). Defaults to 14. */
  size?: number;
  /** Optional CSS class. */
  className?: string;
}

export function FavoriteStar({
  issueKey,
  connectionId,
  initial,
  size = 14,
  className,
}: FavoriteStarProps) {
  const t = useT();
  const queryClient = useQueryClient();
  // Controlled iff `initial` was explicitly passed. We rely on
  // `initial === undefined` as the discriminator rather than a separate
  // `controlled` boolean prop — the existing API already uses the
  // presence/absence of `initial` to signal "parent owns the state".
  // The query stays mounted with `enabled: false` so any future
  // imperative `queryClient.setQueryData(["favorite", key], v)` from
  // somewhere else in the tree still updates this component's view.
  const isControlled = initial !== undefined;
  const q = useQuery({
    queryKey: queryKeys.favorites.one(issueKey, connectionId),
    queryFn: () => isFavoriteCmd(issueKey, connectionId),
    staleTime: 60_000,
    enabled: !isControlled && issueKey.length > 0,
  });
  const isFav = isControlled ? initial : (q.data ?? false);

  const toggle = async (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    try {
      if (isFav) {
        await removeFavorite(issueKey, connectionId);
      } else {
        await addFavorite(issueKey, connectionId);
      }
    } finally {
      queryClient.invalidateQueries({
        queryKey: queryKeys.favorites.one(issueKey, connectionId),
      });
      queryClient.invalidateQueries({ queryKey: queryKeys.favorites.all() });
    }
  };

  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={isFav}
      aria-label={isFav ? t("misc.favorite.remove") : t("misc.favorite.add")}
      title={isFav ? t("misc.favorite.remove") : t("misc.favorite.add")}
      className={
        className ??
        "inline-flex items-center justify-center text-[var(--text-tertiary)] hover:text-[var(--accent)] transition-colors duration-150"
      }
      style={{ width: size + 8, height: size + 8 }}
    >
      <Star
        width={size}
        height={size}
        aria-hidden
        fill={isFav ? "var(--accent)" : "none"}
        stroke={isFav ? "var(--accent)" : "currentColor"}
        strokeWidth={1.75}
      />
    </button>
  );
}
