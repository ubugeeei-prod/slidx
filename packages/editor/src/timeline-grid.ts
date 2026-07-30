/**
 * Drawing the grid: a column header per stop, a row per addressable thing.
 *
 * Split from the panel because it answers one question — what does this slide
 * look like as rows and stops — and the panel answers the other, which is what
 * an author's press means. Handed everything it needs and holding nothing, so
 * the same drawing is what a test reads and what the editor shows.
 *
 * Two details that are accessibility rather than decoration. Every cell is a
 * real `<button>`, so it answers a keyboard without this module reimplementing
 * one; and only the selected cell is a tab stop, so Tab leaves the panel instead
 * of walking thirty buttons to get out of it.
 */

import type { StepGrid, StepRow } from "./client";
import { element } from "./dom";
import { actionAt, isGenerated } from "./timeline-cells";

/** What each intent looks like in a cell, at one character wide. */
const GLYPH: Record<string, string> = {
  reveal: "▸",
  hide: "▪",
  emphasize: "◆",
  set: "≡",
  group: "◈",
};

export interface GridView {
  /** The stop the playhead is on. */
  stop: number;
  /** The row the selection is on. */
  row: number;
  /** A column header was pressed. Never called for the column past the end. */
  scrub(stop: number): void;
  /** A cell was pressed. */
  pick(row: number, stop: number): void;
}

export function drawGrid(grid: StepGrid, view: GridView): HTMLElement {
  // One column per stop plus one past the end, which is where a new stop goes.
  const columns = grid.stops + 1;
  const table = element("div", {
    class: "slidx-timeline-grid",
    role: "grid",
    style: `--slidx-t-columns: ${columns}`,
    "data-generated": String(isGenerated(grid)),
  });

  table.append(stopsRow(grid, view, columns));

  for (const [index, entry] of grid.rows.entries()) {
    table.append(rowOf(grid, view, entry, index, columns));
  }

  return table;
}

function stopsRow(grid: StepGrid, view: GridView, columns: number): HTMLElement {
  const header = element("div", { class: "slidx-timeline-stops", role: "row" }, [
    element("span", { class: "slidx-timeline-label" }, ["Stop"]),
  ]);

  for (let stop = 0; stop < columns; stop += 1) {
    const last = stop === grid.stops;
    const button = element(
      "button",
      {
        type: "button",
        class: "slidx-timeline-stop",
        role: "columnheader",
        "aria-current": stop === view.stop ? "true" : undefined,
        "aria-label": last ? "A new stop" : `Stop ${stop}`,
        tabindex: -1,
      },
      [last ? "+" : String(stop)],
    );

    // A column past the end is not a stop the canvas can show.
    if (!last) button.addEventListener("click", () => view.scrub(stop));
    header.append(button);
  }

  return header;
}

function rowOf(
  grid: StepGrid,
  view: GridView,
  entry: StepRow,
  index: number,
  columns: number,
): HTMLElement {
  const label = element(
    "span",
    {
      class: "slidx-timeline-label",
      role: "rowheader",
      // The selector rather than the words: a row whose label was cut, or whose
      // bullet reads the same as another's, is still identifiable.
      title: entry.target,
      "data-named": String(entry.key !== undefined),
    },
    [entry.label],
  );

  const cells: HTMLElement[] = [];
  for (let stop = 0; stop < columns; stop += 1) {
    cells.push(cellOf(grid, view, entry, index, stop));
  }

  return element("div", { class: "slidx-timeline-row", role: "row" }, [label, ...cells]);
}

function cellOf(
  grid: StepGrid,
  view: GridView,
  entry: StepRow,
  index: number,
  stop: number,
): HTMLElement {
  const found = stop < grid.stops ? actionAt(grid, entry.target, stop) : undefined;
  const grouped = found?.kind === "group";
  // The column past the end shows what the last stop shows, because that is
  // what a new stop would inherit.
  const on = entry.visible[Math.min(stop, grid.stops - 1)] ?? false;
  const current = index === view.row && stop === view.stop;

  const cell = element(
    "button",
    {
      type: "button",
      class: "slidx-timeline-cell",
      role: "gridcell",
      "data-on": String(on),
      "data-kind": found?.kind,
      "data-current": String(current),
      "aria-current": current ? "true" : undefined,
      tabindex: current ? 0 : -1,
      // The line the file holds, so an author can see that what the timeline
      // writes is what they would have written.
      title: found?.source,
      "aria-label": describe(entry, stop, found?.kind, grouped),
    },
    [found ? (GLYPH[found.kind] ?? "•") : ""],
  );

  cell.addEventListener("click", () => view.pick(index, stop));

  return cell;
}

/** What a cell is, said in words, because a glyph says nothing out loud. */
function describe(
  entry: StepRow,
  stop: number,
  kind: string | undefined,
  grouped: boolean,
): string {
  const where = `“${entry.label}” at stop ${stop}`;

  if (grouped) return `part of a group ${where}`;
  if (kind) return `${kind} ${where}`;

  return `nothing happens to ${where}`;
}
