// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
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
