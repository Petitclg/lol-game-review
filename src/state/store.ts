import { filterClips, type ThemeFilter } from "../clips";
import type { Account, Clip, GameEntry, Status } from "../types";

export interface Progress {
  sessionId: string;
  done: number;
  total: number;
}

export interface State {
  accountLoaded: boolean;
  account: Account | null;
  /** Message d'erreur si le client LoL est injoignable. */
  accountError: string | null;
  status: Status | null;
  games: GameEntry[];
  gamesLoading: boolean;
  gamesError: string | null;
  selectedGameId: number | null;
  clips: Clip[];
  clipsLoading: boolean;
  clipsError: string | null;
  /** Génération en cours (sessionId + progression). */
  generating: Progress | null;
  generateError: string | null;
  theme: ThemeFilter;
  selectedClipId: string | null;
  deletedSessions: string[];
  notice: string | null;
}

export type Action =
  | { type: "accountLoaded"; account: Account | null }
  | { type: "accountFailed"; error: string }
  | { type: "statusChanged"; status: Status }
  | { type: "gamesLoading" }
  | { type: "gamesLoaded"; games: GameEntry[] }
  | { type: "gamesFailed"; error: string }
  | { type: "selectGame"; gameId: number | null }
  | { type: "clipsLoading" }
  | { type: "clipsLoaded"; sessionId: string; clips: Clip[] }
  | { type: "clipsFailed"; error: string }
  | { type: "generateStarted"; sessionId: string }
  | { type: "generateProgress"; progress: Progress }
  | { type: "generateDone"; sessionId: string; clips: Clip[] }
  | { type: "generateFailed"; error: string }
  | { type: "setTheme"; theme: ThemeFilter }
  | { type: "selectClip"; clipId: string }
  | { type: "stepClip"; delta: number }
  | { type: "recordingDeleted"; sessionId: string }
  | { type: "notice"; message: string | null };

export function initialState(): State {
  return {
    accountLoaded: false,
    account: null,
    accountError: null,
    status: null,
    games: [],
    gamesLoading: false,
    gamesError: null,
    selectedGameId: null,
    clips: [],
    clipsLoading: false,
    clipsError: null,
    generating: null,
    generateError: null,
    theme: "all",
    selectedClipId: null,
    deletedSessions: [],
    notice: null,
  };
}

export function selectedGame(state: State): GameEntry | null {
  return state.games.find((g) => g.gameId === state.selectedGameId) ?? null;
}

export function visibleClips(state: State): Clip[] {
  return filterClips(state.clips, state.theme);
}

/** Clip courant : la sélection si elle est visible, sinon le premier clip visible. */
export function currentClip(state: State): Clip | null {
  const visible = visibleClips(state);
  return visible.find((c) => c.id === state.selectedClipId) ?? visible[0] ?? null;
}

function resolveSelection(clips: Clip[], theme: ThemeFilter, wanted: string | null): string | null {
  const visible = filterClips(clips, theme);
  return (visible.find((c) => c.id === wanted) ?? visible[0])?.id ?? null;
}

function withClips(state: State, clips: Clip[]): State {
  return {
    ...state,
    clips,
    selectedClipId: resolveSelection(clips, state.theme, state.selectedClipId),
  };
}

export function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "accountLoaded":
      return { ...state, accountLoaded: true, account: action.account, accountError: null };
    case "accountFailed":
      return { ...state, accountLoaded: true, account: null, accountError: action.error };
    case "statusChanged":
      return { ...state, status: action.status };
    case "gamesLoading":
      return { ...state, gamesLoading: true, gamesError: null };
    case "gamesLoaded": {
      const stillThere = action.games.some((g) => g.gameId === state.selectedGameId);
      return {
        ...state,
        games: action.games,
        gamesLoading: false,
        gamesError: null,
        ...(stillThere
          ? {}
          : { selectedGameId: null, clips: [], selectedClipId: null, clipsError: null }),
      };
    }
    case "gamesFailed":
      return { ...state, gamesLoading: false, gamesError: action.error };
    case "selectGame":
      if (action.gameId === state.selectedGameId) return state;
      return {
        ...state,
        selectedGameId: action.gameId,
        clips: [],
        clipsLoading: false,
        clipsError: null,
        generateError: null,
        theme: "all",
        selectedClipId: null,
        notice: null,
      };
    case "clipsLoading":
      return { ...state, clipsLoading: true, clipsError: null };
    case "clipsLoaded": {
      const game = selectedGame(state);
      if (!game || game.sessionId !== action.sessionId) return state;
      return { ...withClips(state, action.clips), clipsLoading: false, clipsError: null };
    }
    case "clipsFailed":
      return { ...state, clipsLoading: false, clipsError: action.error };
    case "generateStarted":
      return {
        ...state,
        generating: { sessionId: action.sessionId, done: 0, total: 0 },
        generateError: null,
      };
    case "generateProgress":
      if (state.generating?.sessionId !== action.progress.sessionId) return state;
      return { ...state, generating: action.progress };
    case "generateDone": {
      const games = state.games.map((g) =>
        g.sessionId === action.sessionId ? { ...g, clipsReady: true } : g,
      );
      const game = state.games.find((g) => g.gameId === state.selectedGameId);
      const base: State = { ...state, games, generating: null, generateError: null };
      return game?.sessionId === action.sessionId ? withClips(base, action.clips) : base;
    }
    case "generateFailed":
      return { ...state, generating: null, generateError: action.error };
    case "setTheme":
      return {
        ...state,
        theme: action.theme,
        selectedClipId: resolveSelection(state.clips, action.theme, state.selectedClipId),
      };
    case "selectClip":
      return { ...state, selectedClipId: action.clipId };
    case "stepClip": {
      const visible = visibleClips(state);
      if (visible.length === 0) return state;
      const current = currentClip(state);
      const idx = visible.findIndex((c) => c.id === current?.id);
      const next = visible[Math.min(visible.length - 1, Math.max(0, idx + action.delta))];
      return next ? { ...state, selectedClipId: next.id } : state;
    }
    case "recordingDeleted":
      return state.deletedSessions.includes(action.sessionId)
        ? state
        : { ...state, deletedSessions: [...state.deletedSessions, action.sessionId] };
    case "notice":
      return { ...state, notice: action.message };
  }
}

export type Listener = (state: State, prev: State) => void;

export interface Store {
  getState(): State;
  dispatch(action: Action): void;
  subscribe(listener: Listener): () => void;
}

export function createStore(initial: State = initialState()): Store {
  let state = initial;
  const listeners = new Set<Listener>();
  return {
    getState: () => state,
    dispatch(action) {
      const prev = state;
      state = reducer(state, action);
      if (state !== prev) for (const l of [...listeners]) l(state, prev);
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
