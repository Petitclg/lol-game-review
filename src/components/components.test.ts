// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { countByTheme, filterClips, type ThemeFilter } from "../clips";
import { SAMPLE_CLIPS, makeClip } from "../testUtils";
import { renderTabs } from "./tabs";
import { statusLabel } from "./header";
import { gameBadge } from "./gameList";
import { el } from "../dom";

describe("renderTabs", () => {
  it("affiche un onglet par thème avec compteurs et aria-selected", () => {
    const container = document.createElement("div");
    renderTabs(container, countByTheme(SAMPLE_CLIPS), "kill", () => undefined);
    const tabs = [...container.querySelectorAll<HTMLElement>('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual([
      "Tous6",
      "Kills2",
      "Morts1",
      "Assists1",
      "Objectifs1",
      "Tours1",
    ]);
    expect(tabs.filter((t) => t.getAttribute("aria-selected") === "true").map((t) => t.dataset["theme"])).toEqual(["kill"]);
    expect(container.getAttribute("role")).toBe("tablist");
  });

  it("le clic sélectionne le thème et le filtrage correspond", () => {
    const container = document.createElement("div");
    let active: ThemeFilter = "all";
    const rerender = () => renderTabs(container, countByTheme(SAMPLE_CLIPS), active, (t) => {
      active = t;
      onSelect(t);
      rerender();
    });
    const onSelect = vi.fn();
    rerender();
    container.querySelector<HTMLElement>('[data-theme="death"]')?.click();
    expect(onSelect).toHaveBeenCalledWith("death");
    expect(container.querySelector('[data-theme="death"]')?.getAttribute("aria-selected")).toBe("true");
    expect(filterClips(SAMPLE_CLIPS, active).map((c) => c.id)).toEqual(["d"]);
  });
});

describe("rendu sûr", () => {
  it("el() n'interprète jamais le HTML des chaînes", () => {
    const evil = '<img src=x onerror="alert(1)">';
    const node = el("div", { text: evil }, [evil]);
    expect(node.querySelector("img")).toBeNull();
    expect(node.textContent).toBe(evil + evil);
  });
  it("les clips sans fichier restent représentables", () => {
    expect(makeClip("z", "kill", 1, { file: null }).file).toBeNull();
  });
});

describe("statusLabel / gameBadge", () => {
  it("états du statut", () => {
    expect(statusLabel(null).kind).toBe("unknown");
    expect(statusLabel({ state: "idle", recording: false, ffmpegAvailable: true }).text).toBe("Inactif");
    expect(statusLabel({ state: "inGame", recording: false, ffmpegAvailable: true }).text).toBe("En partie");
    expect(statusLabel({ state: "inGame", recording: true, ffmpegAvailable: true }).text).toBe("Enregistrement en cours");
  });
  it("badges de partie", () => {
    const base = {
      gameId: 1, createdMs: 0, durationSecs: 1, queueId: 420, championId: 1,
      kills: 0, deaths: 0, assists: 0, win: true,
    };
    expect(gameBadge({ ...base, sessionId: null, clipsReady: false }).text).toBe("Pas de vidéo");
    expect(gameBadge({ ...base, sessionId: "s", clipsReady: false }).text).toBe("Enregistrée");
    expect(gameBadge({ ...base, sessionId: "s", clipsReady: true }).text).toBe("Clips prêts");
  });
});
