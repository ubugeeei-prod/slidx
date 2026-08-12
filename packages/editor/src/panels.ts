/**
 * The two edges between the three panels, and how wide each one is.
 *
 * The outline and the inspector were fixed at 232 and 296 pixels, which is a
 * decision taken once, on one screen, for every deck anybody will ever write.
 * A Japanese title needs more room than an English one; a laptop has less to
 * give than a monitor; and an author who is arranging blocks wants the canvas
 * as wide as it goes and the inspector out of the way.
 *
 * # Why a custom property rather than a style on each panel
 *
 * The width belongs to the grid, and the grid is one declaration. Writing it as
 * two custom properties on the editor root means the layout stays a single
 * `grid-template-columns` that can be read in one line, and a drag is one
 * assignment rather than a pair of writes that can disagree.
 */

/** Which edge a grip moves. */
export type Edge = "outline" | "inspector";

/** The custom property each edge writes. */
export const WIDTH_PROPERTY: Record<Edge, string> = {
  outline: "--slidx-e-outline-width",
  inspector: "--slidx-e-inspector-width",
};

/** Where each panel starts, before anybody drags anything. */
export const DEFAULT_WIDTH: Record<Edge, number> = {
  outline: 232,
  inspector: 296,
};

/**
 * How narrow and how wide a panel may be.
 *
 * The floor is what the panel is still *for*: an outline narrower than this
 * shows a slide number and one character of its title, which is not an outline.
 * The ceiling exists because the canvas is the reason the window is open, and a
 * grip dragged to the far side of the screen should stop rather than leave
 * nothing to edit.
 */
export const LIMITS: Record<Edge, { min: number; max: number }> = {
  outline: { min: 140, max: 480 },
  inspector: { min: 200, max: 560 },
};

/**
 * The width an edge lands on, given where the drag started and how far it went.
 *
 * The inspector is on the right, so dragging its grip left makes it *wider* —
 * the sign is part of which edge this is, not something a caller should have to
 * remember at every call site.
 */
export function resized(edge: Edge, from: number, delta: number): number {
  const { min, max } = LIMITS[edge];
  const moved = edge === "outline" ? from + delta : from - delta;

  return Math.round(Math.min(max, Math.max(min, moved)));
}

/** Where the choice is remembered, which is this browser and nothing else. */
export const WIDTH_KEY: Record<Edge, string> = {
  outline: "slidx.panel.outline",
  inspector: "slidx.panel.inspector",
};

/**
 * The width to start at: what was remembered, or the default.
 *
 * A stored value is clamped rather than trusted. It outlives any version of
 * this editor, and a limit that moved would otherwise leave somebody with a
 * panel they can no longer drag back into range.
 */
export function startingWidth(edge: Edge, storage: Pick<Storage, "getItem"> | undefined): number {
  const found = Number(storage?.getItem(WIDTH_KEY[edge]));

  return Number.isFinite(found) && found > 0 ? resized(edge, found, 0) : DEFAULT_WIDTH[edge];
}
