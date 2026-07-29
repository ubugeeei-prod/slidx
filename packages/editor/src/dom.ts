/**
 * The four lines of DOM plumbing every surface would otherwise repeat.
 *
 * Not a rendering library and not the beginning of one. The surfaces below
 * build elements because that is what the DOM is for; this only exists so that
 * building one does not take five statements.
 */

type Attributes = Record<string, string | number | boolean | undefined>;

export function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Attributes = {},
  children: (Node | string)[] = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);

  for (const [name, value] of Object.entries(attributes)) {
    // `true` is written out rather than left bare: `aria-current` and
    // `draggable` are enumerated attributes, and an empty value means neither
    // yes nor no to a screen reader or to a drag.
    if (value === undefined || value === false) continue;
    node.setAttribute(name, String(value));
  }

  node.append(...children);
  return node;
}

/** Replaces everything in `parent` with `children`. */
export function fill(parent: Element, children: (Node | string)[]): void {
  parent.replaceChildren(...children);
}

/** A labelled control, which is what an inspector is made of. */
export function field(label: string, control: HTMLElement): HTMLElement {
  return element("label", { class: "slidx-field" }, [
    element("span", { class: "slidx-field-name" }, [label]),
    control,
  ]);
}
