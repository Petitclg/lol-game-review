// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../api";
import type { Ddragon } from "../ddragon";
import type { AnnotatedMoment, GameSummary } from "../types";
import { createAnnotationsView } from "./annotationsView";

const at = (year: number) => new Date(year, 5, 15, 12).getTime();

function game(extra: Partial<GameSummary> = {}): GameSummary {
  return {
    gameId: 1,
    createdMs: at(2026),
    durationSecs: 1800,
    queueId: 420,
    championId: 103,
    kills: 4,
    deaths: 2,
    assists: 9,
    win: false,
    ...extra,
  };
}

function moment(partId: string, extra: Partial<AnnotatedMoment> = {}): AnnotatedMoment {
  const key = partId.replace(/[^a-z0-9]/g, "");
  return {
    sessionId: "s1",
    partId,
    theme: "death",
    label: "Mort",
    gameTimeSecs: 160,
    tags: [],
    clipId: key,
    file: `C:\\clips\\${key}.mp4`,
    game: game(),
    ...extra,
  };
}

const MOMENTS: AnnotatedMoment[] = [
  moment("death-160.0", { tags: ["face_check", "greed", "custom:Trop avancé sans vision"] }),
  moment("death-400.0", { gameTimeSecs: 400, tags: ["face_check"] }),
  moment("kill-500.0", { theme: "kill", label: "Kill", tags: ["good_outplay"], sessionId: "s2", game: game({ gameId: 2, championId: 222, win: true, createdMs: at(2025) }) }),
  moment("death-90.0", { gameTimeSecs: 90, tags: ["outnumbered"], sessionId: "s3", file: null, clipId: null, game: null }),
];

const DD = {
  version: "x",
  champions: {
    byKey: new Map([
      [103, { id: "Ahri", name: "Ahri", key: 103 }],
      [222, { id: "Jinx", name: "Jinx", key: 222 }],
    ]),
    byName: new Map(),
  },
} as unknown as Ddragon;

function setup(list: AnnotatedMoment[] | Error = MOMENTS, videoSrc: (p: string) => string | null = (p) => `asset://${p}`) {
  const listAnnotatedMoments = vi.fn(() => (list instanceof Error ? Promise.reject(list.message) : Promise.resolve(list)));
  const backend = { isMock: false, videoSrc, listAnnotatedMoments } as unknown as Backend;
  const onClose = vi.fn();
  const view = createAnnotationsView(backend, { onClose });
  document.body.appendChild(view.el);
  const q = <T extends HTMLElement>(sel: string) => view.el.querySelector(sel) as T;
  const items = () => [...view.el.querySelectorAll<HTMLButtonElement>(".am-item")];
  const setSelect = (id: string, value: string) => {
    const sel = q<HTMLSelectElement>(id);
    sel.value = value;
    sel.dispatchEvent(new Event("change"));
  };
  return { view, onClose, q, items, setSelect, listAnnotatedMoments, video: q<HTMLVideoElement>("video") };
}

beforeEach(() => {
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(() => Promise.resolve());
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
});

afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe("vue des annotations : liste", () => {
  it("est masquée tant qu'on ne l'ouvre pas, puis charge et affiche tous les moments", async () => {
    const ui = setup();
    expect(ui.view.el.hidden).toBe(true);
    expect(ui.listAnnotatedMoments).not.toHaveBeenCalled();
    await ui.view.open();
    expect(ui.view.el.hidden).toBe(false);
    expect(ui.items()).toHaveLength(4);
    expect(ui.q(".am-count").textContent).toBe("4 moments annotés, 3 avec clip à revoir");
  });

  it("montre champion, issue, date et KDA de la partie, ou « Partie inconnue »", async () => {
    const ui = setup();
    ui.view.setDdragon(DD);
    await ui.view.open();
    const first = ui.items()[0]?.textContent ?? "";
    expect(first).toContain("Ahri · Défaite");
    expect(first).toContain("4/2/9");
    expect(first).toContain("Trop avancé sans vision");
    expect(first).toContain("Face-check");
    expect(ui.items()[2]?.textContent).toContain("Jinx · Victoire");
    expect(ui.items()[3]?.textContent).toContain("Partie inconnue");
    expect(ui.items()[3]?.textContent).toContain("clip indisponible");
  });

  it("sans Data Dragon, le champion reste identifiable", async () => {
    const ui = setup();
    await ui.view.open();
    expect(ui.items()[0]?.textContent).toContain("Champion 103");
  });

  it("aucune annotation : message d'aide, pas de filtres", async () => {
    const ui = setup([]);
    await ui.view.open();
    expect(ui.q(".placeholder").textContent).toContain("Aucune annotation pour l'instant");
    expect(ui.q<HTMLElement>(".am-filters").hidden).toBe(true);
    expect(ui.items()).toHaveLength(0);
  });

  it("échec du chargement : message d'erreur", async () => {
    const ui = setup(new Error("disque illisible"));
    await ui.view.open();
    expect(ui.q(".placeholder").textContent).toBe("Impossible de charger les annotations : disque illisible");
  });

  it("« ← Parties » demande le retour à la liste des parties", async () => {
    const ui = setup();
    await ui.view.open();
    ui.q<HTMLButtonElement>(".am-head button").click();
    expect(ui.onClose).toHaveBeenCalledTimes(1);
  });
});

