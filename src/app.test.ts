// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, type App } from "./app";
import { createMockBackend } from "./mock";

async function until(cond: () => boolean, timeoutMs = 4000): Promise<void> {
  const start = Date.now();
  while (!cond()) {
    if (Date.now() - start > timeoutMs) throw new Error("timeout");
    await new Promise((r) => setTimeout(r, 20));
  }
}

describe("application (backend simulé, hors ligne)", () => {
  let app: App;
  let root: HTMLElement;

  beforeEach(() => {
    vi.stubGlobal("fetch", () => Promise.reject(new TypeError("offline")));
    root = document.createElement("div");
    document.body.appendChild(root);
    app = createApp(root, createMockBackend());
  });
  afterEach(() => {
    app.destroy();
    root.remove();
    vi.unstubAllGlobals();
  });

  it("liste les parties, génère les clips et navigue au clavier", async () => {
    await until(() => root.querySelectorAll(".game-item").length > 0);
    const items = [...root.querySelectorAll<HTMLButtonElement>(".game-item")];
    expect(items.length).toBe(6);
    expect(root.textContent).toContain("Pas de vidéo");
    expect(root.textContent).toContain("Ranked Solo");
    expect(root.textContent).toContain("File 1700");

    // partie sans vidéo : pas de bouton de génération actif
    items[2]?.click();
    expect(root.querySelector<HTMLElement>(".toolbar")?.hidden).toBe(true);
    expect(root.textContent).toContain("Seules les parties jouées");

    // partie enregistrée : générer
    items[0]?.click();
    const gen = root.querySelector<HTMLButtonElement>(".toolbar .btn-primary");
    expect(gen?.textContent).toBe("Générer les clips");
    expect(gen?.disabled).toBe(false);
    gen?.click();
    await until(() => root.querySelectorAll(".clip-item").length > 0, 8000);
    expect(root.querySelectorAll('[role="tab"]').length).toBe(6);

    const total = root.querySelectorAll(".clip-item").length;
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(root.querySelector(".clip-item.selected .clip-time")).not.toBeNull();
    root.querySelector<HTMLElement>('[data-theme="kill"]')?.click();
    expect(root.querySelectorAll(".clip-item").length).toBeLessThan(total);
    expect(root.querySelector('[data-theme="kill"]')?.getAttribute("aria-selected")).toBe("true");
    expect(root.textContent).not.toMatch(/cooldown/i);
    expect(root.querySelector(".abilities")).toBeNull();
    expect(root.querySelectorAll(".player-list > li").length).toBe(10);
    expect(root.querySelectorAll(".player-list > li.me").length).toBe(1);
    // le conteneur du lecteur ne doit pas partager de classe avec les lignes joueurs (régression de mise en page)
    expect(root.querySelector(".clip-player")?.classList.contains("player-row")).toBe(false);
    expect(root.querySelector(".clip-player > .player-nav")).not.toBeNull();
    expect(root.querySelector(".clip-list .player-nav")).toBeNull();
  }, 15000);
});
