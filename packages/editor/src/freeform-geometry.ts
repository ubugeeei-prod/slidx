/**
 * Freeform direct manipulation, as geometry rather than DOM.
 *
 * Every coordinate is in the editor viewport because that is what
 * `readGeometry` reports. The file receives percentages of the slide's safe
 * area through [`insetOf`], so browser zoom, a projector, an image and a PDF all
 * resolve the same frame.
 */

import type { Rect, SlideGeometry } from "./geometry";

export type FrameHandle = "move" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";

/** Nine deliberate places a frame can occupy inside the slide's safe area. */
export const FRAME_ANCHORS = [
  "top-left",
  "top-center",
  "top-right",
  "middle-left",
  "middle-center",
  "middle-right",
  "bottom-left",
  "bottom-center",
  "bottom-right",
] as const;

export type FrameAnchor = (typeof FRAME_ANCHORS)[number];

export interface FrameGuide {
  kind: "safe" | "block";
  axis: "x" | "y";
  at: number;
}

export interface ManipulatedFrame {
  rect: Rect;
  guides: FrameGuide[];
}

/** Places a frame exactly without changing the size the author already chose. */
export function alignedFrame(rect: Rect, safe: Rect, anchor: FrameAnchor): Rect {
  const [vertical, horizontal] = anchor.split("-") as [
    "top" | "middle" | "bottom",
    "left" | "center" | "right",
  ];
  const left =
    horizontal === "left"
      ? safe.left
      : horizontal === "center"
        ? safe.left + (safe.width - rect.width) / 2
        : safe.left + safe.width - rect.width;
  const top =
    vertical === "top"
      ? safe.top
      : vertical === "middle"
        ? safe.top + (safe.height - rect.height) / 2
        : safe.top + safe.height - rect.height;

  return { ...rect, left, top };
}

/** The exact safe-area anchor a frame currently occupies, when it occupies one. */
export function frameAnchorOf(rect: Rect, safe: Rect): FrameAnchor | undefined {
  return FRAME_ANCHORS.find((anchor) => sameFrame(rect, alignedFrame(rect, safe, anchor)));
}

/** Small enough to stay useful, large enough that every handle remains distinct. */
const MIN_SIZE = 28;
/** How close an edge or centre must be before it snaps. */
const SNAP = 6;

/** Moves or resizes one frame, clamps it to the safe area, then snaps it. */
export function manipulateFrame(
  geometry: SlideGeometry,
  index: number,
  from: Rect,
  handle: FrameHandle,
  dx: number,
  dy: number,
  snap = true,
): ManipulatedFrame {
  const safe = geometry.safe;
  let left = from.left;
  let right = from.left + from.width;
  let top = from.top;
  let bottom = from.top + from.height;

  if (handle === "move") {
    left += dx;
    right += dx;
    top += dy;
    bottom += dy;
  } else {
    if (handle.includes("w")) left += dx;
    if (handle.includes("e")) right += dx;
    if (handle.includes("n")) top += dy;
    if (handle.includes("s")) bottom += dy;
  }

  ({ left, right, top, bottom } = constrained({ left, right, top, bottom }, safe, handle));

  const guides: FrameGuide[] = [];
  if (snap) {
    const x = snapAxis(geometry, index, { left, right }, handle, "x");
    left += x.delta;
    right += x.deltaRight;
    if (x.guide) guides.push(x.guide);

    const y = snapAxis(geometry, index, { top, bottom }, handle, "y");
    top += y.delta;
    bottom += y.deltaRight;
    if (y.guide) guides.push(y.guide);
  }

  ({ left, right, top, bottom } = constrained({ left, right, top, bottom }, safe, handle));

  return {
    rect: { left, top, width: right - left, height: bottom - top },
    guides,
  };
}

