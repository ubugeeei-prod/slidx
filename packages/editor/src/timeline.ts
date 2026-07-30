/**
 * The animation timeline: rows are what a slide addresses, columns are its stops.
 *
 * A conventional slide editor puts animation behind a list of effects with a
 * duration and an easing on each. This puts it behind the deck's own model,
 * which is a better surface for one structural reason: **every stop is a
 * complete compiled snapshot rather than a delta.** So the playhead here scrubs
 * — dragging it left is exactly as cheap as dragging it right, nothing is
 * replayed, and there is no animation state to run forward. Scrubbing is the
 * feature; the grid is how you author what it scrubs through.
 *
 * A cell holds the author's *intent* — reveal, hide, emphasize, set — and never
 * a duration or an easing. Motion belongs to the theme, so an editor that wrote
 * timing onto every action it touched would move that decision into the deck one
 * click at a time.
 *
 * # The one-way door
 *
 * A slide staged by `autoSteps:` has stops with no line in the file, so nothing
 * can change one in place. Those stops are shown, marked as generated, and
 * refuse to be clicked — with one action offered that writes them out as an
 * explicit `steps:` list. That door is deliberately visible: converting on the
 * first click would rewrite an author's frontmatter for a gesture they might
 * have meant as a look.
 */

import type { SlideSummary, StepGrid, StepRow } from "./client";
import { element, fill } from "./dom";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import { createPlayhead, type Playhead, type PlayheadOptions } from "./playhead";
import type { EditorState } from "./session";
import {
  NO_STEPS,
  actionAt,
  isGenerated,
  moveCell,
  setKind,
  toggleCell,
  type CellKind,
} from "./timeline-cells";
import { applyTimelineStyles } from "./timeline-styles";

export interface TimelineHandlers {
  run(op: EditOp): void;
}

export interface TimelineOptions extends PlayheadOptions {
  /** Substituted in tests; otherwise the panel makes its own. */
  playhead?: Playhead;
}

/** What each intent looks like in a cell, at one character wide. */
const GLYPH: Record<string, string> = {
  reveal: "▸",
  hide: "▪",
  emphasize: "◆",
  set: "≡",
  group: "◈",
};

const KINDS: CellKind[] = ["reveal", "hide", "emphasize"];

