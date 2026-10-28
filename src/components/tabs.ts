import { THEME_TABS, type ThemeFilter } from "../clips";
import { el, replaceChildren } from "../dom";

/** Onglets par thème avec compteurs ; aria-selected reflète l'onglet actif. */
export function renderTabs(
  container: HTMLElement,
  counts: Record<ThemeFilter, number>,
  active: ThemeFilter,
  onSelect: (theme: ThemeFilter) => void,
  panelId = "clip-panel",
): void {
  container.setAttribute("role", "tablist");
  container.setAttribute("aria-label", "Thèmes de clips");
  const hadFocus = container.contains(document.activeElement);
  const tabs = THEME_TABS.map((tab) => {
    const selected = tab.id === active;
    return el(
      "button",
      {
        class: `tab${selected ? " active" : ""}`,
        attrs: {
          type: "button",
          role: "tab",
          id: `tab-${tab.id}`,
          "aria-selected": selected ? "true" : "false",
          "aria-controls": panelId,
          tabindex: 0,
          "data-theme": tab.id,
        },
        on: { click: () => onSelect(tab.id) },
      },
      [tab.label, el("span", { class: "tab-count", text: String(counts[tab.id]) })],
    );
  });
  replaceChildren(container, tabs);
  if (hadFocus) container.querySelector<HTMLElement>('[aria-selected="true"]')?.focus();
}
