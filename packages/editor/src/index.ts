/**
 * `@slidx/editor` — the deck outline, the slide canvas, and the inspector.
 *
 * The claim this package exists to keep is that the canvas and the Markdown are
 * two views of one document rather than an import and an export. Three rules
 * follow from it, and none of them is negotiable for a small case:
 *
 * **Nothing here writes Markdown.** Every change is an operation handed to
 * `slidx_edit`, which splices the bytes the author saved. A string built in a
 * browser would be a second writer, and the round trip does not survive one.
 *
 * **The canvas is the deck's own page.** An iframe on the real route, rendered
 * by the same WebAssembly module the build uses. A preview drawn some other way
 * would be a second answer about layout.
 *
 * **No framework.** slidx is framework-agnostic and every integration is opt-in
 * and removable. An editor that needed Vue or React would make that false in
 * the one place an author looks at all day.
 *
 * It ships in the dev server and only there: the routes it talks to write to
 * the author's files, and `vite build` never registers them.
 */

import { createCanvas } from "./canvas";
import { createClient, type EditorClient } from "./client";
import { createDiagnostics } from "./diagnostics";
import { element } from "./dom";
import { createInspector } from "./inspector";
import { createOutline, type Surface } from "./outline";
import { occurrenceInRendered, locateSelection } from "./selection";
import { createSession, type Session } from "./session";
import { applyStyles } from "./styles";

export type { EditOp, Edit, MarkAttributes, ByteSpan } from "./operations";
export type { EditorClient, DeckState, EditAnswer, Finding, SlideSummary } from "./client";
export type { EditorState, Selection, Session } from "./session";
export { createClient } from "./client";
export { createSession } from "./session";
export { createHistory } from "./history";
export { locateSelection, occurrenceInRendered } from "./selection";
export { routeFor } from "./canvas";

export interface MountOptions {
  /** The route the deck is served under, so the canvas shows the real page. */
  deckBase?: string;
  client?: EditorClient;
}

export interface MountedEditor {
  session: Session;
  destroy(): void;
}

/** Builds the editor into an element and reads the deck into it. */
export function mount(root: HTMLElement, options: MountOptions = {}): MountedEditor {
  applyStyles(root.ownerDocument);

  const client = options.client ?? createClient();
  const session = createSession(client);
  const bodyOf = (slide: number) => session.bodyOf(slide);

  const run = (op: Parameters<Session["run"]>[0]) => void session.run(op);
  const select = (slide: number) => session.select({ slide, range: undefined, text: undefined });

  const outline = createOutline({ select, run });
  const canvas = createCanvas(
    {
      run,
      selected(text, at) {
        // Which appearance was picked is decided in the text a reader sees,
        // and then looked for in the Markdown. The inspector says so when the
        // two spell it differently.
        const body = bodyOf(session.state().selection.slide);
        const found = locateSelection(body, text, occurrenceInRendered(body, text, at));

        session.select({ text, range: "problem" in found ? undefined : found.range });
      },
    },
    { deckBase: options.deckBase ?? "slides", bodyOf },
  );
  const inspector = createInspector({ run }, { bodyOf });
  const diagnostics = createDiagnostics({ select });

  const surfaces: Surface[] = [outline, canvas, inspector, diagnostics];
  const frame = element(
    "div",
    { class: "slidx-editor" },
    surfaces.map((surface) => surface.root),
  );
  root.append(frame);

  const unsubscribe = session.subscribe((state) => {
    for (const surface of surfaces) surface.render(state);
  });

  const keys = keyboard(session);
  root.ownerDocument.addEventListener("keydown", keys);

  void session.open();

  return {
    session,
    destroy() {
      unsubscribe();
      root.ownerDocument.removeEventListener("keydown", keys);
      frame.remove();
    },
  };
}

/**
 * Undo and redo, on the two shapes every editor on both platforms uses.
 *
 * Nothing else is bound. A tool that steals keys from the field an author is
 * typing in is a tool they fight.
 */
export function keyboard(session: Session): (event: KeyboardEvent) => void {
  return (event) => {
    if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;

    event.preventDefault();
    void (event.shiftKey ? session.redo() : session.undo());
  };
}