describe("vue des annotations : filtres", () => {
  it("proposent les champions, années, types et étiquettes présents", async () => {
    const ui = setup();
    ui.view.setDdragon(DD);
    await ui.view.open();
    const texts = (sel: string) => [...ui.view.el.querySelectorAll(`${sel} option`)].map((o) => o.textContent);
    expect(texts("#am-champion")).toEqual(["Tous", "Ahri", "Jinx"]);
    expect(texts("#am-year")).toEqual(["Toutes", "2026", "2025"]);
    expect(texts("#am-outcome")).toEqual(["Toutes", "Victoires", "Défaites"]);
    expect(texts("#am-theme")).toEqual(["Tous", "Kill", "Mort"]);
    expect(texts("#am-tag")).toContain("Face-check");
  });

  it("par champion, année et issue", async () => {
    const ui = setup();
    ui.view.setDdragon(DD);
    await ui.view.open();
    ui.setSelect("#am-champion", "222");
    expect(ui.items()).toHaveLength(1);
    expect(ui.items()[0]?.textContent).toContain("Jinx");
    ui.setSelect("#am-champion", "");
    ui.setSelect("#am-year", "2026");
    expect(ui.items()).toHaveLength(2);
    ui.setSelect("#am-year", "");
    ui.setSelect("#am-outcome", "loss");
    expect(ui.items()).toHaveLength(2);
    ui.setSelect("#am-outcome", "win");
    expect(ui.items()).toHaveLength(1);
  });

  it("par type de moment et par texte (noms des tags, personnalisés compris)", async () => {
    const ui = setup();
    await ui.view.open();
    ui.setSelect("#am-theme", "kill");
    expect(ui.items()).toHaveLength(1);
    ui.setSelect("#am-theme", "");
    const text = ui.q<HTMLInputElement>("#am-text");
    text.value = "AVANCE";
    text.dispatchEvent(new Event("input"));
    expect(ui.items()).toHaveLength(1);
    expect(ui.items()[0]?.textContent).toContain("Trop avancé");
  });

  it("aucun résultat : message, puis « Réinitialiser » rétablit tout", async () => {
    const ui = setup();
    await ui.view.open();
    ui.setSelect("#am-champion", "103");
    ui.setSelect("#am-outcome", "win");
    expect(ui.items()).toHaveLength(0);
    expect(ui.q(".placeholder").textContent).toBe("Aucun moment ne correspond à ces filtres.");
    const reset = ui.q<HTMLButtonElement>(".am-filters .btn");
    expect(reset.disabled).toBe(false);
    reset.click();
    expect(ui.items()).toHaveLength(4);
    expect(reset.disabled).toBe(true);
  });
});

