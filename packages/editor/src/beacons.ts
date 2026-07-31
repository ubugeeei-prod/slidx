/**
 * Where the other people are, drawn on the block they are actually in.
 *
 * The roster in `collab.ts` says a co-presenter is on slide four. That is
 * enough to stop two people opening the same slide by accident and not enough
 * to stop them rewriting the same paragraph, which is the collision that costs
 * somebody their sentence. So the same roster is drawn a second time here: an
 * outline around the block each remote editor has selected, with their name on
 * it, over the slide the author is looking at.
 *
 * # It draws only what it was told
 *
 * A viewer with no block selected gets no mark, and neither does one whose
 * block is not on this slide — which happens for a second or two whenever
 * somebody moves, because their position and the deck they are positioned in
 * arrive on the same stream but not in the same frame. Guessing a rectangle for
 * them would put another person's name on a paragraph they have never opened,
 * and a marker that is wrong occasionally is one an author learns to ignore.
 *
 * # The colour survives somebody leaving
 *
 * It comes from a hash of the seat id rather than from the viewer's place in
 * the roster, so a guest keeps the colour they had when another guest closes
 * their tab. Two guests can land on the same hue, which the name beside the
 * mark is there to settle.
 *
 * Outside the canvas iframe, like every other overlay: the moment the editor
 * puts an element into the deck's page, the preview stops being the page the
 * build emits.
 */

import { applyBeaconStyles } from "./beacon-styles";
import type { Viewer } from "./collab";
import { element, fill } from "./dom";
import { readGeometry, type Rect, type SlideGeometry } from "./geometry";
import type { Surface } from "./outline";
import type { EditorState } from "./session";

/**
 * The hues a mark can take.
 *
 * Held at one lightness and one chroma so no viewer is quieter than another,
 * and dark enough that the name sits on them in white at a readable contrast.
 * Five rather than more because the point of the colour is to tell two marks
 * apart on one slide, and a slide with five people editing different blocks of
 * it has a problem no palette solves.
 */
export const BEACON_INKS = [
  "oklch(52% 0.17 25)",
  "oklch(52% 0.17 145)",
  "oklch(52% 0.17 250)",
  "oklch(52% 0.17 300)",
  "oklch(52% 0.17 75)",
];

export interface BeaconOptions {
  /** The slide's boxes. Injected so the drawing is testable without layout. */
  geometry?(): SlideGeometry | undefined;
}

export function createBeacons(options: BeaconOptions = {}): Surface {
  const root = element("div", {
    class: "slidx-beacons",
    "aria-hidden": "true",
  });

  let slide = 0;
  let viewers: readonly Viewer[] = [];
  let frame: HTMLIFrameElement | undefined;

  function read(): SlideGeometry | undefined {
    if (options.geometry) return options.geometry();
    return frame ? readGeometry(frame) : undefined;
  }

  function paint(): void {
    const geometry = read();

    fill(
      root,
      geometry === undefined
        ? []
        : marksFor(viewers, slide, geometry).map(({ viewer, rect }) => mark(viewer, rect)),
    );
  }

  return {
    root,

    render(state: EditorState) {
      applyBeaconStyles(root.ownerDocument);
      slide = state.selection.slide;
      viewers = state.viewers;

      if (!frame && !options.geometry) {
        frame =
          root.ownerDocument.querySelector<HTMLIFrameElement>(".slidx-canvas-frame") ?? undefined;
        frame?.addEventListener("load", paint);
        root.ownerDocument.defaultView?.addEventListener("resize", paint);
      }

      paint();
    },
  };
}

/** One mark to draw, for each viewer there is somewhere to draw one. */
export function marksFor(
  viewers: readonly Viewer[],
  slide: number,
  geometry: SlideGeometry,
): { viewer: Viewer; rect: Rect }[] {
  return viewers.flatMap((viewer) => {
    if (viewer.local || viewer.slide !== slide || viewer.block === undefined) return [];

    const box = geometry.blocks.find((candidate) => candidate.index === viewer.block);

    return box ? [{ viewer, rect: box.rect }] : [];
  });
}

function mark(viewer: Viewer, rect: Rect): HTMLElement {
  const node = element("div", { class: "slidx-beacon", "data-viewer": viewer.id }, [
    element("span", { class: "slidx-beacon-name" }, [viewer.label]),
  ]);

  node.style.setProperty("--slidx-beacon-ink", inkFor(viewer.id));
  node.style.left = `${rect.left}px`;
  node.style.top = `${rect.top}px`;
  node.style.width = `${rect.width}px`;
  node.style.height = `${rect.height}px`;

  return node;
}

/**
 * The colour a seat keeps for as long as it is connected.
 *
 * FNV-1a, which is here because it is four lines and spreads short strings
 * well. Nothing depends on it being hard to reverse: the input is a seat id the
 * server issued to a browser it is already talking to.
 */
export function inkFor(id: string): string {
  let hash = 0x811c9dc5;

  for (let index = 0; index < id.length; index += 1) {
    hash ^= id.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }

  return BEACON_INKS[hash % BEACON_INKS.length]!;
}
