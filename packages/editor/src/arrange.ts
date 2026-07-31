/**
 * Direct manipulation: dragging a block into a region, and reordering inside
 * one.
 *
 * The gesture is drawn entirely in the editor's own document, at coordinates
 * read from inside the canvas frame. Nothing is added to the deck page, which
 * is the only way the canvas keeps being the page the build emits.
 *
 * # One gesture, one operation, one press of undo
 *
 * A drag ends in exactly one [`moveBlock`](./operations), whether it changed a
 * region, a position, or both. A drag that ends where it started plans nothing
 * at all — `slidx_edit` drops a replacement that matches what it would replace,
 * and the undo stack ignores an empty inverse — so putting a block back costs
 * nothing and undo still steps through the changes an author actually made.
 *
 * # Every gesture has a key
 *
 * The grips are buttons, so the tab order walks the blocks of the slide and the
 * arrow keys move one: up and down through its region, left and right between
 * regions. A deck that can only be arranged with a pointer is a deck half the
 * people who write one cannot arrange at all.
 *
 * # Warning before the block lands
 *
 * This is what a region buys that a freeform canvas cannot. A region is its own
 * box, the same WebAssembly module the build uses is one call away, and the
 * measurements this can take exactly — [`arrival`](./placement) — go to the
 * same Rust rule. So a code block being dragged into a column too narrow for it
 * says so while it is still moving, in the sentence a build would have used.
 */

import { applyArrangeStyles } from "./arrange-styles";
import type { Finding } from "./client";
import { element, fill } from "./dom";
import { readGeometry, type Rect, type SlideGeometry } from "./geometry";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import {
  arrival,
  guides,
  landing,
  nudge,
  type Guide,
  type Landing,
  type Measurement,
  type Nudge,
} from "./placement";

export interface ArrangeHandlers {
  run(op: EditOp): void;
  /** What the linter says about a landing that has not happened yet. */
  foresee(findings: Finding[]): void;
}

export interface ArrangeOptions {
  /** The slide's boxes. Injected so the gesture is drivable without a layout. */
  geometry?(): SlideGeometry | undefined;
  /** Asks the pipeline what a measurement means. */
  measure?(measured: Measurement[]): Promise<Finding[]>;
}

/**
 * The chrome's hit size, which is what a grip is.
 *
 * Repeated here because the rectangle is computed in script and the stylesheet
 * cannot place a fixed element over a box only the canvas knows about. The
 * stylesheet spends the token; a test holds the two together.
 */
const HIT = 28;

/** Which way each arrow key sends a block. */
const KEYS: Record<string, Nudge> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "before",
  ArrowRight: "after",
};

