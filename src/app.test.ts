// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "./api";
import { createApp, type App } from "./app";
import { createMockBackend } from "./mock";
import type { ClipsGenerated, ClipsProgress } from "./types";

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

  it("affiche la version de l'application dans l'en-tête", async () => {
    await until(() => root.querySelector<HTMLElement>(".app-version")?.hidden === false);
    expect(root.querySelector(".app-version")?.textContent).toBe("v0.0.0-dev");
  });

  it("change de compte avec le menu de l'en-tête", async () => {
    await until(() => root.querySelectorAll(".game-item").length > 0);
    const select = root.querySelector<HTMLSelectElement>(".account-select");
    expect(select?.hidden).toBe(false);
    expect(select?.value).toBe("invocateur#euw");
    expect(root.querySelectorAll(".game-item")).toHaveLength(6);

    root.querySelector<HTMLElement>(".game-item")?.click();
    select!.value = "remplacant#euw";
    select!.dispatchEvent(new Event("change"));
    const items = root.querySelectorAll(".game-item");
    expect(items).toHaveLength(1);
    expect(items[0]?.textContent).toContain("Résultat inconnu");
    expect(root.querySelector(".clip-list")?.children.length ?? 0).toBe(0);
  });

  it("garde l'en-tête utilisable quand la version est indisponible", async () => {
    app.destroy();
    root.replaceChildren();
    const backend = { ...createMockBackend(), getVersion: () => Promise.reject("indisponible") };
    app = createApp(root, backend);
    await until(() => root.querySelectorAll(".game-item").length > 0);
    expect(root.querySelector<HTMLElement>(".app-version")?.hidden).toBe(true);
    expect(root.querySelector(".app-title")?.textContent).toBe("LoL Game Review");
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
    expect(root.querySelectorAll('[role="tab"]').length).toBe(9);

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

  it("la vue « Annotations » remplace la liste des parties, et on y retrouve ce qu'on a annoté", async () => {
    await until(() => root.querySelectorAll(".game-item").length > 0);
    root.querySelectorAll<HTMLButtonElement>(".game-item")[0]?.click();
    root.querySelector<HTMLButtonElement>(".toolbar .btn-primary")?.click();
    await until(() => root.querySelectorAll(".clip-item").length > 0, 8000);
    root.querySelector<HTMLElement>('[data-theme="death"]')?.click();
    const chip = root.querySelector<HTMLButtonElement>('.annotation-part [data-tag="face_check"]') as HTMLButtonElement;
    chip.click();
    await new Promise((r) => setTimeout(r, 200));

    const button = root.querySelector<HTMLButtonElement>(".annotations-btn") as HTMLButtonElement;
    button.click();
    expect(root.querySelector<HTMLElement>(".layout")?.hidden).toBe(true);
    expect(root.querySelector<HTMLElement>(".annotations-view")?.hidden).toBe(false);
    expect(button.getAttribute("aria-pressed")).toBe("true");
    await until(() => root.querySelectorAll(".am-item").length === 1);
    expect(root.querySelector(".am-item")?.textContent).toContain("Face-check");
    expect(root.querySelector(".am-summary")?.textContent).toContain("Face-check 1");

    root.querySelector<HTMLButtonElement>(".annotations-view .am-head button")?.click();
    expect(root.querySelector<HTMLElement>(".layout")?.hidden).toBe(false);
    expect(root.querySelector<HTMLElement>(".annotations-view")?.hidden).toBe(true);
    expect(button.getAttribute("aria-pressed")).toBe("false");
  }, 20000);

  it("annote un clip fusionné : étiquettes par moment, tags personnalisés, enregistrement et marque dans la liste", async () => {
    app.destroy();
    root.replaceChildren();
    const backend = createMockBackend();
    const setAnnotation = vi.spyOn(backend, "setAnnotation");
    app = createApp(root, backend);
    await until(() => root.querySelectorAll(".game-item").length > 0);
    const sessionId = (await backend.listGames(6))[0]?.sessionId as string;
    root.querySelectorAll<HTMLButtonElement>(".game-item")[0]?.click();
    root.querySelector<HTMLButtonElement>(".toolbar .btn-primary")?.click();
    await until(() => root.querySelectorAll(".clip-item").length > 0, 8000);
    expect(root.querySelector<HTMLElement>(".annotations")?.hidden).toBe(false);

    // le clip fusionné (kill + mort) apparaît dans les deux onglets, avec ses deux thèmes
    root.querySelector<HTMLElement>('[data-theme="death"]')?.click();
    const merged = [...root.querySelectorAll<HTMLButtonElement>(".clip-item")].find((b) =>
      b.textContent?.includes("Kill +"),
    );
    expect(merged?.querySelector(".clip-theme")?.textContent).toContain("Mort · Kill");
    merged?.click();
    const parts = root.querySelectorAll<HTMLElement>(".annotation-part");
    expect(parts.length).toBe(2);
    const [killPart, deathPart] = [...parts] as [HTMLElement, HTMLElement];
    expect(killPart.querySelector('[data-tag="good_outplay"]')).not.toBeNull();
    expect(killPart.querySelector('[data-tag="face_check"]')).toBeNull();
    expect(deathPart.querySelector('[data-tag="face_check"]')).not.toBeNull();

    // une étiquette d'erreur sur la mort
    const chip = deathPart.querySelector<HTMLButtonElement>('[data-tag="face_check"]') as HTMLButtonElement;
    chip.click();
    const partId = deathPart.dataset.part as string;
    expect(chip.getAttribute("aria-pressed")).toBe("true");
    expect(merged?.isConnected).toBe(false); // la liste est redessinée avec la marque
    expect(root.querySelector(".clip-annotated")?.textContent).toContain("✎ 1");
    await until(() => setAnnotation.mock.calls.length === 1);
    expect(setAnnotation).toHaveBeenLastCalledWith(sessionId, partId, ["face_check"]);

    // un tag personnalisé : il est créé, posé sur le moment, puis proposé partout
    const input = deathPart.querySelector<HTMLInputElement>(".custom-tag-input") as HTMLInputElement;
    input.value = "  trop avancé  sans vision ";
    deathPart.querySelector("form")?.dispatchEvent(new Event("submit", { cancelable: true }));
    await until(() => setAnnotation.mock.calls.length === 2);
    expect(setAnnotation).toHaveBeenLastCalledWith(sessionId, partId, ["face_check", "custom:trop avancé sans vision"]);
    expect(await backend.getCustomTags()).toEqual(["trop avancé sans vision"]);
    await until(() => killPart.querySelector('[data-tag="custom:trop avancé sans vision"]') !== null);
    expect(
      deathPart.querySelector('[data-tag="custom:trop avancé sans vision"]')?.getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      killPart.querySelector('[data-tag="custom:trop avancé sans vision"]')?.getAttribute("aria-pressed"),
    ).toBe("false");
    expect(await backend.getAnnotations(sessionId)).toEqual({
      [partId]: { tags: ["face_check", "custom:trop avancé sans vision"] },
    });

    // le même tag, tapé avec une autre casse, retrouve celui qui existe
    input.value = "TROP AVANCÉ SANS VISION";
    deathPart.querySelector("form")?.dispatchEvent(new Event("submit", { cancelable: true }));
    await until(() => setAnnotation.mock.calls.length === 3);
    expect(await backend.getCustomTags()).toEqual(["trop avancé sans vision"]);
    expect(setAnnotation).toHaveBeenLastCalledWith(sessionId, partId, ["face_check", "custom:trop avancé sans vision"]);

    // la croix ne propose plus le tag mais le moment le garde
    const cross = deathPart.querySelector<HTMLButtonElement>(".tag-remove") as HTMLButtonElement;
    cross.click();
    await until(() => deathPart.querySelector(".tag-remove") === null);
    expect(await backend.getCustomTags()).toEqual([]);
    expect(killPart.querySelector('[data-tag="custom:trop avancé sans vision"]')).toBeNull();
    expect(
      deathPart.querySelector('[data-tag="custom:trop avancé sans vision"]')?.getAttribute("aria-pressed"),
    ).toBe("true");
    expect((await backend.getAnnotations(sessionId))[partId]?.tags).toContain("custom:trop avancé sans vision");

    // décocher le tag puis l'étiquette supprime l'annotation
    deathPart.querySelector<HTMLButtonElement>('[data-tag="custom:trop avancé sans vision"]')?.click();
    chip.click();
    await until(() => setAnnotation.mock.calls.length === 5);
    await new Promise((r) => setTimeout(r, 150));
    expect(await backend.getAnnotations(sessionId)).toEqual({});
    expect(root.querySelector(".clip-annotated")).toBeNull();
  }, 25000);

  it("génération automatique : suit la progression, puis rafraîchit les parties et les clips", async () => {
    app.destroy();
    root.replaceChildren();
    const base = createMockBackend();
    let emit: ((e: ClipsGenerated) => void) | undefined;
    let progress: ((p: ClipsProgress) => void) | undefined;
    const backend: Backend = {
      ...base,
      onClipsGenerated: (cb) => {
        emit = cb;
        return Promise.resolve(() => undefined);
      },
      onClipsProgress: (cb) => {
        progress = cb;
        return Promise.resolve(() => undefined);
      },
    };
    const listGames = vi.spyOn(backend, "listGames");
    const getClips = vi.spyOn(backend, "getClips");
    app = createApp(root, backend);
    await until(() => root.querySelectorAll(".game-item").length > 0);
    const sessionId = (await base.listGames(6))[0]?.sessionId as string;
    root.querySelectorAll<HTMLButtonElement>(".game-item")[0]?.click();
    // les clips de cette partie existent déjà côté simulé une fois générés
    await base.generateClips(sessionId);
    await until(() => emit !== undefined && progress !== undefined);

    progress?.({ sessionId, done: 2, total: 9 });
    await until(() => root.querySelector(".progress-text")?.textContent?.includes("2 / 9") === true);
    expect(root.querySelector<HTMLButtonElement>(".toolbar .btn-primary")?.disabled).toBe(true);

    const gamesBefore = listGames.mock.calls.length;
    const clipsBefore = getClips.mock.calls.length;
    emit?.({ sessionId, error: null });
    await until(() => listGames.mock.calls.length > gamesBefore && getClips.mock.calls.length > clipsBefore);
    await until(() => root.querySelectorAll(".clip-item").length > 0);
    expect(root.querySelector<HTMLElement>(".progress-box")?.hidden).toBe(true);
  }, 20000);

  it("génération automatique échouée : message, et plus de progression", async () => {
    app.destroy();
    root.replaceChildren();
    let emit: ((e: ClipsGenerated) => void) | undefined;
    const backend: Backend = {
      ...createMockBackend(),
      onClipsGenerated: (cb) => {
        emit = cb;
        return Promise.resolve(() => undefined);
      },
    };
    app = createApp(root, backend);
    await until(() => root.querySelectorAll(".game-item").length > 0 && emit !== undefined);
    root.querySelectorAll<HTMLButtonElement>(".game-item")[0]?.click();
    emit?.({ sessionId: "inconnue", error: "ffmpeg a échoué" });
    await until(() => root.textContent?.includes("Génération automatique des clips échouée : ffmpeg a échoué") === true);
  });

  it("signale une annotation qui n'a pas pu être enregistrée et revient à l'état enregistré", async () => {
    app.destroy();
    root.replaceChildren();
    const backend = createMockBackend();
    vi.spyOn(backend, "setAnnotation").mockRejectedValue("disque plein");
    app = createApp(root, backend);
    await until(() => root.querySelectorAll(".game-item").length > 0);
    root.querySelectorAll<HTMLButtonElement>(".game-item")[0]?.click();
    root.querySelector<HTMLButtonElement>(".toolbar .btn-primary")?.click();
    await until(() => root.querySelectorAll(".clip-item").length > 0, 8000);

    const chip = root.querySelector<HTMLButtonElement>(".annotation-part .tag-chip") as HTMLButtonElement;
    chip.click();
    await until(() => root.textContent?.includes("Annotation non enregistrée : disque plein") === true);
    await until(
      () =>
        root.querySelector(".annotation-part .tag-chip")?.getAttribute("aria-pressed") === "false",
    );
  }, 20000);
});
