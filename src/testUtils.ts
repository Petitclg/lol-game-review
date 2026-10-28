import type { Clip, Theme } from "./types";

export function makeClip(id: string, theme: Theme, gameTimeSecs: number, extra: Partial<Clip> = {}): Clip {
  return {
    id,
    theme,
    label: theme,
    gameTimeSecs,
    startSecs: Math.max(0, gameTimeSecs - 10),
    endSecs: gameTimeSecs + 5,
    file: `C:\\clips\\${id}.mp4`,
    players: [],
    ...extra,
  };
}

export const SAMPLE_CLIPS: Clip[] = [
  makeClip("c", "objective", 900),
  makeClip("a", "kill", 300),
  makeClip("d", "death", 600),
  makeClip("b", "kill", 450),
  makeClip("e", "turret", 1200),
  makeClip("f", "assist", 300),
];
