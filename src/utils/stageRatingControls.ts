export type StageRatingIntent = "like" | "dislike";
export type StageRatingCommand = StageRatingIntent | "reset";

export interface StageRatingControl {
  command: StageRatingCommand;
  label: string;
  pressed: boolean;
}

export interface StageRatingTarget {
  path: string;
  rating: number;
}

export type StageRatingActions = Record<
  StageRatingCommand,
  (path: string) => Promise<unknown>
>;

export async function applyStageRating(
  target: StageRatingTarget,
  intent: StageRatingIntent,
  actions: StageRatingActions,
): Promise<StageRatingCommand> {
  const command = stageRatingControl(target.rating, intent).command;
  await actions[command](target.path);
  return command;
}

export interface StageActionGate {
  value: boolean;
}

export async function runExclusiveStageAction(
  gate: StageActionGate,
  action: () => Promise<void>,
): Promise<boolean> {
  if (gate.value) return false;
  gate.value = true;
  try {
    await action();
    return true;
  } finally {
    gate.value = false;
  }
}

export function stageRatingControl(
  rating: number,
  intent: StageRatingIntent,
): StageRatingControl {
  const pressed = intent === "like" ? rating === 1 : rating === -1;
  if (pressed) {
    return {
      command: "reset",
      label:
        intent === "like"
          ? "Unlike displayed wallpaper"
          : "Clear dislike from displayed wallpaper",
      pressed: true,
    };
  }

  return {
    command: intent,
    label:
      intent === "like"
        ? "Like displayed wallpaper"
        : "Dislike displayed wallpaper",
    pressed: false,
  };
}
