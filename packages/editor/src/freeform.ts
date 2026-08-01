/**
 * Free movement and four-edge resizing for one selected block.
 *
 * The overlay is outside the deck iframe, over rectangles read from the real
 * rendered page. A gesture paints its destination on every pointer event and
 * commits exactly one `inset` custom property when it ends. That single value
 * carries top, right, bottom and left, so diagonal movement and corner resizing
 * remain one collaboration transaction and one press of undo.
 */

import { element, fill } from "./dom";
import {
  insetOf,
  manipulateFrame,
  sameFrame,
  type FrameGuide,
  type FrameHandle,
  type ManipulatedFrame,
} from "./freeform-geometry";
import { applyFreeformStyles } from "./freeform-styles";
import { readGeometry, type Rect, type SlideGeometry } from "./geometry";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";

export interface FreeformHandlers {
  run(op: EditOp): void;
  select(block: number | undefined): void;
}

export interface FreeformOptions {
  /** The slide's boxes. Injected so every gesture is testable without layout. */
  geometry?(): SlideGeometry | undefined;
}

const HIT = 28;
/** Keeps the frame and handles clear of the selected block's content. */
const FRAME_GAP = 8;
/**
 * The move grip: wide enough to read as something to grab, narrow enough that
 * it cannot be mistaken for the frame it sits above.
 */
const MOVE_WIDTH = 44;
/** Between the grip and the band the top three resize handles occupy. */
const MOVE_GAP = 4;
const HANDLES: Exclude<FrameHandle, "move">[] = ["n", "ne", "e", "se", "s", "sw", "w", "nw"];

interface Drag {
  index: number;
  handle: FrameHandle;
  pointer: { x: number; y: number };
  from: Rect;
  shown: ManipulatedFrame;
}

