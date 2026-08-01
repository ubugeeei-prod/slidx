/**
 * Narrative starting points for a new slide.
 *
 * The miniatures communicate composition, while the operation carries only a
 * kind. Source, layout declarations, region attributes, and placeholder copy
 * all remain owned by the Rust writer.
 */

import { element } from "./dom";
import type { SlideKind } from "./operations";
import { applySlideMenuStyles } from "./slide-menu-styles";

interface Choice {
  kind: SlideKind;
  label: string;
  hint: string;
  parts: string[];
}

const CHOICES: Choice[] = [
  {
    kind: "title-body",
    label: "Title + body",
    hint: "Lead, then explain",
    parts: ["title", "body"],
  },
  {
    kind: "statement",
    label: "Statement",
    hint: "One idea, full frame",
    parts: ["statement"],
  },
  {
    kind: "comparison",
    label: "Comparison",
    hint: "Two equal sides",
    parts: ["left", "right"],
  },
  {
    kind: "points",
    label: "Key points",
    hint: "A title and list",
    parts: ["title", "point", "point", "point"],
  },
];

let nextMenu = 0;

export interface SlideMenu {
  root: HTMLElement;
  open(): void;
  close(): void;
  destroy(): void;
}

export function createSlideMenu(pick: (kind: SlideKind) => void | Promise<void>): SlideMenu {
  applySlideMenuStyles(document);
  const menuId = `slidx-slide-menu-${++nextMenu}`;
  const toggle = element(
    "button",
    {
      type: "button",
      class: "slidx-slide-add-toggle",
      "aria-label": "Add slide",
      "aria-expanded": false,
      "aria-controls": menuId,
      "aria-haspopup": "menu",
      title: "Choose a starting point",
    },
    ["Add slide"],
  );
  const items = CHOICES.map(({ kind, label, hint, parts }) => {
    const preview = element(
      "span",
      { class: "slidx-slide-choice-preview", "aria-hidden": "true" },
      parts.map((part) => element("i", { "data-part": part })),
    );
    const item = element(
      "button",
      {
        type: "button",
        role: "menuitem",
        tabindex: -1,
        class: "slidx-slide-choice",
        "data-slide-kind": kind,
      },
      [
        preview,
        element("span", { class: "slidx-slide-choice-copy" }, [
          element("strong", {}, [label]),
          element("span", {}, [hint]),
        ]),
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
    { id: menuId, class: "slidx-slide-menu", role: "menu", "aria-label": "New slide" },
    items,
  );
  menu.hidden = true;
  const root = element("div", { class: "slidx-slide-add" }, [toggle, menu]);

  const outside = (event: PointerEvent) => {
    if (event.target instanceof Node && !root.contains(event.target)) close(false);
  };

  function open(): void {
    if (!menu.hidden) return;
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
    destroy: () => close(false),
  };
}
