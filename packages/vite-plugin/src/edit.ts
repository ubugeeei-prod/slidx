/**
 * Applying one edit operation to the deck on disk.
 *
 * Every write the visual editor makes comes through here, and every byte of it
 * was computed by `slidx_edit` in the pipeline. This module joins the files,
 * hands the operation over, and puts the result back in the files it came from.
 * It never inspects an operation and never composes Markdown — if the editor
 * needs a change the operation set cannot express, the answer is a new
 * operation in Rust.
 *
 * An operation that names a slide the deck no longer has comes back as an
 * answer rather than a throw: the editor builds operations from a deck it
 * parsed a keystroke ago, and that race is ordinary traffic.
 */

import { writeFile, rm } from "node:fs/promises";

import { applyEdit, revertEdit, slideSpans } from "@slidx/wasm";

import {
  joinDeck,
  planFileWrites,
  type DeckFile,
  type FileWrite,
  type LocatedSource,
} from "./files";
import { ensureReady } from "./pipeline";

export type { DeckFile, FileWrite, SlideSpans } from "./files";

/**
 * One change to a deck, as `slidx_edit` defines it.
 *
 * Opaque here on purpose. The plugin routes an operation to the pipeline and
 * never reads it, so adding an operation in Rust needs nothing on this side.
 */
export type EditOp = Record<string, unknown>;

/** The splices that take an edit back, for the editor's undo stack. */
export type Edit = readonly unknown[];

/** What an operation named that the deck does not have. */
export interface EditRefusal {
  error: string;
  [detail: string]: unknown;
}

/** A deck source, after an operation was asked for. */
export interface DeckEdit {
  /** Files whose bytes changed. Empty when the deck already said this. */
  writes: FileWrite[];
  /** The whole deck source after the operation. */
  source: string;
  /** The edit that takes this one back. */
  undo: Edit;
  /** Set when the operation named something the deck does not have. */
  error?: EditRefusal;
}

interface WasmEdit extends LocatedSource {
  source: string;
  undo: Edit;
  error?: EditRefusal;
}

/** The deck with one operation applied, and the files that have to change. */
export async function applyOperation(
  files: readonly DeckFile[],
  separator: string,
  op: EditOp,
): Promise<DeckEdit> {
  return plan(
    files,
    separator,
    (before) => applyEdit(before.source, op, { separator }) as WasmEdit,
  );
}

/**
 * The deck with an edit off the undo stack applied.
 *
 * Redo is undo of undo, so this serves both directions: the `undo` it hands
 * back is the edit that does the change again.
 */
export async function revertOperation(
  files: readonly DeckFile[],
  separator: string,
  edit: Edit,
): Promise<DeckEdit> {
  return plan(
    files,
    separator,
    (before) => revertEdit(before.source, edit, { separator }) as WasmEdit,
  );
}

/**
 * Where every slide of a source is.
 *
 * The editor needs these to show one slide at a time and to turn a selection
 * into the byte range an operation names, and they have to come from the same
 * pipeline that computes the splice or the two would be measuring different
 * documents.
 */
export async function locate(source: string, separator: string): Promise<LocatedSource> {
  await ensureReady();

  return { source, slides: slideSpans(source, { separator }) as LocatedSource["slides"] };
}

async function plan(
  files: readonly DeckFile[],
  separator: string,
  run: (before: LocatedSource) => WasmEdit,
): Promise<DeckEdit> {
  await ensureReady();

  const before = await locate(joinDeck(files, separator).source, separator);
  const after = run(before);

  if (after.error) return { writes: [], source: before.source, undo: [], error: after.error };

  return {
    writes: planFileWrites(files, separator, before, after),
    source: after.source,
    undo: after.undo,
  };
}

/**
 * Puts a write plan on disk.
 *
 * A file with nothing left in it is removed rather than emptied: an empty slide
 * file would join the deck as a blank slide, and the deck would gain one every
 * time an author deleted the last slide of a file.
 */
export async function writeDeck(writes: readonly FileWrite[]): Promise<void> {
  await Promise.all(
    writes.map((write) =>
      write.source === null ? rm(write.path, { force: true }) : writeFile(write.path, write.source),
    ),
  );
}
