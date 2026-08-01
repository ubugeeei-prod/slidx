/**
 * The product signature and the deck currently open in it.
 *
 * Panel headings answer where a control lives. This answers the wider question
 * they cannot: which product and which deck the author is looking at. It takes
 * one shared row rather than branding every panel, so the identity is present
 * without competing with the slide.
 *
 * The mark uses the same 24-unit geometry as `assets/brand/mark-*.svg`: one
 * document, a one-module gutter, and three pages. Its fills are editor tokens so
 * it resolves with the chrome's scheme without shipping a second image asset.
 */

import { element } from "./dom";
import { applyAppbarStyles } from "./appbar-styles";
import type { Surface } from "./outline";
import type { EditorState } from "./session";

const SVG = "http://www.w3.org/2000/svg";

/** The brand mark, at the geometry published by `slidx_brand`. */
function mark(): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", "slidx-appbar-mark");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");

  for (const [x, y, width, height, part] of [
    [0, 0, 9, 24, "document"],
    [12, 0, 12, 6, "page"],
    [12, 9, 12, 6, "page"],
    [12, 18, 12, 6, "page"],
  ] as const) {
    const rect = document.createElementNS(SVG, "rect");
    rect.setAttribute("x", String(x));
    rect.setAttribute("y", String(y));
    rect.setAttribute("width", String(width));
    rect.setAttribute("height", String(height));
    rect.setAttribute("class", `slidx-appbar-mark-${part}`);
    svg.append(rect);
  }

  return svg;
}

export interface AppbarHandlers {
  undo(): void;
  redo(): void;
  present(): void;
  audience(): void;
  print(): void;
}

export interface AppbarOptions {
  /** Compact global surfaces, such as search and the people currently editing. */
  accessories?: readonly HTMLElement[];
}

/** One quiet row of product identity, document context, and primary commands. */
export function createAppbar(handlers: AppbarHandlers, options: AppbarOptions = {}): Surface {
  const title = element("strong", { class: "slidx-appbar-title" });
  const position = element("span", { class: "slidx-appbar-position", "aria-live": "polite" });
  const status = element("span", {
    class: "slidx-appbar-status",
    role: "status",
    "aria-live": "polite",
  });
  const undo = command("Undo", "⌘/Ctrl Z", "↶", () => handlers.undo());
  const redo = command("Redo", "Shift ⌘/Ctrl Z", "↷", () => handlers.redo());
  const present = element(
    "button",
    {
      type: "button",
      class: "slidx-appbar-present",
      title: "Open presenter view (P)",
      "aria-label": "Open presenter view",
    },
    [
      element("span", { "aria-hidden": "true" }, ["▶"]),
      element("span", { class: "slidx-appbar-present-label" }, ["Present"]),
    ],
  ) as HTMLButtonElement;
  const audience = deliveryOption("▣", "Audience view", "Selected slide, without editor chrome");
  const print = deliveryOption("⇩", "Print / PDF", "Every stop in one printable document");
  const menuId = `slidx-delivery-menu-${++nextDeliveryMenu}`;
  const menu = element(
    "div",
    {
      id: menuId,
      class: "slidx-appbar-delivery-menu",
      role: "menu",
      "aria-label": "Presentation outputs",
      hidden: true,
    },
    [audience, print],
  );
  const deliveryToggle = element(
    "button",
    {
      type: "button",
      class: "slidx-appbar-delivery-toggle",
      title: "More presentation outputs",
      "aria-label": "More presentation outputs",
      "aria-haspopup": "menu",
      "aria-expanded": "false",
      "aria-controls": menuId,
    },
    [element("span", { "aria-hidden": "true" }, ["▾"])],
  ) as HTMLButtonElement;
  const delivery = element("div", { class: "slidx-appbar-delivery" }, [
    present,
    deliveryToggle,
    menu,
  ]);

  function openDelivery(): void {
    menu.hidden = false;
    deliveryToggle.setAttribute("aria-expanded", "true");
    audience.focus();
  }

  function closeDelivery(restore = false): void {
    if (menu.hidden) return;
    menu.hidden = true;
    deliveryToggle.setAttribute("aria-expanded", "false");
    if (restore) deliveryToggle.focus();
  }

  function runDelivery(action: () => void): void {
    closeDelivery();
    action();
  }

  present.addEventListener("click", () => runDelivery(() => handlers.present()));
  audience.addEventListener("click", () => runDelivery(() => handlers.audience()));
  print.addEventListener("click", () => runDelivery(() => handlers.print()));
  deliveryToggle.addEventListener("click", () => {
    if (menu.hidden) openDelivery();
    else closeDelivery(true);
  });
  menu.addEventListener("keydown", (event) =>
    deliveryKeydown(event, [audience, print], closeDelivery),
  );
  const root = element("header", { class: "slidx-appbar", "aria-label": "slidx editor" }, [
    element("div", { class: "slidx-appbar-lockup", "aria-label": "slidx" }, [
      mark(),
      element("span", { class: "slidx-appbar-wordmark" }, ["slidx"]),
    ]),
    element("div", { class: "slidx-appbar-context" }, [title, position]),
    element("div", { class: "slidx-appbar-commands", "aria-label": "Deck commands" }, [
      ...(options.accessories ?? []),
      status,
      element("span", { class: "slidx-appbar-command-rule", "aria-hidden": "true" }),
      undo,
      redo,
      delivery,
    ]),
  ]);
  applyAppbarStyles(root.ownerDocument);

  const dismiss = (event: PointerEvent) => {
    if (event.target instanceof Node && !delivery.contains(event.target)) closeDelivery();
  };
  root.ownerDocument.addEventListener("pointerdown", dismiss);

  return {
    root,
    render(state: EditorState) {
      const total = state.slides.length;
      const current = total === 0 ? 0 : Math.min(state.selection.slide + 1, total);

      const deckTitle = state.title?.trim() || "Untitled deck";
      title.textContent = deckTitle;
      root.ownerDocument.title = `${deckTitle} — editor`;
      position.textContent = `${current} / ${total}`;
      position.setAttribute("aria-label", `Slide ${current} of ${total}`);

      const saved = saveState(state);
      status.textContent = saved.label;
      status.setAttribute("data-state", saved.state);
      status.title = saved.detail;

      undo.disabled = state.writing || !state.canUndo || state.canEdit === false;
      redo.disabled = state.writing || !state.canRedo || state.canEdit === false;
      present.disabled = total === 0;
      deliveryToggle.disabled = total === 0;
      audience.disabled = total === 0;
      print.disabled = total === 0;
      if (total === 0) closeDelivery();
    },
    destroy() {
      root.ownerDocument.removeEventListener("pointerdown", dismiss);
    },
  };
}

