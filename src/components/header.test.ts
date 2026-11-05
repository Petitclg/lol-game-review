// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { initialState, reducer, type State } from "../state/store";
import type { GameEntry } from "../types";
import { createHeader, versionLabel } from "./header";

describe("versionLabel", () => {
  it("préfixe le numéro avec « v »", () => {
    expect(versionLabel("0.1.9")).toBe("v0.1.9");
    expect(versionLabel(" 1.2.3 ")).toBe("v1.2.3");
    expect(versionLabel("0.0.0-dev")).toBe("v0.0.0-dev");
  });
  it("n'ajoute pas un second « v »", () => {
    expect(versionLabel("v0.1.9")).toBe("v0.1.9");
    expect(versionLabel("V2.0.0")).toBe("V2.0.0");
  });
  it("vide, blanc ou absent : rien à afficher", () => {
    expect(versionLabel("")).toBeNull();
    expect(versionLabel("   ")).toBeNull();
    expect(versionLabel(null)).toBeNull();
    expect(versionLabel(undefined)).toBeNull();
  });
});

describe("version dans l'en-tête", () => {
  const versionEl = (header: ReturnType<typeof createHeader>) =>
    header.el.querySelector<HTMLElement>(".app-version")!;

  it("est masquée tant que la version n'est pas connue", () => {
    const header = createHeader();
    expect(versionEl(header).hidden).toBe(true);
    expect(versionEl(header).textContent).toBe("");
  });

  it("s'affiche à côté du titre une fois connue", () => {
    const header = createHeader();
    header.setVersion("0.1.9");
    expect(versionEl(header).hidden).toBe(false);
    expect(versionEl(header).textContent).toBe("v0.1.9");
    expect(header.el.querySelector(".app-brand")?.textContent).toBe("LoL Game Review" + "v0.1.9");
  });

  it("se masque de nouveau si la version devient vide", () => {
    const header = createHeader();
    header.setVersion("0.1.9");
    header.setVersion("");
    expect(versionEl(header).hidden).toBe(true);
    expect(versionEl(header).textContent).toBe("");
  });
});

describe("sélecteur de compte", () => {
  const owned = (gameId: number, account: string, createdMs: number): GameEntry => ({
    gameId,
    createdMs,
    durationSecs: 1800,
    queueId: 420,
    championId: 1,
    kills: 0,
    deaths: 0,
    assists: 0,
    win: true,
    account,
    sessionId: null,
    clipsReady: false,
    hasRecording: false,
  });
  const stateWith = (games: GameEntry[], connected: string | null = null): State => {
    let s = reducer(initialState(), {
      type: "accountLoaded",
      account: connected ? { riotId: connected, level: 42 } : null,
    });
    s = reducer(s, { type: "gamesLoaded", games });
    return s;
  };
  const select = (header: ReturnType<typeof createHeader>) =>
    header.el.querySelector<HTMLSelectElement>(".account-select")!;
  const name = (header: ReturnType<typeof createHeader>) =>
    header.el.querySelector<HTMLElement>(".account-name")!;

  it("un seul compte : son nom et son niveau, sans menu", () => {
    const header = createHeader();
    header.update(stateWith([owned(1, "Moi#EUW", 1)], "Moi#EUW"));
    expect(select(header).hidden).toBe(true);
    expect(name(header).hidden).toBe(false);
    expect(name(header).textContent).toBe("Moi#EUW");
    expect(header.el.querySelector(".account-level")?.textContent).toBe("Niv. 42");
  });

  it("plusieurs comptes : un menu, le compte connecté sélectionné et repéré", () => {
    const header = createHeader();
    header.update(stateWith([owned(1, "Moi#EUW", 1), owned(2, "Autre#EUW", 2)], "Moi#EUW"));
    expect(select(header).hidden).toBe(false);
    expect(name(header).hidden).toBe(true);
    expect([...select(header).options].map((o) => o.textContent)).toEqual(["Autre#EUW", "Moi#EUW (connecté)"]);
    expect(select(header).value).toBe("moi#euw");
  });

  it("choisir un compte appelle le gestionnaire avec sa clé", () => {
    const onSelectAccount = vi.fn();
    const header = createHeader({ onSelectAccount });
    header.update(stateWith([owned(1, "Moi#EUW", 1), owned(2, "Autre#EUW", 2)]));
    select(header).value = "moi#euw";
    select(header).dispatchEvent(new Event("change"));
    expect(onSelectAccount).toHaveBeenCalledWith("moi#euw");
  });

  it("le niveau n'est affiché que pour le compte connecté", () => {
    const header = createHeader();
    const state = stateWith([owned(1, "Moi#EUW", 1), owned(2, "Autre#EUW", 2)], "Moi#EUW");
    header.update(state);
    expect(header.el.querySelector<HTMLElement>(".account-level")?.hidden).toBe(false);
    header.update(reducer(state, { type: "selectAccount", key: "autre#euw" }));
    expect(select(header).value).toBe("autre#euw");
    expect(header.el.querySelector<HTMLElement>(".account-level")?.hidden).toBe(true);
  });

  it("ne reconstruit pas le menu quand rien ne change", () => {
    const header = createHeader();
    const state = stateWith([owned(1, "Moi#EUW", 1), owned(2, "Autre#EUW", 2)]);
    header.update(state);
    const first = select(header).options[0];
    header.update({ ...state, notice: "autre chose" });
    expect(select(header).options[0]).toBe(first);
  });

  it("sans compte ni partie : indique l'état du client", () => {
    const header = createHeader();
    header.update(initialState());
    expect(name(header).textContent).toBe("Chargement…");
    header.update(reducer(initialState(), { type: "accountFailed", error: "x" }));
    expect(name(header).textContent).toBe("Client LoL éteint");
    header.update(reducer(initialState(), { type: "accountLoaded", account: null }));
    expect(name(header).textContent).toBe("Aucun compte connecté");
  });
});