describe("vue des annotations : décompte des étiquettes", () => {
  const chip = (ui: ReturnType<typeof setup>, tag: string) =>
    ui.q<HTMLButtonElement>(`.am-summary [data-tag="${tag}"]`);

  it("compte les moments par étiquette, les plus fréquentes d'abord", async () => {
    const ui = setup();
    await ui.view.open();
    const labels = [...ui.view.el.querySelectorAll(".am-summary .tag-chip")].map((c) => c.textContent);
    expect(labels[0]).toBe("Face-check 2");
    expect(labels).toContain("Greed 1");
  });

  it("cliquer sur une étiquette ne garde que les moments qui la portent, recliquer retire le filtre", async () => {
    const ui = setup();
    await ui.view.open();
    chip(ui, "face_check").click();
    expect(ui.items()).toHaveLength(2);
    expect(chip(ui, "face_check").getAttribute("aria-pressed")).toBe("true");
    expect(ui.q<HTMLSelectElement>("#am-tag").value).toBe("face_check");
    chip(ui, "face_check").click();
    expect(ui.items()).toHaveLength(4);
  });

  it("le décompte suit les autres filtres : face-check sur les défaites de 2026 = 2", async () => {
    const ui = setup();
    await ui.view.open();
    ui.setSelect("#am-outcome", "win");
    expect([...ui.view.el.querySelectorAll(".am-summary .tag-chip")].map((c) => c.textContent)).toEqual([
      "Bon outplay 1",
    ]);
    ui.setSelect("#am-outcome", "loss");
    expect(chip(ui, "face_check").textContent).toBe("Face-check 2");
  });

  it("garde l'étiquette choisie visible pour pouvoir en changer", async () => {
    const ui = setup();
    await ui.view.open();
    chip(ui, "greed").click();
    expect(ui.items()).toHaveLength(1);
    expect(chip(ui, "face_check")).not.toBeNull();
  });
});

describe("vue des annotations : revoir les clips", () => {
  it("charge le clip du premier moment sans le lancer, avec ses tags", async () => {
    const ui = setup();
    await ui.view.open();
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death1600.mp4");
    expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();
    expect(ui.q(".am-now").textContent).toContain("Trop avancé sans vision");
  });

  it("cliquer sur un moment lit son clip", async () => {
    const ui = setup();
    await ui.view.open();
    ui.items()[1]?.click();
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
    expect(ui.items()[1]?.getAttribute("aria-current")).toBe("true");
    expect(HTMLMediaElement.prototype.play).toHaveBeenCalled();
  });

  it("suivant / précédent parcourent les clips filtrés et sautent ceux qui n'existent plus", async () => {
    const ui = setup();
    await ui.view.open();
    const [prev, next] = [...ui.view.el.querySelectorAll<HTMLButtonElement>(".player-nav .btn")] as [HTMLButtonElement, HTMLButtonElement];
    expect(prev.disabled).toBe(true);
    next.click();
    next.click();
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\kill5000.mp4");
    expect(next.disabled).toBe(true); // le dernier moment n'a plus de clip
    prev.click();
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
  });

  it("la lecture continue enchaîne le clip suivant à la fin du clip, sauf si elle est décochée", async () => {
    const ui = setup();
    await ui.view.open();
    ui.video.dispatchEvent(new Event("ended"));
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
    const box = ui.q<HTMLInputElement>("#am-autoplay");
    box.checked = false;
    box.dispatchEvent(new Event("change"));
    ui.video.dispatchEvent(new Event("ended"));
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
  });

  it("revoir tous les face-check : le filtre par étiquette restreint la lecture", async () => {
    const ui = setup();
    await ui.view.open();
    ui.q<HTMLButtonElement>('.am-summary [data-tag="face_check"]').click();
    ui.video.dispatchEvent(new Event("ended"));
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
    ui.video.dispatchEvent(new Event("ended"));
    expect(ui.video.getAttribute("src")).toBe("asset://C:\\clips\\death4000.mp4");
  });

  it("un moment dont le clip est supprimé le dit au lieu de lire une vidéo", async () => {
    const ui = setup();
    await ui.view.open();
    ui.items()[3]?.click();
    expect(ui.video.hasAttribute("src")).toBe(false);
    expect(ui.q(".video-msg").textContent).toContain("n'existe plus");
  });

  it("source illisible (mode simulé) : message d'aperçu indisponible", async () => {
    const ui = setup(MOMENTS, () => null);
    await ui.view.open();
    expect(ui.q(".video-msg").textContent).toContain("mode simulé");
  });

  it("fermer la vue arrête la lecture", async () => {
    const ui = setup();
    await ui.view.open();
    ui.view.close();
    expect(ui.view.el.hidden).toBe(true);
    expect(ui.video.hasAttribute("src")).toBe(false);
  });

  it("insère tags et libellés comme du texte, jamais comme du HTML", async () => {
    const ui = setup([moment("death-1.0", { tags: ["custom:<img src=x onerror=alert(1)>"], label: "<b>x</b>" })]);
    await ui.view.open();
    expect(ui.view.el.querySelector("img")).toBeNull();
    expect(ui.view.el.querySelector("b")).toBeNull();
    expect(ui.view.el.textContent).toContain("<img src=x onerror=alert(1)>");
  });
});