let nextDeliveryMenu = 0;

function deliveryOption(mark: string, label: string, hint: string): HTMLButtonElement {
  return element(
    "button",
    { type: "button", class: "slidx-appbar-delivery-option", role: "menuitem", tabindex: -1 },
    [
      element("span", { class: "slidx-appbar-delivery-mark", "aria-hidden": "true" }, [mark]),
      element("span", { class: "slidx-appbar-delivery-copy" }, [
        element("span", { class: "slidx-appbar-delivery-label" }, [label]),
        element("span", { class: "slidx-appbar-delivery-hint" }, [hint]),
      ]),
    ],
  ) as HTMLButtonElement;
}

function deliveryKeydown(
  event: KeyboardEvent,
  options: readonly HTMLButtonElement[],
  close: (restore?: boolean) => void,
): void {
  const at = options.indexOf(event.target as HTMLButtonElement);

  if (event.key === "Escape") {
    event.preventDefault();
    close(true);
    return;
  }

  const next =
    event.key === "ArrowDown"
      ? (at + 1) % options.length
      : event.key === "ArrowUp"
        ? (at - 1 + options.length) % options.length
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? options.length - 1
            : undefined;
  if (next === undefined) return;

  event.preventDefault();
  options[next]?.focus();
}

function command(
  label: string,
  shortcut: string,
  glyph: string,
  act: () => void,
): HTMLButtonElement {
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-appbar-command",
      title: `${label} (${shortcut})`,
      "aria-label": label,
    },
    [element("span", { "aria-hidden": "true" }, [glyph])],
  ) as HTMLButtonElement;
  button.addEventListener("click", act);
  return button;
}

function saveState(state: EditorState): { state: string; label: string; detail: string } {
  if (state.writing) {
    return { state: "writing", label: "Saving…", detail: "Writing this change to the deck." };
  }
  if (state.problem) {
    const unopened = state.source.length === 0 && state.slides.length === 0;
    return {
      state: "error",
      label: unopened ? "Open failed" : "Write stopped",
      detail: state.problem,
    };
  }
  if (state.canEdit === false) {
    return {
      state: "readonly",
      label: "View only",
      detail: "This link can read the deck but cannot change its files.",
    };
  }
  if (state.refusal) {
    return { state: "warning", label: "Not applied", detail: state.refusal.error };
  }
  if (state.source.length === 0 && state.slides.length === 0) {
    return { state: "opening", label: "Opening…", detail: "Reading the deck from disk." };
  }

  return { state: "saved", label: "Saved", detail: "The editor matches the deck on disk." };
}
