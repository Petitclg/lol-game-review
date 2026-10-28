import { championIconUrl, findChampion, type Ddragon, type ItemPrices } from "../ddragon";
import { el, remoteImage } from "../dom";
import {
  FLASH_LABELS,
  FLASH_NOTE,
  GOLD_NOTE,
  ROLE_LABELS,
  UNAVAILABLE_NOTE,
  flashTone,
  formatGold,
  formatItemAdvantage,
  formatLevelAdvantage,
  involvedPlayers,
  isFlashEquipped,
  itemValue,
  summarize,
  toneOf,
  type InvolvedPlayer,
  type Summary,
  type Tone,
} from "../insights";
import type { Clip } from "../types";

function badge(label: string, value: string, tone: Tone, title?: string): HTMLElement {
  return el(
    "li",
    { class: `insight insight-${tone}`, attrs: { title } },
    [el("span", { class: "insight-label", text: label }), el("span", { class: "insight-value", text: value })],
  );
}

function summaryBadges(summary: Summary, hasPrices: boolean): HTMLElement[] {
  const out: HTMLElement[] = [];
  if (summary.levelAdvantage !== null) {
    out.push(
      badge("Niveaux d'avance", formatLevelAdvantage(summary.levelAdvantage), toneOf(summary.levelAdvantage)),
    );
  }
  if (summary.itemAdvantage !== null) {
    const totals = summary.itemTotals;
    out.push(
      badge(
        "Avance d'items",
        formatItemAdvantage(summary.itemAdvantage),
        toneOf(summary.itemAdvantage),
        totals ? `Total alliés : ${formatGold(totals.ally)} · Total ennemis : ${formatGold(totals.enemy)}` : undefined,
      ),
    );
  } else if (!hasPrices) {
    out.push(badge("Avance d'items", "—", "neutral", "Prix des items indisponibles (Data Dragon hors ligne)."));
  }
  if (summary.flash !== null) {
    out.push(badge("Flash", FLASH_LABELS[summary.flash], flashTone(summary.flash), FLASH_NOTE));
  }
  return out;
}

function row(entry: InvolvedPlayer, dd: Ddragon | null, prices: ItemPrices | null): HTMLElement {
  const { player } = entry;
  const champ = findChampion(dd?.champions ?? null, player.champion);
  const value = itemValue(player.items, prices);
  const flash = isFlashEquipped(player);
  return el(
    "tr",
    { class: `involved-row ${entry.isAlly ? "ally" : "enemy"}${entry.isMe ? " me" : ""}` },
    [
      el("td", { class: "involved-who" }, [
        remoteImage(dd && champ ? championIconUrl(dd.version, champ.id) : null, "", "champ-icon small"),
        el("span", { class: "involved-id" }, [
          el("span", { class: "involved-champ", text: champ?.name ?? player.champion, attrs: { title: player.name } }),
          el(
            "span",
            { class: "roles" },
            entry.roles.map((r) => el("span", { class: `role role-${r}`, text: ROLE_LABELS[r] })),
          ),
        ]),
      ]),
      el("td", { class: "involved-level", text: String(player.level) }),
      el("td", { class: "involved-value", text: value === null ? "—" : formatGold(value) }),
      el("td", {
        class: `involved-flash ${flash ? "yes" : "no"}`,
        text: flash ? "✓" : "✗",
        attrs: { title: flash ? "Flash équipé" : "Flash non équipé", "aria-label": flash ? "Flash équipé" : "Flash non équipé" },
      }),
    ],
  );
}

/** Bloc « Analyse du moment » : joueurs impliqués + résumé avantage/désavantage. */
export function renderInsights(clip: Clip, dd: Ddragon | null, prices: ItemPrices | null): HTMLElement {
  const section = el("section", { class: "insights", attrs: { "aria-labelledby": "insights-heading" } }, [
    el("h3", { text: "Analyse du moment", attrs: { id: "insights-heading" } }),
  ]);
  const involved = involvedPlayers(clip);
  if (involved.length === 0) {
    section.appendChild(el("p", { class: "muted small insights-unavailable", text: UNAVAILABLE_NOTE }));
    return section;
  }

  const summary = summarize(involved, prices);
  if (summary) {
    const badges = summaryBadges(summary, prices !== null);
    if (badges.length > 0) section.appendChild(el("ul", { class: "insight-list" }, badges));
  }

  const head = el("tr", {}, [
    el("th", { text: "Joueur", attrs: { scope: "col" } }),
    el("th", { text: "Niv.", attrs: { scope: "col" } }),
    el("th", { text: "Valeur des items", attrs: { scope: "col" } }),
    el("th", { text: "Flash équipé", attrs: { scope: "col", title: FLASH_NOTE } }),
  ]);
  section.appendChild(
    el("table", { class: "involved-table", attrs: { "aria-label": "Joueurs impliqués" } }, [
      el("thead", {}, [head]),
      el("tbody", {}, involved.map((e) => row(e, dd, prices))),
    ]),
  );

  if (clip.myGold !== null && clip.myGold !== undefined) {
    section.appendChild(
      el("p", { class: "my-gold", text: `Or non dépensé : ${formatGold(clip.myGold)}` }),
    );
  }
  section.appendChild(el("p", { class: "muted small insights-note", text: GOLD_NOTE }));
  return section;
}
