/**
 * Undo, as a list of edits rather than a second model of the document.
 *
 * The obvious implementation keeps a stack of whole sources. It works, and it
 * means the editor holds a second copy of the document that can disagree with
 * the file — which is the failure this architecture exists to prevent. An edit
 * is already a value that knows how to take itself back, so the stack holds
 * those and the pipeline stays the only thing that touches bytes.
 *
 * Redo is undo of undo. Applying an edit hands back its inverse, so one shape
 * serves both directions and neither needs the operation that started it.
 */

import type { Edit } from "./operations";

/**
 * Past this, a session is holding more history than anyone will walk back, and
 * every entry is a list of byte ranges into a source that no longer exists.
 */
const DEPTH = 200;

export interface History {
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  /** An operation was applied, and `inverse` takes it back. */
  applied(inverse: Edit): void;
  /** The edit undo should apply, taken off the stack. */
  nextUndo(): Edit | undefined;
  /** What came back from applying it, which is what redo will use. */
  undone(inverse: Edit): void;
  nextRedo(): Edit | undefined;
  redone(inverse: Edit): void;
  clear(): void;
}

export function createHistory(depth = DEPTH): History {
  let undoable: Edit[] = [];
  let redoable: Edit[] = [];

  function push(stack: Edit[], edit: Edit): void {
    stack.push(edit);
    if (stack.length > depth) stack.shift();
  }

  return {
    get canUndo() {
      return undoable.length > 0;
    },
    get canRedo() {
      return redoable.length > 0;
    },

    applied(inverse) {
      // An operation that asked for what the deck already said changed nothing
      // and must not cost a press. Editors emit those constantly — a drag that
      // ends where it started, a field committed without being touched.
      if (inverse.length === 0) return;

      push(undoable, inverse);
      // Editing after undoing starts a new line of history; the branch that
      // was ahead is no longer reachable from here.
      redoable = [];
    },

    nextUndo: () => undoable.pop(),
    undone: (inverse) => push(redoable, inverse),
    nextRedo: () => redoable.pop(),
    redone: (inverse) => push(undoable, inverse),

    clear() {
      undoable = [];
      redoable = [];
    },
  };
}
