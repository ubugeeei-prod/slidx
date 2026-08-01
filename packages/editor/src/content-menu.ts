/**
 * The visual editor's first move: add common content without opening source.
 *
 * This surface only names an intention. It never carries a Markdown template;
 * `slidx_edit` owns the authored form behind each `BlockKind`. That keeps the
 * menu small, the diff reviewable, and the browser out of the writer business.
 */

import { applyContentMenuStyles } from "./content-menu-styles";
import { element } from "./dom";
import type { BlockKind } from "./operations";

interface Choice {
  kind: BlockKind;
  label: string;
  hint: string;
}

const CHOICES: Choice[] = [
  { kind: "heading", label: "Heading", hint: "Section title" },
  { kind: "text", label: "Text", hint: "Paragraph" },
  { kind: "list", label: "List", hint: "Key points" },
  { kind: "quote", label: "Quote", hint: "Takeaway" },
];

let nextMenu = 0;

export interface ContentMenu {
  root: HTMLElement;
  open(): void;
  close(): void;
  setEnabled(enabled: boolean): void;
  destroy(): void;
}

export function createContentMenu(pick: (kind: BlockKind) => void | Promise<void>): ContentMenu {
  applyContentMenuStyles(document);
  const menuId = `slidx-content-menu-${++nextMenu}`;

  const toggle = element(
    "button",
    {
      type: "button",
      class: "slidx-content-toggle",
      "aria-label": "Add content",
      "aria-expanded": false,
      "aria-controls": menuId,
      "aria-haspopup": "menu",
      title: "Add content (A)",
    },
    ["Add"],
  );
  const items = CHOICES.map(({ kind, label, hint }) => {
    const item = element(
      "button",
      {
        type: "button",
        role: "menuitem",
        tabindex: -1,
        class: "slidx-content-item",
        "data-kind": kind,
      },
      [
        element("span", { class: "slidx-content-label" }, [label]),
        element("span", { class: "slidx-content-hint" }, [hint]),
      ],
    );
    item.addEventListener("click", () => {
      close(false);
      void pick(kind);
    });
    return item;
  });
  const menu = element(
    "div",
    { id: menuId, class: "slidx-content-menu", role: "menu", "aria-label": "Add content" },
    items,
  );
  menu.hidden = true;
  const root = element("div", { class: "slidx-content" }, [toggle, menu]);

  /** Light-dismiss while the menu is open, and only while it is open. */
  const outside = (event: PointerEvent) => {
    if (event.target instanceof Node && !root.contains(event.target)) close(false);
  };

  function open(): void {
    if (toggle.disabled || !menu.hidden) return;
    menu.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    document.addEventListener("pointerdown", outside);
    items[0]?.focus();
  }

  function close(restore = false): void {
    if (menu.hidden) return;
    menu.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    document.removeEventListener("pointerdown", outside);
    if (restore) toggle.focus();
  }

  toggle.addEventListener("click", () => (menu.hidden ? open() : close(true)));
  menu.addEventListener("keydown", (event) => {
    const key = (event as KeyboardEvent).key;
    if (key === "Escape") {
      event.preventDefault();
      close(true);
      return;
    }

    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(key)) return;
    event.preventDefault();
    const active = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      key === "Home"
        ? 0
        : key === "End"
          ? items.length - 1
          : (Math.max(active, 0) + (key === "ArrowDown" ? 1 : items.length - 1)) % items.length;
    items[next]?.focus();
  });

  return {
    root,
    open,
    close: () => close(false),
    setEnabled(enabled) {
      toggle.disabled = !enabled;
      if (!enabled) close(false);
    },
    destroy() {
      close(false);
    },
  };
}
