/**
 * Direct manipulation: making a block take less of its region.
 *
 * The other half of "where is this and how big is it". [`arrange`](./arrange)
 * moves a block between regions and within one; this changes how much of the
 * region it takes, and it snaps to the shares the theme names while the pointer
 * is still moving — so what is under the cursor is what the file will say.
 *
 * # One gesture, one operation, one press of undo
 *
 * A drag ends in exactly one [`setBlockWidth`](./operations). The default share
 * is written by *removing* the property, so dragging a block narrower and back
 * again is byte-identical — the same property the region class already has, and
 * the reason `slidx_edit` owns the rule rather than this file.
 *
 * # Why there is no handle to make a block wider than its region
 *
 * Because that is not a size. A block wider than its region is a block in
 * another region, or a slide with another layout, and both are already sayable —
 * [`arrange`](./arrange) does the first and `layout:` the second. A handle that
 * wrote a span across the layout's columns would give one block two answers
 * about where it is, and the reasoning is in `slidx_theme::layout::width`.
 *
 * # Warning before the block is narrowed
 *
 * The share is a fraction of a region that is itself a fraction of the slide, so
 * the box the block is heading for is a number both sides can compute. A code
 * block dragged narrower than its own content says so while it is still moving,
 * in the sentence a build would have used. A pixel width could not have done
 * that: 340 of what?
 */

import type { Finding } from "./client";
import { element, fill } from "./dom";
import { readGeometry, type Rect, type SlideGeometry } from "./geometry";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import { applyResizeStyles } from "./resize-styles";
import { boxAt, narrowing, shareAt, stepped, widthOf, type BlockWidth, type Step } from "./widths";
import type { Measurement } from "./placement";

export interface ResizeHandlers {
  run(op: EditOp): void;
  /** What the linter says about a share the block does not have yet. */
  foresee(findings: Finding[]): void;
}

export interface ResizeOptions {
  /** The slide's boxes. Injected so the gesture is drivable without a layout. */
  geometry?(): SlideGeometry | undefined;
  /** Asks the pipeline what a measurement means. */
  measure?(measured: Measurement[]): Promise<Finding[]>;
}

/**
 * The handle's hit size.
 *
 * The chrome's target size, repeated here because the rectangle is computed in
 * script and no stylesheet can place a fixed element over a box only the canvas
 * knows about. The stylesheet spends the token; a test holds the two together.
 */
const HIT = 28;

/** Which way each arrow key sends a handle. */
const KEYS: Record<string, Step> = { ArrowLeft: "narrower", ArrowRight: "wider" };

