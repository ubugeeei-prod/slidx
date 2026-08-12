/**
 * The draggable edges between the panels.
 *
 * A grip is a real element in the grid rather than a border with a cursor on
 * it, because it has to be reachable without a mouse: an author who tabs to it
 * resizes with the arrow keys, and the focus ring has somewhere to sit.
 *
 * It writes one custom property on the editor root and nothing else. The panels
 * do not know they are being resized, and nothing about a width reaches the
 * deck — this is furniture, not content.
 */

import { element } from "./dom";
import type { Surface } from "./outline";
import { resized, startingWidth, WIDTH_KEY, WIDTH_PROPERTY, type Edge } from "./panels";

const EDGES: Edge[] = ["outline", "inspector"];

/** How far an arrow key moves an edge. */
const STEP = 16;

export interface GripOptions {
  storage?: Pick<Storage, "getItem" | "setItem"> | undefined;
}

export function createGrips(options: GripOptions = {}): Surface {
  const root = element("div", { class: "slidx-grips" });
  const width: Record<Edge, number> = {
    outline: startingWidth("outline", options.storage),
    inspector: startingWidth("inspector", options.storage),
  };

  /** The editor root, which is where the grid — and so the width — lives. */
  let editor: HTMLElement | undefined;

  function apply(edge: Edge): void {
    editor?.style.setProperty(WIDTH_PROPERTY[edge], `${width[edge]}px`);
  }

  function set(edge: Edge, to: number): void {
    width[edge] = to;
    apply(edge);
    options.storage?.setItem(WIDTH_KEY[edge], String(to));
  }

  const grips = EDGES.map((edge) => {
    const grip = element("div", {
      class: "slidx-grip",
      "data-edge": edge,
      role: "separator",
      tabindex: 0,
      "aria-orientation": "vertical",
      "aria-label": edge === "outline" ? "Resize the slide list" : "Resize the inspector",
    });

    let from: number | undefined;
    let at = 0;

    grip.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      grip.setPointerCapture(event.pointerId);
      from = width[edge];
      at = event.clientX;
      grip.setAttribute("data-dragging", "true");
    });

    grip.addEventListener("pointermove", (event) => {
      if (from === undefined) return;
      // Recomputed from where the drag started rather than accumulated per
      // event: a clamped edge would otherwise drift, because every event past
      // the limit adds a delta the width did not take.
      set(edge, resized(edge, from, event.clientX - at));
    });

    const stop = (): void => {
      from = undefined;
      grip.setAttribute("data-dragging", "false");
    };

    grip.addEventListener("pointerup", stop);
    grip.addEventListener("pointercancel", stop);

    grip.addEventListener("keydown", (event) => {
      const by = event.key === "ArrowLeft" ? -STEP : event.key === "ArrowRight" ? STEP : 0;
      if (by === 0) return;

      event.preventDefault();
      // The same sign rule the pointer takes, so an arrow and a drag agree
      // about which way is wider.
      set(edge, resized(edge, width[edge], edge === "outline" ? by : -by));
    });

    return grip;
  });

  root.append(...grips);

  return {
    root,
    render() {
      if (editor) return;

      editor = root.closest<HTMLElement>(".slidx-editor") ?? undefined;
      for (const edge of EDGES) apply(edge);
    },
  };
}