interface Edges {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

function constrained(edges: Edges, safe: Rect, handle: FrameHandle): Edges {
  const safeRight = safe.left + safe.width;
  const safeBottom = safe.top + safe.height;
  let { left, right, top, bottom } = edges;

  if (handle === "move") {
    const width = right - left;
    const height = bottom - top;
    left = clamp(left, safe.left, safeRight - width);
    top = clamp(top, safe.top, safeBottom - height);
    right = left + width;
    bottom = top + height;
    return { left, right, top, bottom };
  }

  if (handle.includes("w")) left = clamp(left, safe.left, right - MIN_SIZE);
  if (handle.includes("e")) right = clamp(right, left + MIN_SIZE, safeRight);
  if (handle.includes("n")) top = clamp(top, safe.top, bottom - MIN_SIZE);
  if (handle.includes("s")) bottom = clamp(bottom, top + MIN_SIZE, safeBottom);

  return { left, right, top, bottom };
}

interface AxisSnap {
  /** Applied to the leading edge, or to both edges for a move. */
  delta: number;
  /** Applied to the trailing edge for a resize; the same delta for a move. */
  deltaRight: number;
  guide?: FrameGuide;
}

function snapAxis(
  geometry: SlideGeometry,
  index: number,
  edges: Pick<Edges, "left" | "right"> | Pick<Edges, "top" | "bottom">,
  handle: FrameHandle,
  axis: FrameGuide["axis"],
): AxisSnap {
  const horizontal = axis === "x";
  const leading = horizontal
    ? (edges as Pick<Edges, "left" | "right">).left
    : (edges as Pick<Edges, "top" | "bottom">).top;
  const trailing = horizontal
    ? (edges as Pick<Edges, "left" | "right">).right
    : (edges as Pick<Edges, "top" | "bottom">).bottom;
  const points =
    handle === "move"
      ? [leading, (leading + trailing) / 2, trailing]
      : controlsLeading(handle, axis)
        ? [leading]
        : controlsTrailing(handle, axis)
          ? [trailing]
          : [];
  if (points.length === 0) return { delta: 0, deltaRight: 0 };

  const candidates = targets(geometry, index, axis);
  let nearest: { distance: number; delta: number; guide: FrameGuide } | undefined;

  for (const point of points) {
    for (const candidate of candidates) {
      const delta = candidate.at - point;
      const distance = Math.abs(delta);
      if (distance > SNAP || (nearest && distance >= nearest.distance)) continue;

      nearest = { distance, delta, guide: candidate };
    }
  }

  if (!nearest) return { delta: 0, deltaRight: 0 };
  if (handle === "move") {
    return { delta: nearest.delta, deltaRight: nearest.delta, guide: nearest.guide };
  }
  if (controlsLeading(handle, axis)) {
    return { delta: nearest.delta, deltaRight: 0, guide: nearest.guide };
  }

  return { delta: 0, deltaRight: nearest.delta, guide: nearest.guide };
}

function controlsLeading(handle: FrameHandle, axis: FrameGuide["axis"]): boolean {
  return axis === "x" ? handle.includes("w") : handle.includes("n");
}

function controlsTrailing(handle: FrameHandle, axis: FrameGuide["axis"]): boolean {
  return axis === "x" ? handle.includes("e") : handle.includes("s");
}

function targets(geometry: SlideGeometry, index: number, axis: FrameGuide["axis"]): FrameGuide[] {
  const points = (rect: Rect) =>
    axis === "x"
      ? [rect.left, rect.left + rect.width / 2, rect.left + rect.width]
      : [rect.top, rect.top + rect.height / 2, rect.top + rect.height];
  const found: FrameGuide[] = points(geometry.safe).map((at) => ({ kind: "safe", axis, at }));

  for (const block of geometry.blocks) {
    if (block.index === index) continue;
    found.push(...points(block.rect).map((at) => ({ kind: "block" as const, axis, at })));
  }

  return found;
}

/** The one CSS value a whole gesture stores in the Markdown-managed style. */
export function insetOf(rect: Rect, safe: Rect): string {
  const right = safe.left + safe.width - (rect.left + rect.width);
  const bottom = safe.top + safe.height - (rect.top + rect.height);

  return [
    percent(rect.top - safe.top, safe.height),
    percent(right, safe.width),
    percent(bottom, safe.height),
    percent(rect.left - safe.left, safe.width),
  ].join(" ");
}

/** Whether a pointer or key actually changed the box. */
export function sameFrame(left: Rect, right: Rect): boolean {
  return (["left", "top", "width", "height"] as const).every(
    (key) => Math.abs(left[key] - right[key]) < 0.25,
  );
}

function percent(value: number, whole: number): string {
  const ratio = whole <= 0 ? 0 : clamp((value / whole) * 100, 0, 100);
  const written = ratio.toFixed(3).replace(/\.?0+$/, "");
  return `${written}%`;
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(Math.max(value, low), Math.max(low, high));
}
