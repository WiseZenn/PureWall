import type { WallpaperEntry } from "../stores/wallpapers";

function dimensionsLabel(wallpaper: WallpaperEntry): string {
  if (wallpaper.width > 0 && wallpaper.height > 0) {
    return `${wallpaper.width}x${wallpaper.height}`;
  }
  return "";
}

function ratingLabel(rating: number): string {
  if (rating === 1) return "Liked";
  if (rating === -1) return "Disliked";
  return "Unrated";
}

export function wallpaperAccessibleLabel(wallpaper: WallpaperEntry | null | undefined): string {
  if (!wallpaper) return "Wallpaper preview";

  const displayTitle = wallpaper.display_title.trim();
  const tagName = wallpaper.tags[0]?.name;
  const name =
    displayTitle ||
    (tagName ? `${tagName} wallpaper` : "Wallpaper");
  const details = [ratingLabel(wallpaper.rating), dimensionsLabel(wallpaper)].filter(Boolean);

  return details.length > 0 ? `${name}, ${details.join(", ")}` : name;
}