export function createTimeline(
  handlers: TimelineHandlers,
  options: TimelineOptions = {},
): Surface {
  const where = element("span", { class: "slidx-timeline-where" });
  const scrub = element("input", {
    type: "range",
    class: "slidx-timeline-scrub",
    min: 0,
    max: 0,
    step: 1,
    "aria-label": "Stop",
  });
  const body = element("div", { class: "slidx-timeline-body" });
  const root = element("section", { class: "slidx-timeline", "aria-label": "Timeline" }, [
    element("header", { class: "slidx-panel-head" }, [
      element("h2", {}, ["Timeline"]),
      scrub,
      where,
    ]),
    body,
  ]);

  applyTimelineStyles(root.ownerDocument);

  // Looked up rather than held: the canvas owns that element and replaces its
  // `src` on every edit, so this surface knows it only by the stop being shown.
  const findFrame =
    options.frame ??
    (() => root.ownerDocument.querySelector<HTMLIFrameElement>(".slidx-canvas-frame"));
  const playhead = options.playhead ?? createPlayhead({ ...options, frame: findFrame });

  let slide = 0;
  let grid: StepGrid = NO_STEPS;
  let row = 0;
  let watched: HTMLIFrameElement | null = null;

  /**
   * Says the stop again every time the canvas finishes loading.
   *
   * Every edit reloads that frame, and a reloaded page comes up at its first
   * stop. Without this, clicking a cell to see what stop four now does would
   * answer by showing stop zero. The listener goes on the element rather than
   * on each `src`, because the canvas replaces the URL and keeps the frame.
   */
  function watchFrame(): void {
    const frame = findFrame();
    if (!frame || frame === watched) return;

    watched = frame;
    frame.addEventListener("load", () => playhead.resend());
  }

  /** Shows a stop, and redraws so the column that is current says so. */
  function scrubTo(stop: number): void {
    playhead.show(stop);
    draw();
  }

  function act(op: EditOp | undefined): void {
    if (op) handlers.run(op);
  }

  function selected(): StepRow | undefined {
    return grid.rows[row];
  }

  function draw(): void {
    const focused = root.contains(root.ownerDocument.activeElement);

    scrub.setAttribute("max", String(Math.max(grid.stops - 1, 0)));
    scrub.value = String(playhead.stop);
    // A slide that advances in one press has nothing to scrub through, and a
    // control that cannot move is a control an author tries to move.
    scrub.toggleAttribute("hidden", grid.stops <= 1);
    fill(where, [`${playhead.stop} of ${grid.stops}`]);

    fill(body, panels());
    // A redraw replaces the element the keyboard was on, so focus has to be put
    // back or every arrow key would drop the author out of the grid.
    if (focused) currentCell()?.focus();
  }

  function panels(): HTMLElement[] {
    if (grid.rows.length === 0) {
      return [
        element("p", { class: "slidx-hint" }, [
          "Nothing on this slide is addressable yet. Give a phrase a name in the inspector, or set `autoSteps:` to stage it by structure.",
        ]),
      ];
    }

    const panels: HTMLElement[] = [];
    if (isGenerated(grid)) panels.push(generatedNotice(grid));
    panels.push(gridOf(grid));
    if (!isGenerated(grid)) panels.push(actionBar());

    return panels;
  }

  /**
   * What a generated slide says about itself, and the one thing it offers.
   *
   * Named rather than described: an author who wrote `autoSteps: list` is told
   * that is what they are looking at, because the alternative is a grid that
   * silently ignores clicks.
   */
  function generatedNotice(grid: StepGrid): HTMLElement {
    const adopt = element("button", { type: "button", class: "slidx-timeline-adopt" }, [
      "Write out as steps:",
    ]);
    adopt.addEventListener("click", () => handlers.run({ op: "adoptSteps", slide }));

    const from = grid.auto ? `autoSteps: ${grid.auto}` : "step markers";

    return element("p", { class: "slidx-timeline-generated" }, [
      element("span", {}, [`These stops come from \`${from}\`, so they have no line to change.`]),
      adopt,
    ]);
  }

  function gridOf(grid: StepGrid): HTMLElement {
    // One column per stop plus one past the end, which is where a new stop goes.
    const columns = grid.stops + 1;
    const table = element("div", {
      class: "slidx-timeline-grid",
      role: "grid",
      style: `--slidx-t-columns: ${columns}`,
      "data-generated": String(isGenerated(grid)),
    });

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
          "aria-current": stop === playhead.stop ? "true" : undefined,
          "aria-label": last ? "A new stop" : `Stop ${stop}`,
          tabindex: -1,
        },
        [last ? "+" : String(stop)],
      );
      // A column past the end is not a stop the canvas can show.
      if (!last) button.addEventListener("click", () => scrubTo(stop));
      header.append(button);
    }
    table.append(header);

    for (const [index, entry] of grid.rows.entries()) {
      table.append(rowOf(grid, entry, index, columns));
    }

    return table;
  }

  function rowOf(grid: StepGrid, entry: StepRow, index: number, columns: number): HTMLElement {
    const label = element(
      "span",
      {
        class: "slidx-timeline-label",
        role: "rowheader",
        // The selector rather than the words: a row whose label was cut, or
        // whose bullet reads the same as another, is still identifiable.
        title: entry.target,
        "data-named": String(entry.key !== undefined),
      },
      [entry.label],
    );

    const cells: HTMLElement[] = [];
    for (let stop = 0; stop < columns; stop += 1) {
      cells.push(cellOf(grid, entry, index, stop));
    }

    return element("div", { class: "slidx-timeline-row", role: "row" }, [label, ...cells]);
  }

  function cellOf(grid: StepGrid, entry: StepRow, index: number, stop: number): HTMLElement {
    const found = stop < grid.stops ? actionAt(grid, entry.target, stop) : undefined;
    const grouped = found?.kind === "group";
    const on = entry.visible[Math.min(stop, grid.stops - 1)] ?? false;
    const current = index === row && stop === playhead.stop;

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
        // Roving: one tab stop for the whole grid, so Tab still leaves the
        // panel rather than walking thirty buttons to get out of it.
        tabindex: current ? 0 : -1,
        title: found?.source,
        "aria-label": describe(entry, stop, found?.kind, grouped),
      },
      [found ? (GLYPH[found.kind] ?? "•") : ""],
    );

    cell.addEventListener("click", () => {
      row = index;
      // Clicking a cell also moves the canvas to that stop: the point of the
      // grid is to see what the change did, and stop `n` is one index away.
      if (stop < grid.stops) playhead.show(stop);
      const op = toggleCell(grid, slide, entry, stop);
      op ? handlers.run(op) : draw();
    });

    return cell;
  }

  /**
   * What the selected stop does, and the presses that change it.
   *
   * Every button here is one operation, which is one press of undo. A control
   * that needed two would make the timeline the one surface in the editor where
   * undo does half a job.
   */
  function actionBar(): HTMLElement {
    const entry = selected();
    const found = entry ? actionAt(grid, entry.target, playhead.stop) : undefined;

    const buttons = KINDS.map((kind) => {
      const button = element(
        "button",
        {
          type: "button",
          "data-kind": kind,
          "aria-pressed": found?.kind === kind ? "true" : undefined,
          disabled: found === undefined,
        },
        [kind],
      );
      button.addEventListener("click", () => {
        if (entry) act(setKind(grid, slide, entry, playhead.stop, kind));
      });
      return button;
    });

    const shift = ([-1, 1] as const).map((by) => {
      const button = element(
        "button",
        { type: "button", class: "slidx-timeline-shift", disabled: found === undefined },
        [by < 0 ? "◀ earlier" : "later ▶"],
      );
      button.addEventListener("click", () => {
        if (entry) act(moveCell(grid, slide, entry, playhead.stop, by));
      });
      return button;
    });

    return element("div", { class: "slidx-timeline-actions" }, [
      element("span", { class: "slidx-timeline-what" }, [found?.source ?? "No stop selected."]),
      ...buttons,
      ...shift,
    ]);
  }

  function currentCell(): HTMLElement | null {
    return body.querySelector<HTMLElement>('.slidx-timeline-cell[data-current="true"]');
  }

  /**
   * The presses a timeline needs, and only while it has focus.
   *
   * Everything in slidx is reachable without a mouse, and a grid that answered
   * arrow keys from anywhere on the page would fight the canvas and the fields
   * around it.
   */
  function onKey(event: KeyboardEvent): void {
    if (event.metaKey || event.ctrlKey) return;

    const entry = selected();
    const stop = playhead.stop;

    switch (event.key) {
      case "ArrowRight":
      case "ArrowLeft": {
        const by = event.key === "ArrowRight" ? 1 : -1;
        // With a modifier the selected action moves; without one the playhead
        // does. The same two keys, because they are the same axis.
        if (event.altKey && entry) act(moveCell(grid, slide, entry, stop, by));
        else scrubTo(stop + by);
        break;
      }
      case "ArrowDown":
      case "ArrowUp":
        row = clamp(row + (event.key === "ArrowDown" ? 1 : -1), grid.rows.length - 1);
        draw();
        break;
      case "Enter":
      case " ":
        if (entry) act(toggleCell(grid, slide, entry, stop));
        break;
      case "Delete":
      case "Backspace": {
        const found = entry ? actionAt(grid, entry.target, stop) : undefined;
        if (found && !isGenerated(grid)) {
          handlers.run({ op: "removeStep", slide, index: found.index });
        }
        break;
      }
      default:
        return;
    }

    event.preventDefault();
  }

  root.addEventListener("keydown", onKey);
  scrub.addEventListener("input", () => scrubTo(Number(scrub.value)));

  return {
    root,
    render(state: EditorState) {
      const at = state.selection.slide;
      grid = gridFor(state.slides[at]);
      slide = at;
      row = clamp(row, grid.rows.length - 1);

      playhead.moveTo(at, grid.stops);
      watchFrame();
      draw();
    },
  };
}

/**
 * A slide's grid, or an empty one.
 *
 * Absent rather than empty when the answer came from a pipeline that predates
 * the field, which is a payload the editor still has to draw something for.
 */
function gridFor(slide: SlideSummary | undefined): StepGrid {
  return slide?.steps ?? NO_STEPS;
}

function clamp(value: number, top: number): number {
  return Math.max(0, Math.min(value, Math.max(top, 0)));
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
