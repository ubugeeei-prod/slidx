/**
 * How wide the two side panels are allowed to be.
 *
 * They were fixed at 232 and 296 pixels — a decision taken once, on one screen,
 * for every deck anybody will ever write. A Japanese title needs more room than
 * an English one, a laptop has less to give than a monitor, and an author
 * arranging blocks wants the canvas as wide as it goes.
 */

import { describe, expect, it } from "vite-plus/test";

import { DEFAULT_WIDTH, LIMITS, resized, startingWidth } from "../src/panels";
import { STYLESHEET } from "../src/styles";
import { TIMELINE_STYLESHEET } from "../src/timeline-styles";

describe("where an edge lands", () => {
  it("widens the outline when its grip is dragged right", () => {
    expect(resized("outline", 232, 40)).toBe(272);
  });

  it("widens the inspector when its grip is dragged left", () => {
    // It is on the right, so the sign is part of which edge this is rather
    // than something every call site has to remember.
    expect(resized("inspector", 296, -40)).toBe(336);
  });

  it("stops at the floor, which is what the panel is still for", () => {
    // An outline narrower than this shows a slide number and one character of
    // its title, which is not an outline.
    expect(resized("outline", 232, -400)).toBe(LIMITS.outline.min);
  });

  it("stops at the ceiling, because the canvas is why the window is open", () => {
    expect(resized("inspector", 296, -2000)).toBe(LIMITS.inspector.max);
  });

  it("rounds, so a width is never a fraction of a pixel", () => {
    expect(resized("outline", 232.4, 0.3)).toBe(233);
  });
});

/**
 * The rows every `.slidx-editor` rule lays the grid out in, one per stylesheet
 * that has an opinion about it.
 */
function editorAreas(sheet: string): string[][] {
  const css = sheet.replaceAll(/\/\*[\s\S]*?\*\//g, "");
  const areas: string[][] = [];

  for (const rule of css.matchAll(/\.slidx-editor\s*\{([^}]*)\}/g)) {
    const declared = /grid-template-areas:([^;]*);/.exec(rule[1]!)?.[1];
    if (declared === undefined) continue;
    areas.push(
      Array.from(declared.matchAll(/"([^"]*)"/g), (row) => row[1]!.trim().split(/\s+/).join(" ")),
    );
  }

  return areas;
}

describe("the grid the panels sit in", () => {
  // `grid-template-areas` is one declaration for the whole grid, so a panel
  // that adds a row of its own restates the panel row too. One that named the
  // three columns from before the grips existed put the canvas in a 4px grip
  // track and handed its width to the inspector.
  const PANELS = "outline grip-outline canvas grip-inspector inspector";
  const sheets = [STYLESHEET, TIMELINE_STYLESHEET].flatMap(editorAreas);

  it("is laid out by more than one stylesheet, which is why the rest of this matters", () => {
    expect(sheets.length).toBeGreaterThan(1);
  });

  it("puts the panels and their grips in the same columns everywhere", () => {
    for (const rows of sheets) expect(rows[0]).toBe(PANELS);
  });

  it("gives every row the same number of columns as the panel row", () => {
    const columns = PANELS.split(" ").length;
    for (const rows of sheets) {
      for (const row of rows) expect(row.split(" ")).toHaveLength(columns);
    }
  });
});

describe("where a panel starts", () => {
  it("uses the old fixed widths when nothing was remembered", () => {
    expect(startingWidth("outline", undefined)).toBe(DEFAULT_WIDTH.outline);
    expect(startingWidth("inspector", undefined)).toBe(DEFAULT_WIDTH.inspector);
  });

  it("remembers what was dragged", () => {
    expect(startingWidth("outline", { getItem: () => "300" })).toBe(300);
  });

  it("clamps a remembered width rather than trusting it", () => {
    // Storage outlives any version of this editor. A limit that moved would
    // otherwise leave somebody with a panel they cannot drag back into range.
    expect(startingWidth("outline", { getItem: () => "9999" })).toBe(LIMITS.outline.max);
    expect(startingWidth("outline", { getItem: () => "1" })).toBe(LIMITS.outline.min);
  });

  it("ignores a stored value that is not a width", () => {
    expect(startingWidth("inspector", { getItem: () => "wide" })).toBe(DEFAULT_WIDTH.inspector);
    expect(startingWidth("inspector", { getItem: () => "" })).toBe(DEFAULT_WIDTH.inspector);
  });
});
