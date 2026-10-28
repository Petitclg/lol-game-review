import { championIconUrl, findChampion, itemIconUrl, type Ddragon } from "../ddragon";
import { el, remoteImage, replaceChildren } from "../dom";
import { formatDuration, formatKda } from "../format";
import { currentClip, type State } from "../state/store";
import type { Clip, PlayerSnapshot } from "../types";

export interface SidePanel {
  el: HTMLElement;
  update(state: State, dd: Ddragon | null): void;
}

function playerRow(p: PlayerSnapshot, dd: Ddragon | null): HTMLElement {
  const champ = findChampion(dd?.champions ?? null, p.champion);
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
  return el("li", { class: `player-row${p.isMe ? " me" : ""}`, attrs: { "aria-current": p.isMe ? "true" : undefined } }, [
    remoteImage(dd && champ ? championIconUrl(dd.version, champ.id) : null, "", "champ-icon small"),
    el("span", { class: "player-info" }, [
      el("span", { class: "player-name", text: p.name, attrs: { title: p.name } }),
      el("span", { class: "player-champ", text: `${champ?.name ?? p.champion} · niv. ${p.level}` }),
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

export function createSidePanel(): SidePanel {
  const root = el("aside", { class: "side-panel", attrs: { "aria-label": "Détails du clip" } });
  let last: { clip: Clip | null; dd: Ddragon | null } | null = null;

  return {
    el: root,
    update(state, dd) {
      const clip = currentClip(state);
      if (last && last.clip === clip && last.dd === dd) return;
      last = { clip, dd };
      if (!clip) {
        replaceChildren(root, [
          el("p", { class: "muted", text: "Sélectionnez un clip pour voir le détail de la partie à cet instant." }),
        ]);
        return;
      }
      const mine = clip.players.find((p) => p.isMe);
      const order = clip.players.filter((p) => p.team === "ORDER");
      const chaos = clip.players.filter((p) => p.team === "CHAOS");
      const allies = mine?.team === "CHAOS" ? chaos : order;
      const enemies = mine?.team === "CHAOS" ? order : chaos;
      const side = (t: "ORDER" | "CHAOS") => (t === "CHAOS" ? "Chaos" : "Ordre");
      const myTeam = mine?.team ?? "ORDER";
      replaceChildren(root, [
        el("h3", { text: `${clip.label} — ${formatDuration(clip.gameTimeSecs)}` }),
        el("section", { class: "teams" }, [
          teamBlock(`Équipe alliée (${side(myTeam)})`, "ally", allies, dd),
          teamBlock(`Équipe ennemie (${side(myTeam === "ORDER" ? "CHAOS" : "ORDER")})`, "enemy", enemies, dd),
        ]),
      ]);
    },
  };
}
