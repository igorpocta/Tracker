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

export interface FavoriteStarProps {
  issueKey: string;
  /** Optional override — when the parent already knows the state. */
  initial?: boolean;
  /** Optional size override (in px). Defaults to 14. */
  size?: number;
  /** Optional CSS class. */
  className?: string;
}

export function FavoriteStar({
  issueKey,
  initial,
  size = 14,
  className,
}: FavoriteStarProps) {
  const queryClient = useQueryClient();
  const q = useQuery({
    queryKey: ["favorite", issueKey],
    queryFn: () => isFavoriteCmd(issueKey),
    initialData: initial,
    staleTime: 60_000,
    enabled: issueKey.length > 0,
  });
  const isFav = q.data ?? false;

  const toggle = async (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    try {
      if (isFav) {
        await removeFavorite(issueKey);
      } else {
        await addFavorite(issueKey);
      }
    } finally {
      queryClient.invalidateQueries({ queryKey: ["favorite", issueKey] });
      queryClient.invalidateQueries({ queryKey: ["favorites"] });
    }
  };

  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={isFav}
      aria-label={isFav ? "Odebrat z oblíbených" : "Přidat do oblíbených"}
      title={isFav ? "Odebrat z oblíbených" : "Přidat do oblíbených"}
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
