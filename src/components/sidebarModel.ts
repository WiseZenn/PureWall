import type {
  CollectionEntry,
  FilterKey,
  Stats,
} from "../stores/wallpapers";

export interface SidebarFilterDefinition {
  key: FilterKey | "tags";
  label: string;
  icon: string;
  count: (stats: Stats, tagCount: number) => number;
}

export const sidebarFilters = [
  {
    key: "all",
    label: "Library",
    icon: "library",
    count: (stats: Stats) => stats.total,
  },
  {
    key: "liked",
    label: "Liked",
    icon: "heart",
    count: (stats: Stats) => stats.liked,
  },
  {
    key: "disliked",
    label: "Disliked",
    icon: "dislike",
    count: (stats: Stats) => stats.disliked,
  },
  {
    key: "blacklisted",
    label: "Hidden",
    icon: "hidden",
    count: (stats: Stats) => stats.blacklisted,
  },
  {
    key: "tags",
    label: "Tags",
    icon: "tag",
    count: (_stats: Stats, tagCount: number) => tagCount,
  },
] satisfies readonly SidebarFilterDefinition[];

export interface CollectionCreator {
  createCollection: (
    name: string,
    color: string,
  ) => Promise<CollectionEntry | null>;
}

export async function createCollectionFromDraft(
  creator: CollectionCreator,
  draft: string,
  color: string,
): Promise<boolean> {
  const name = draft.trim();
  if (!name) return false;

  return (await creator.createCollection(name, color)) !== null;
}
