import { describe, expect, it } from "vitest";
import { formatDuration, formatKda, formatRelativeDate, queueLabel } from "./format";

describe("formatDuration", () => {
  it("mm:ss", () => {
    expect(formatDuration(0)).toBe("00:00");
    expect(formatDuration(65)).toBe("01:05");
    expect(formatDuration(1812)).toBe("30:12");
  });
  it("heures au-delà de 3600 s", () => {
    expect(formatDuration(3725)).toBe("1:02:05");
  });
  it("valeurs invalides ramenées à 0", () => {
    expect(formatDuration(-5)).toBe("00:00");
    expect(formatDuration(NaN)).toBe("00:00");
    expect(formatDuration(59.9)).toBe("00:59");
  });
});

describe("queueLabel", () => {
  it("files connues", () => {
    expect(queueLabel(420)).toBe("Ranked Solo");
    expect(queueLabel(440)).toBe("Ranked Flex");
    expect(queueLabel(400)).toBe("Normal Draft");
    expect(queueLabel(430)).toBe("Normal Blind");
    expect(queueLabel(450)).toBe("ARAM");
    expect(queueLabel(490)).toBe("Quickplay");
  });
  it("file inconnue", () => {
    expect(queueLabel(1700)).toBe("File 1700");
  });
});

describe("formatRelativeDate", () => {
  const now = 1_700_000_000_000;
  it("échelles de temps", () => {
    expect(formatRelativeDate(now - 10_000, now)).toBe("À l'instant");
    expect(formatRelativeDate(now - 5 * 60_000, now)).toBe("Il y a 5 min");
    expect(formatRelativeDate(now - 3 * 3_600_000, now)).toBe("Il y a 3 h");
    expect(formatRelativeDate(now - 30 * 3_600_000, now)).toBe("Hier");
    expect(formatRelativeDate(now - 4 * 86_400_000, now)).toBe("Il y a 4 j");
    expect(formatRelativeDate(now - 65 * 86_400_000, now)).toBe("Il y a 2 mois");
    expect(formatRelativeDate(now - 800 * 86_400_000, now)).toBe("Il y a 2 ans");
  });
  it("date future → à l'instant", () => {
    expect(formatRelativeDate(now + 10_000, now)).toBe("À l'instant");
  });
});

describe("formatKda", () => {
  it("k/d/a", () => {
    expect(formatKda(1, 2, 3)).toBe("1/2/3");
  });
});
