import type { DisplayMode } from "../stores/wallpapers";

export interface DisplayModeOption {
  key: DisplayMode;
  label: string;
  shortLabel: string;
}

export const displayModeOptions: DisplayModeOption[] = [
  {
    key: "all",
    label: "Same",
    shortLabel: "Same",
  },
  {
    key: "span",
    label: "Span",
    shortLabel: "Span",
  },
  {
    key: "independent",
    label: "Separate",
    shortLabel: "Separate",
  },
];

export function getDisplayModeOption(mode: DisplayMode): DisplayModeOption {
  return displayModeOptions.find((option) => option.key === mode) ?? displayModeOptions[0];
}
