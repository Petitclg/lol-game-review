import { baseCooldownForRank, effectiveCooldown, formatCooldown } from "../cooldown";
import {
  championIconUrl,
  findChampionByName,
  getChampionSpells,
  itemIconUrl,
  type Ddragon,
  type SpellInfo,
} from "../ddragon";
import { el, remoteImage, replaceChildren } from "../dom";
import { formatDuration, formatKda } from "../format";
import { currentClip, type State } from "../state/store";
import type { Clip, PlayerSnapshot } from "../types";

export const COOLDOWN_LABEL = "Cooldown de base au rang actuel (estimation)";

export interface SidePanel {
  el: HTMLElement;
  update(state: State, dd: Ddragon | null): void;
}

function playerRow(p: PlayerSnapshot, dd: Ddragon | null): HTMLElement {
  const champ = findChampionByName(dd?.champions ?? null, p.champion);
  const items = p.items
    .filter((i) => i.id > 0)
    .map((item) => {
      const wrap = el("span", {
        class: "item",
        attrs: { title: item.name, role: "img", "aria-label": item.name },
      });
      wrap.appendChild(remoteImage(dd ? itemIconUrl(dd.version, item.id) : null, "", "item-icon"));
      return wrap;
    });
  return el("li", { class: `player${p.isMe ? " me" : ""}`, attrs: { "aria-current": p.isMe ? "true" : undefined } }, [
    remoteImage(dd && champ ? championIconUrl(dd.version, champ.id) : null, "", "champ-icon small"),
    el("span", { class: "player-info" }, [
      el("span", { class: "player-name", text: p.name, attrs: { title: p.name } }),
      el("span", { class: "player-champ", text: `${p.champion} · niv. ${p.level}` }),
    ]),
    el("span", { class: "player-kda", text: formatKda(p.kills, p.deaths, p.assists) }),
    el("span", { class: "player-spells", text: p.spells.join(" / ") }),
    el("span", { class: "player-items" }, items),
  ]);
}

function teamBlock(title: string, className: string, players: PlayerSnapshot[], dd: Ddragon | null) {
  return el("div", { class: `team ${className}` }, [
    el("h4", { text: title }),
    el("ul", { class: "player-list" }, players.map((p) => playerRow(p, dd))),
  ]);
}

const SLOTS = ["q", "w", "e", "r"] as const;

function abilitySection(clip: Clip, spells: SpellInfo[] | null, ddUnavailable: boolean): HTMLElement {
  const section = el("section", { class: "abilities", attrs: { "aria-labelledby": "abilities-heading" } }, [
    el("h3", { text: "Mes sorts", attrs: { id: "abilities-heading" } }),
  ]);
  if (!clip.abilityRanks) {
    section.appendChild(el("p", { class: "muted", text: "Rangs de sorts indisponibles pour ce clip." }));
    return section;
  }
  section.appendChild(el("p", { class: "muted small", text: COOLDOWN_LABEL }));
  const haste = clip.abilityHaste;
  if (haste !== null) {
    section.appendChild(el("p", { class: "muted small", text: `Ability haste : ${haste}` }));
  }
  const rows = SLOTS.map((slot, i) => {
    const rank = clip.abilityRanks?.[slot] ?? 0;
    const spellName = spells?.[i]?.name;
    let detail: string;
    if (rank <= 0) {
      detail = "Non appris";
    } else if (!spells) {
      detail = ddUnavailable ? "Cooldown indisponible (Data Dragon hors ligne)" : "Chargement du cooldown…";
    } else {
      const base = baseCooldownForRank(spells[i]?.cooldown, rank);
      if (base === null) {
        detail = "Cooldown inconnu";
      } else if (haste !== null && haste > 0) {
        detail = `${formatCooldown(base)} → ${formatCooldown(effectiveCooldown(base, haste))} avec haste`;
      } else {
        detail = formatCooldown(base);
      }
    }
    return el("li", { class: "ability" }, [
      el("span", { class: "ability-key", text: slot.toUpperCase() }),
      el("span", { class: "ability-name", text: spellName ?? "" }),
      el("span", { class: "ability-rank", text: rank > 0 ? `Rang ${rank}` : "Rang 0" }),
      el("span", { class: "ability-cd", text: detail }),
    ]);
  });
  section.appendChild(el("ul", { class: "ability-list" }, rows));
  return section;
}

export function createSidePanel(): SidePanel {
  const root = el("aside", { class: "side-panel", attrs: { "aria-label": "Détails du clip" } });
  let token = 0;
  let spells: SpellInfo[] | null = null;
  let spellsFor: string | null = null;
  let spellsFailed = false;
  let last: { clip: Clip | null; dd: Ddragon | null; spellsKey: string } | null = null;
  let stateRef: State | null = null;
  let ddRef: Ddragon | null = null;

  function render(clip: Clip | null, dd: Ddragon | null): void {
    last = { clip, dd, spellsKey: `${spellsFor}|${spells ? 1 : 0}|${spellsFailed ? 1 : 0}` };
    if (!clip) {
      replaceChildren(root, [el("p", { class: "muted", text: "Sélectionnez un clip pour voir le détail de la partie à cet instant." })]);
      return;
    }
    const mine = clip.players.find((p) => p.isMe);
    const order = clip.players.filter((p) => p.team === "ORDER");
    const chaos = clip.players.filter((p) => p.team === "CHAOS");
    replaceChildren(root, [
      el("h3", { text: `${clip.label} — ${formatDuration(clip.gameTimeSecs)}` }),
      el("section", { class: "teams" }, [
        teamBlock(mine?.team === "CHAOS" ? "Équipe alliée (Chaos)" : "Équipe alliée (Ordre)", "ally", mine?.team === "CHAOS" ? chaos : order, dd),
        teamBlock(mine?.team === "CHAOS" ? "Équipe ennemie (Ordre)" : "Équipe ennemie (Chaos)", "enemy", mine?.team === "CHAOS" ? order : chaos, dd),
      ]),
      abilitySection(clip, spells, !dd || spellsFailed),
    ]);
  }

  return {
    el: root,
    update(state, dd) {
      const clip = currentClip(state);
      const mine = clip?.players.find((p) => p.isMe);
      const champ = dd && mine ? findChampionByName(dd.champions, mine.champion) : null;
      const wantKey = dd && champ ? `${dd.version}/${champ.id}` : null;

      if (wantKey !== spellsFor) {
        spellsFor = wantKey;
        spells = null;
        spellsFailed = false;
        const mine_token = ++token;
        if (dd && champ) {
          void getChampionSpells(dd.version, champ.id).then((result) => {
            if (mine_token !== token) return;
            spells = result;
            spellsFailed = result === null;
            render(stateRef ? currentClip(stateRef) : null, ddRef);
          });
        } else {
          spellsFailed = true;
        }
      }
      stateRef = state;
      ddRef = dd;

      const key = `${spellsFor}|${spells ? 1 : 0}|${spellsFailed ? 1 : 0}`;
      if (last && last.clip === clip && last.dd === dd && last.spellsKey === key) return;
      render(clip, dd);
    },
  };
}