export function createFreeform(handlers: FreeformHandlers, options: FreeformOptions = {}): Surface {
  const frameBox = element("div", { class: "slidx-freeform-frame" });
  const guides = element("div", { class: "slidx-freeform-guides" });
  const label = element("div", { class: "slidx-freeform-label" });
  const move = control("move", "Move selected block");
  const handles = HANDLES.map((handle) => control(handle, `Resize selected block ${handle}`));
  const status = element("div", {
    class: "slidx-freeform-status",
    role: "status",
    "aria-live": "polite",
  });
  const root = element(
    "div",
    {
      class: "slidx-freeform",
      "data-active": "false",
      "data-manipulating": "false",
      "aria-label": "Freeform block controls",
    },
    [frameBox, guides, label, move, ...handles, status],
  );

  let slide = 0;
  let selected: number | undefined;
  let frame: HTMLIFrameElement | undefined;
  let geometry: SlideGeometry | undefined;
  let drag: Drag | undefined;

  function read(): SlideGeometry | undefined {
    if (options.geometry) return options.geometry();
    return frame ? readGeometry(frame) : undefined;
  }

  function refresh(): void {
    if (!root.isConnected && options.geometry === undefined) return;
    if (drag) return;

    geometry = read();
    paintSelection();
  }

  function paintSelection(): void {
    const block = geometry?.blocks.find((candidate) => candidate.index === selected);
    root.setAttribute("data-active", String(block !== undefined));
    if (block) {
      paint(block.rect, []);
    }
  }

  function paint(rect: Rect, shownGuides: FrameGuide[]): void {
    const chrome = {
      left: rect.left - FRAME_GAP,
      top: rect.top - FRAME_GAP,
      width: rect.width + FRAME_GAP * 2,
      height: rect.height + FRAME_GAP * 2,
    };

    box(frameBox, chrome);
    // A grip above the frame, not a bar across it.
    //
    // This was the width of the whole block and straddled its top edge, which
    // put a transparent button over the first fourteen pixels of the content —
    // most of a one-line block. Every click that landed there started a drag
    // instead of reaching the text, and nothing on screen said why, because the
    // control had no mark of its own.
    //
    // So it is clear of the frame entirely, and clear of the band the `n`, `nw`
    // and `ne` handles own — half a hit target either side of the top edge —
    // with a gap so the two never round into each other.
    box(move, {
      left: chrome.left + chrome.width / 2 - MOVE_WIDTH / 2,
      top: chrome.top - HIT / 2 - HIT - MOVE_GAP,
      width: MOVE_WIDTH,
      height: HIT,
    });

    const positions: Record<Exclude<FrameHandle, "move">, { x: number; y: number }> = {
      n: { x: chrome.left + chrome.width / 2, y: chrome.top },
      ne: { x: chrome.left + chrome.width, y: chrome.top },
      e: { x: chrome.left + chrome.width, y: chrome.top + chrome.height / 2 },
      se: { x: chrome.left + chrome.width, y: chrome.top + chrome.height },
      s: { x: chrome.left + chrome.width / 2, y: chrome.top + chrome.height },
      sw: { x: chrome.left, y: chrome.top + chrome.height },
      w: { x: chrome.left, y: chrome.top + chrome.height / 2 },
      nw: { x: chrome.left, y: chrome.top },
    };

    for (const handle of handles) {
      const name = handle.getAttribute("data-handle") as Exclude<FrameHandle, "move">;
      const at = positions[name];
      box(handle, {
        left: at.x - HIT / 2,
        top: at.y - HIT / 2,
        width: HIT,
        height: HIT,
      });
    }

    fill(
      guides,
      shownGuides.map((guide) => guideLine(guide, geometry!)),
    );

    const safe = geometry?.safe;
    if (safe) {
      const width = Math.round((rect.width / safe.width) * 1000) / 10;
      const height = Math.round((rect.height / safe.height) * 1000) / 10;
      label.textContent = `${width}% × ${height}%`;
      box(label, {
        left: chrome.left,
        top: chrome.top + chrome.height + 10,
        width: 0,
        height: 0,
      });
      label.style.width = "auto";
      label.style.height = "auto";
    }
  }

  function start(control: HTMLElement, handle: FrameHandle, event: PointerEvent): void {
    geometry = read();
    const block = geometry?.blocks.find((candidate) => candidate.index === selected);
    if (!geometry || !block || selected === undefined) return;

    event.preventDefault();
    control.setPointerCapture?.(event.pointerId);
    drag = {
      index: selected,
      handle,
      pointer: { x: event.clientX, y: event.clientY },
      from: block.rect,
      shown: { rect: block.rect, guides: [] },
    };
    root.setAttribute("data-manipulating", "true");
  }

  function track(event: PointerEvent): void {
    if (!drag || !geometry) return;

    drag.shown = manipulateFrame(
      geometry,
      drag.index,
      drag.from,
      drag.handle,
      event.clientX - drag.pointer.x,
      event.clientY - drag.pointer.y,
    );
    paint(drag.shown.rect, drag.shown.guides);
  }

  function settle(): void {
    const finished = drag;
    drag = undefined;
    root.setAttribute("data-manipulating", "false");
    fill(guides, []);
    if (!finished || !geometry || sameFrame(finished.from, finished.shown.rect)) {
      paintSelection();
      return;
    }

    commit(finished.index, finished.shown.rect);
  }

  function cancel(): void {
    drag = undefined;
    root.setAttribute("data-manipulating", "false");
    fill(guides, []);
    paintSelection();
  }

  function key(handle: FrameHandle, event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      handlers.select(undefined);
      return;
    }

    if (!event.key.startsWith("Arrow") || event.repeat) return;
    geometry = read();
    const block = geometry?.blocks.find((candidate) => candidate.index === selected);
    if (!geometry || !block || selected === undefined) return;

    const by = event.shiftKey ? 10 : 1;
    const dx = event.key === "ArrowLeft" ? -by : event.key === "ArrowRight" ? by : 0;
    const dy = event.key === "ArrowUp" ? -by : event.key === "ArrowDown" ? by : 0;
    if (!movesOn(handle, dx, dy)) return;

    event.preventDefault();
    // A key is an exact nudge. Pointer snapping makes alignment effortless, but
    // applying it to an arrow would trap a frame on the guide it is leaving.
    const shown = manipulateFrame(geometry, selected, block.rect, handle, dx, dy, false);
    if (!sameFrame(block.rect, shown.rect)) commit(selected, shown.rect);
  }

  function commit(index: number, rect: Rect): void {
    const safe = geometry?.safe;
    if (!safe) return;

    handlers.run({
      op: "setBlockStyle",
      slide,
      block: index,
      property: "inset",
      value: insetOf(rect, safe),
    });
    status.textContent = `Block ${index + 1} frame is ${insetOf(rect, safe)}.`;
  }

  for (const control of [move, ...handles]) {
    const handle = control.getAttribute("data-handle") as FrameHandle;
    control.addEventListener("pointerdown", (event) => start(control, handle, event));
    control.addEventListener("pointermove", track);
    control.addEventListener("pointerup", settle);
    control.addEventListener("pointercancel", cancel);
    control.addEventListener("keydown", (event) => key(handle, event));
  }
  return {
    root,
    render(state) {
      applyFreeformStyles(root.ownerDocument);
      slide = state.selection.slide;
      selected = state.selection.block;

      if (!frame && !options.geometry) {
        frame =
          root.ownerDocument.querySelector<HTMLIFrameElement>(".slidx-canvas-frame") ?? undefined;
        frame?.addEventListener("load", refresh);
        root.ownerDocument.defaultView?.addEventListener("resize", refresh);
      }

      refresh();
    },
  };
}

function control(handle: FrameHandle, label: string): HTMLButtonElement {
  return element("button", {
    type: "button",
    class: handle === "move" ? "slidx-freeform-move" : "slidx-freeform-handle",
    "data-handle": handle,
    "aria-label": label,
    tabindex: 0,
  }) as HTMLButtonElement;
}

function movesOn(handle: FrameHandle, dx: number, dy: number): boolean {
  if (handle === "move") return dx !== 0 || dy !== 0;
  if (dx !== 0 && (handle.includes("e") || handle.includes("w"))) return true;
  return dy !== 0 && (handle.includes("n") || handle.includes("s"));
}

function guideLine(guide: FrameGuide, geometry: SlideGeometry): HTMLElement {
  return box(
    element("div", {
      class: "slidx-freeform-guide",
      "data-kind": guide.kind,
      "data-axis": guide.axis,
    }),
    guide.axis === "x"
      ? { left: guide.at, top: geometry.safe.top, width: 1, height: geometry.safe.height }
      : { left: geometry.safe.left, top: guide.at, width: geometry.safe.width, height: 1 },
  );
}

function box<T extends HTMLElement>(node: T, rect: Rect): T {
  node.style.left = `${rect.left}px`;
  node.style.top = `${rect.top}px`;
  node.style.width = `${rect.width}px`;
  node.style.height = `${rect.height}px`;
  return node;
}
