type Child = Node | string | null | undefined | false;

export interface ElOptions {
  class?: string;
  text?: string;
  attrs?: Record<string, string | number | boolean | undefined>;
  on?: Record<string, EventListener>;
}

/** Crée un élément ; tout texte passe par textContent / createTextNode (jamais innerHTML). */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  options: ElOptions = {},
  children: Child[] = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (options.class) node.className = options.class;
  if (options.text !== undefined) node.textContent = options.text;
  for (const [name, value] of Object.entries(options.attrs ?? {})) {
    if (value === undefined || value === false) continue;
    node.setAttribute(name, value === true ? "" : String(value));
  }
  for (const [type, handler] of Object.entries(options.on ?? {})) {
    node.addEventListener(type, handler);
  }
  appendChildren(node, children);
  return node;
}

export function appendChildren(parent: Node, children: Child[]): void {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    parent.appendChild(typeof child === "string" ? document.createTextNode(child) : child);
  }
}

export function clear(node: Node): void {
  while (node.firstChild) node.removeChild(node.firstChild);
}

export function replaceChildren(node: Node, children: Child[]): void {
  clear(node);
  appendChildren(node, children);
}

/** Image distante ; si elle ne charge pas (hors ligne), on la masque. */
export function remoteImage(src: string | null, alt: string, className: string): HTMLElement {
  if (!src) return el("span", { class: `${className} img-fallback`, attrs: { "aria-hidden": true } });
  const img = el("img", {
    class: className,
    attrs: { src, alt, loading: "lazy", referrerpolicy: "no-referrer", width: 32, height: 32 },
  });
  img.addEventListener("error", () => {
    img.removeAttribute("src");
    img.classList.add("img-fallback");
  });
  return img;
}