export function createArrange(handlers: ArrangeHandlers, options: ArrangeOptions = {}): Surface {
  const safe = element("div", { class: "slidx-arrange-safe" });
  const drop = element("div", { class: "slidx-arrange-drop" });
  const ghost = element("div", { class: "slidx-arrange-ghost" });
  const areas = element("div", { class: "slidx-arrange-areas" });
  const grips = element("div", { class: "slidx-arrange-grips" });
  const lines = element("div", { class: "slidx-arrange-guides" });
  const status = element("div", {
    class: "slidx-arrange-status",
    role: "status",
    "aria-live": "polite",
  });

  const root = element(
    "div",
    { class: "slidx-arrange", "data-dragging": "false", "aria-label": "Arrange blocks" },
    [safe, areas, lines, drop, ghost, grips, status],
  );

  let slide = 0;
  let frame: HTMLIFrameElement | undefined;
  let geometry: SlideGeometry | undefined;
  let moving: { index: number; target?: Landing | undefined } | undefined;
  /** The region the linter was last asked about, so a drag asks once per one. */
  let asked: string | undefined;
  /** The block whose grip should hold focus once the canvas comes back. */
  let holding: number | undefined;

  function read(): SlideGeometry | undefined {
    if (options.geometry) return options.geometry();

    return frame ? readGeometry(frame) : undefined;
  }

  function refresh(): void {
    // A destroyed editor leaves this listening on a window that outlives it.
    if (!root.isConnected && options.geometry === undefined) return;

    // A gesture owns the overlay while it is running. `paint` rebuilds every
    // grip, and a browser sends the rest of a drag to the element that took the
    // pointer capture — so replacing that element mid-drag ends the gesture with
    // the ghost still on screen and nothing written.
    //
    // This is reached mid-drag by the feature next to it: a warning about the
    // landing is session state, so the answer to a measurement re-renders every
    // surface, including this one. Nothing is lost by declining, because the
    // canvas cannot have re-laid out while a pointer is down on it.
    if (moving !== undefined) return;

    geometry = read();
    paint();
  }

  function paint(): void {
    // Rebuilding the grips takes the focused one away with it, and a canvas
    // that reloads after every move would end an author's second arrow press
    // on the document body. The block wanted after a move is the one it landed
    // on; otherwise it is whichever was focused before.
    const wanted = holding ?? focused();
    holding = undefined;

    if (geometry === undefined) {
      hide(safe);
      fill(areas, []);
      fill(grips, []);
      return;
    }

    box(safe, geometry.safe);
    fill(
      areas,
      geometry.regions.map((region) =>
        box(
          element("div", { class: "slidx-arrange-region", "data-region": region.name }, [
            element("span", { class: "slidx-arrange-name" }, [region.name]),
          ]),
          region.rect,
        ),
      ),
    );

    fill(
      grips,
      geometry.blocks.map((block) => grip(block.index, block.region, block.rect)),
    );

    if (wanted !== undefined) focus(wanted);
  }

  /** The block whose grip has focus, if one does. */
  function focused(): number | undefined {
    const active = root.ownerDocument.activeElement;
    if (!(active instanceof HTMLElement) || !grips.contains(active)) return undefined;

    const at = Number(active.getAttribute("data-block"));
    return Number.isInteger(at) ? at : undefined;
  }

  function grip(index: number, region: string, rect: Rect): HTMLElement {
    const handle = element(
      "button",
      {
        type: "button",
        class: "slidx-arrange-grip",
        // The chrome's focus ring is written against `[tabindex]`, and a button
        // matches that selector only when it carries the attribute.
        tabindex: 0,
        "data-block": index,
        // Not `aria-pressed`: a drag is not a toggle, and a button that gains
        // the attribute halfway through a gesture changes what it is to a
        // screen reader. What is happening is said in words, in the live
        // region, which is where it belongs.
        "data-moving": false,
        "aria-label": `Move block ${index + 1}, in ${region}`,
      },
      [element("span", {}, ["≡"])],
    );

    // Straddling the corner rather than inside it: a target over the first word
    // of a heading is a target that stops the heading being clicked, and a
    // heading is edited in place.
    const half = HIT / 2;
    box(handle, { left: rect.left - half, top: rect.top - half, width: HIT, height: HIT });

    handle.addEventListener("pointerdown", (event) => start(handle, index, event));
    handle.addEventListener("pointermove", (event) => track(event));
    handle.addEventListener("pointerup", () => land());
    handle.addEventListener("pointercancel", () => stop());
    handle.addEventListener("keydown", (event) => key(index, event));

    return handle;
  }

  function start(handle: HTMLElement, index: number, event: PointerEvent): void {
    // The canvas may have re-laid out since the last render, and a drag against
    // stale rectangles drops the block somewhere the author did not point at.
    geometry = read();
    if (geometry === undefined) return;

    event.preventDefault();
    handle.setPointerCapture?.(event.pointerId);
    handle.setAttribute("data-moving", "true");

    moving = { index };
    asked = undefined;
    root.setAttribute("data-dragging", "true");
    track(event);
  }

  function track(event: PointerEvent): void {
    if (moving === undefined || geometry === undefined) return;

    const target = landing(geometry, moving.index, event.clientX, event.clientY);
    moving.target = target;
    show(moving.index, target);
    foresee(moving.index, target);
  }

  /** Draws the landing: where the block goes, and what it is lining up with. */
  function show(index: number, target: Landing | undefined): void {
    const block = geometry?.blocks.find((candidate) => candidate.index === index);

    for (const area of areas.querySelectorAll(".slidx-arrange-region")) {
      area.setAttribute("data-over", String(area.getAttribute("data-region") === target?.region));
    }

    if (geometry === undefined || target === undefined || block === undefined) {
      hide(drop);
      hide(ghost);
      fill(lines, []);
      return;
    }

    box(drop, { ...target.rect, height: 2 });

    // The ghost is at the landing rather than under the cursor: the file will
    // say a region and a position, so that is what the gesture should show.
    const settled = { ...target.rect, height: block.rect.height };
    box(ghost, settled);
    fill(
      lines,
      guides(geometry, index, settled).map((guide) => line(guide, geometry!)),
    );
  }

  /**
   * Asks the linter what this landing would cost, once per region entered.
   *
   * Per region rather than per pointer event: the answer cannot change while
   * the cursor moves inside one box, and a question per frame would be a
   * question per frame.
   */
  function foresee(index: number, target: Landing | undefined): void {
    const key = target?.region;
    if (key === asked) return;
    asked = key;

    const measured =
      geometry !== undefined && target !== undefined ? arrival(geometry, index, target, slide) : [];

    if (measured.length === 0 || options.measure === undefined) {
      handlers.foresee([]);
      return;
    }

    void options
      .measure(measured)
      .then((findings) => {
        // The pointer may have left the region while the answer was in flight,
        // and a warning about a box the block is no longer over is worse than
        // none: it names the wrong region.
        if (asked === key) handlers.foresee(findings);
      })
      .catch(() => handlers.foresee([]));
  }

  function land(): void {
    const target = moving?.target;
    const index = moving?.index;
    stop();

    if (target === undefined || index === undefined) return;
    commit(index, target);
  }

  function stop(): void {
    moving = undefined;
    asked = undefined;
    root.setAttribute("data-dragging", "false");
    hide(drop);
    hide(ghost);
    fill(lines, []);
    handlers.foresee([]);

    for (const handle of grips.querySelectorAll(".slidx-arrange-grip")) {
      handle.setAttribute("data-moving", "false");
    }
  }

  function key(index: number, event: KeyboardEvent): void {
    const direction = KEYS[event.key];
    if (direction === undefined || geometry === undefined) return;

    const target = nudge(geometry, index, direction);
    event.preventDefault();
    if (target === undefined) return;

    commit(index, target);
  }

  function commit(index: number, target: Landing): void {
    // Always with a region, even when it is the one the block is already in:
    // that is what lets a stale class from another layout be tidied up by the
    // gesture that moves the block, and it costs nothing when nothing changed.
    handlers.run({
      op: "moveBlock",
      slide,
      block: index,
      to: target.to,
      region: target.region,
    });

    // The canvas reloads under this, so the grip that had focus is about to be
    // replaced by one for the block in its new position.
    holding = target.to;
    status.textContent = `Block ${index + 1} moved to ${target.region}, position ${target.at + 1}.`;
  }

  function focus(index: number): void {
    grips.querySelector<HTMLElement>(`.slidx-arrange-grip[data-block="${index}"]`)?.focus();
  }

  return {
    root,
    render(state) {
      applyArrangeStyles(root.ownerDocument);
      slide = state.selection.slide;
      root.setAttribute("data-freeform-selection", String(state.selection.block !== undefined));

      if (frame === undefined) {
        const found = root.ownerDocument.querySelector<HTMLIFrameElement>(".slidx-canvas-frame");
        if (found) {
          frame = found;
          found.addEventListener("load", refresh);
          root.ownerDocument.defaultView?.addEventListener("resize", refresh);
          // Switching between the slide and its Markdown is not a change to the
          // deck, so nothing else here would hear about it.
          root.ownerDocument
            .querySelector(".slidx-canvas-toggle")
            ?.addEventListener("click", refresh);
        }
      }

      refresh();
    },
  };
}

/** One alignment line, drawn the length of the slide it belongs to. */
function line(guide: Guide, geometry: SlideGeometry): HTMLElement {
  const rect =
    guide.axis === "x"
      ? { left: guide.at, top: geometry.slide.top, width: 1, height: geometry.slide.height }
      : { left: geometry.slide.left, top: guide.at, width: geometry.slide.width, height: 1 };

  return box(
    element("div", {
      class: "slidx-arrange-guide",
      "data-kind": guide.kind,
      "data-axis": guide.axis,
    }),
    rect,
  );
}

function box<T extends HTMLElement>(node: T, rect: Rect): T {
  node.style.left = `${rect.left}px`;
  node.style.top = `${rect.top}px`;
  node.style.width = `${rect.width}px`;
  node.style.height = `${rect.height}px`;

  return node;
}

function hide(node: HTMLElement): void {
  node.style.width = "0px";
  node.style.height = "0px";
}