export function createResize(handlers: ResizeHandlers, options: ResizeOptions = {}): Surface {
  const ghost = element("div", { class: "slidx-resize-ghost" });
  const name = element("div", { class: "slidx-resize-name" });
  const grips = element("div", { class: "slidx-resize-grips" });
  const status = element("div", {
    class: "slidx-resize-status",
    role: "status",
    "aria-live": "polite",
  });

  const root = element(
    "div",
    { class: "slidx-resize", "data-resizing": "false", "aria-label": "Resize blocks" },
    [ghost, name, grips, status],
  );

  let slide = 0;
  let frame: HTMLIFrameElement | undefined;
  let geometry: SlideGeometry | undefined;
  let sizing: { index: number; to: BlockWidth } | undefined;
  /** The share the linter was last asked about, so a drag asks once per one. */
  let asked: string | undefined;

  function read(): SlideGeometry | undefined {
    if (options.geometry) return options.geometry();

    return frame ? readGeometry(frame) : undefined;
  }

  function refresh(): void {
    // A destroyed editor leaves this listening on a window that outlives it.
    if (!root.isConnected && options.geometry === undefined) return;

    geometry = read();
    paint();
  }

  function paint(): void {
    if (geometry === undefined) {
      fill(grips, []);
      return;
    }

    fill(
      grips,
      geometry.blocks.flatMap((block) => {
        const region = geometry!.regions.find((candidate) => candidate.name === block.region);
        // A block in a region with no width to share is a slide that has not
        // finished laying out, and a handle on it would resize against zero.
        return region && region.rect.width > 0 ? [grip(block.index, block.rect)] : [];
      }),
    );
  }

  function grip(index: number, rect: Rect): HTMLElement {
    const handle = element(
      "button",
      {
        type: "button",
        class: "slidx-resize-grip",
        // The chrome's focus ring is written against `[tabindex]`, and a button
        // matches that selector only when it carries the attribute.
        tabindex: 0,
        "data-block": index,
        "data-moving": false,
        "aria-label": `Resize block ${index + 1}`,
      },
      [],
    );

    // Straddling the trailing edge, so the handle is not over the last word of
    // a line — a line of the slide is edited in place, and a target on top of it
    // is a line that cannot be clicked.
    box(handle, {
      left: rect.left + rect.width - HIT / 2,
      top: rect.top + rect.height / 2 - HIT / 2,
      width: HIT,
      height: HIT,
    });

    handle.addEventListener("pointerdown", (event) => start(handle, index, event));
    handle.addEventListener("pointermove", (event) => track(event));
    handle.addEventListener("pointerup", () => settle());
    handle.addEventListener("pointercancel", () => stop());
    handle.addEventListener("keydown", (event) => key(index, event));

    return handle;
  }

  function start(handle: HTMLElement, index: number, event: PointerEvent): void {
    // The canvas may have re-laid out since the last render, and a drag against
    // stale rectangles snaps to a share the author did not point at.
    geometry = read();
    if (geometry === undefined) return;

    event.preventDefault();
    handle.setPointerCapture?.(event.pointerId);
    handle.setAttribute("data-moving", "true");

    sizing = { index, to: shareOfBlock(index) };
    asked = undefined;
    root.setAttribute("data-resizing", "true");
    track(event);
  }

  function track(event: PointerEvent): void {
    const region = regionOf(sizing?.index);
    if (sizing === undefined || region === undefined) return;

    sizing.to = shareAt(region, event.clientX);
    show(sizing.index, sizing.to);
    foresee(sizing.index, sizing.to);
  }

  /** Draws the share the file will say, snapped. */
  function show(index: number, to: BlockWidth): void {
    const block = geometry?.blocks.find((candidate) => candidate.index === index);
    const region = regionOf(index);
    if (block === undefined || region === undefined) return;

    const settled = boxAt(region, block, to);
    box(ghost, settled);
    name.textContent = to;
    box(name, {
      left: settled.left + settled.width + HIT / 4,
      top: settled.top,
      width: 0,
      height: 0,
    });
    name.style.width = "auto";
    name.style.height = "auto";
  }

  /**
   * Asks the linter what this share would cost, once per share.
   *
   * Per share rather than per pointer event: the answer cannot change while the
   * cursor moves within one snap target, and a question per frame would be a
   * question per frame.
   */
  function foresee(index: number, to: BlockWidth): void {
    if (to === asked) return;
    asked = to;

    const block = geometry?.blocks.find((candidate) => candidate.index === index);
    const region = regionOf(index);
    const measured = block && region ? narrowing(region, block, to, slide) : ([] as Measurement[]);

    if (measured.length === 0 || options.measure === undefined) {
      handlers.foresee([]);
      return;
    }

    void options
      .measure(measured)
      .then((findings) => {
        // The pointer may have moved to another share while the answer was in
        // flight, and a warning about a box the block is no longer heading for
        // is worse than none.
        if (asked === to) handlers.foresee(findings);
      })
      .catch(() => handlers.foresee([]));
  }

  function settle(): void {
    const to = sizing?.to;
    const index = sizing?.index;
    stop();

    if (to === undefined || index === undefined) return;
    commit(index, to);
  }

  function stop(): void {
    sizing = undefined;
    asked = undefined;
    root.setAttribute("data-resizing", "false");
    handlers.foresee([]);

    for (const handle of grips.querySelectorAll(".slidx-resize-grip")) {
      handle.setAttribute("data-moving", "false");
    }
  }

  function key(index: number, event: KeyboardEvent): void {
    const step = KEYS[event.key];
    if (step === undefined || geometry === undefined) return;

    event.preventDefault();
    const to = stepped(shareOfBlock(index), step);
    if (to === undefined) return;

    commit(index, to);
  }

  function commit(index: number, to: BlockWidth): void {
    handlers.run({ op: "setBlockWidth", slide, block: index, width: to });
    status.textContent = `Block ${index + 1} takes ${to} of its region.`;
  }

  /** The share a block currently says it takes. */
  function shareOfBlock(index: number): BlockWidth {
    const block = geometry?.blocks.find((candidate) => candidate.index === index);

    return block ? widthOf(block) : "full";
  }

  function regionOf(index: number | undefined) {
    const block = geometry?.blocks.find((candidate) => candidate.index === index);

    return geometry?.regions.find((candidate) => candidate.name === block?.region);
  }

  return {
    root,
    render(state) {
      applyResizeStyles(root.ownerDocument);
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

function box<T extends HTMLElement>(node: T, rect: Rect): T {
  node.style.left = `${rect.left}px`;
  node.style.top = `${rect.top}px`;
  node.style.width = `${rect.width}px`;
  node.style.height = `${rect.height}px`;

  return node;
}
