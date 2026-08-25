/**
 * What a cell of the timeline means, and what changing one asks for.
 *
 * Separate from the panel because it fails differently: this is arithmetic over
 * a grid — which position an inserted action needs in order to land on the
 * column that was clicked — and it is wrong silently, on the one slide nobody
 * tried. The panel is DOM, and it is wrong on sight.
 *
 * Nothing here builds Markdown. Every function returns an
 * [`EditOp`](./operations), which `slidx_edit` turns into a byte-range splice;
 * an action is data all the way across.
 */

import type { StepGrid, StepPlacement, StepRow } from "./client";
import type { EditOp, SlideRef, StepAction, StepOptions } from "./operations";

/**
 * Every named animation the compiler accepts, in the order `EffectPreset::ALL`
 * lists them. Completing from this constant is what makes a preset added to
 * the enum reachable from the timeline without a second list here — keep the
 * two in step; the rust side is the definition.
 */
export const PRESETS = [
  "none",
  "fade",
  "fly-in",
  "wipe",
  "zoom",
  "split",
  "grow",
  "float",
  "typewriter",
  "draw",
  "pulse",
  "shake",
  "spin",
  "color-pulse",
  "underline",
  "fade-out",
  "fly-out",
  "wipe-out",
  "zoom-out",
  "shrink",
] as const;

export type PresetName = (typeof PRESETS)[number];

/** A slide with nothing staged, so a surface has a grid before one exists. */
export const NO_STEPS: StepGrid = { rows: [], actions: [], stops: 1, declared: false };

/** The intents a cell can be switched between. */
export type CellKind = "reveal" | "hide" | "emphasize";

/**
 * True when the stops came from structure rather than from lines in the file.
 *
 * The distinction a timeline has to make before it changes anything: a
 * generated stop has no line to splice, so it can be shown and it cannot be
 * edited in place. A slide with no steps at all is *not* generated — adding the
 * first one writes the key.
 */
export function isGenerated(grid: StepGrid): boolean {
  return !grid.declared && grid.actions.length > 0;
}

/** The action landing on `stop` that names `target`, if there is one. */
export function actionAt(grid: StepGrid, target: string, stop: number): StepPlacement | undefined {
  return grid.actions.find((action) => action.stop === stop && action.targets.includes(target));
}

/**
 * Where an action has to sit in the list to land on `stop`.
 *
 * Before everything already at that stop or later, which is what makes a click
 * on the third column produce a third stop rather than a fourth. A column past
 * the last stop gives the end of the list, so appending needs no separate case.
 */
export function positionFor(grid: StepGrid, stop: number): number {
  const at = grid.actions.findIndex((action) => action.stop >= stop);

  return at === -1 ? grid.actions.length : at;
}

/**
 * The operation a click on one cell asks for.
 *
 * `undefined` for a cell that is not a step: the resting frame, where nothing
 * has happened yet, and every cell of a slide whose stops are generated.
 */
export function toggleCell(
  grid: StepGrid,
  slide: SlideRef,
  row: StepRow,
  stop: number,
): EditOp | undefined {
  if (stop === 0 || isGenerated(grid)) return undefined;

  const found = actionAt(grid, row.target, stop);
  if (found) return { op: "removeStep", slide, index: found.index };

  return { op: "addStep", slide, at: positionFor(grid, stop), action: intentFor(row, stop) };
}

/**
 * The operation that changes what the action in one cell does.
 *
 * Only the intent changes; the target and the timing are whatever the author
 * already wrote, because a picker that also reset the duration would make
 * choosing an effect a destructive act.
 */
export function setKind(
  grid: StepGrid,
  slide: SlideRef,
  row: StepRow,
  stop: number,
  kind: CellKind,
): EditOp | undefined {
  const found = actionAt(grid, row.target, stop);
  if (!found || isGenerated(grid)) return undefined;

  return {
    op: "setStep",
    slide,
    index: found.index,
    action: actionOf(kind, row.target, found.preset),
  };
}

/**
 * The operation that moves the action in one cell a stop earlier or later.
 *
 * Refused at the ends rather than clamped: a move that goes nowhere would still
 * be a press on the undo stack.
 */
export function moveCell(
  grid: StepGrid,
  slide: SlideRef,
  row: StepRow,
  stop: number,
  by: number,
): EditOp | undefined {
  const found = actionAt(grid, row.target, stop);
  if (!found || isGenerated(grid)) return undefined;

  const to = found.index + by;
  if (to < 0 || to >= grid.actions.length) return undefined;

  return { op: "moveStep", slide, from: found.index, to };
}

/**
 * What a new action on this row at this stop should do.
 *
 * A row already on screen has nothing left to reveal, so the thing that reads
 * as "something happens here" is taking it away. Either way it is one
 * operation, and the intent is changeable afterwards without moving the stop.
 */
function intentFor(row: StepRow, stop: number): StepAction {
  const showing = row.visible[stop - 1] ?? false;

  return actionOf(showing ? "hide" : "reveal", row.target);
}

/**
 * The operation that names an animation on one cell.
 *
 * Empty string means the theme should pick, which is written by *omitting*
 * `preset` rather than by writing `none` — `none` is a real preset that turns
 * motion off.
 */
export function setPreset(
  grid: StepGrid,
  slide: SlideRef,
  row: StepRow,
  stop: number,
  preset: string,
): EditOp | undefined {
  const found = actionAt(grid, row.target, stop);
  const kind = found ? cellKindOf(found.kind) : undefined;
  if (!found || kind === undefined || isGenerated(grid)) return undefined;

  return {
    op: "setStep",
    slide,
    index: found.index,
    action: actionOf(kind, row.target, preset === "" ? undefined : preset),
  };
}

function cellKindOf(kind: StepPlacement["kind"]): CellKind | undefined {
  if (kind === "reveal" || kind === "hide" || kind === "emphasize") return kind;
  return undefined;
}

/**
 * One intent, with no timing said out loud.
 *
 * Empty options on purpose when the author has not named a preset: the theme
 * owns motion, and an editor that wrote a duration and an easing onto every
 * action it touched would turn a one-word change into a diff across the slide.
 * A preset the author *did* pick is the one thing that belongs here.
 */
function actionOf(kind: CellKind, target: string, preset?: string): StepAction {
  const options: StepOptions = {};
  if (preset !== undefined) options.preset = preset;

  if (kind === "hide") return { hide: { target, options } };
  if (kind === "emphasize") return { emphasize: { target, options } };

  return { reveal: { target, options } };
}
