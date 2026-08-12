/**
 * `@slidxjs/editor` — the deck outline, the slide canvas, and the inspector.
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

import { createArrange } from "./arrange";
import { createBeacons } from "./beacons";
import { createCanvas, routeFor } from "./canvas";
import { createClient, type EditorClient } from "./client";
import { createPresence } from "./collab";
import { createDiagnostics } from "./diagnostics";
import { element } from "./dom";
import { createFreeform } from "./freeform";
import { createGrips } from "./grips";
import { createInspector } from "./inspector";
import { createMediaDrop } from "./media-drop";
import { createOutline, type Surface } from "./outline";
import { createResize } from "./resize";
import { createRevisions } from "./revisions";
import { occurrenceInRendered, locateSelection } from "./selection";
import { createSession, type Session } from "./session";
import { createShortcuts } from "./shortcuts";
import { createStoryboard } from "./storyboard";
import { applyStyles } from "./styles";
import { createTimeline } from "./timeline";

export type {
  EditOp,
  Edit,
  MarkAttributes,
  BlockAttributes,
  ByteSpan,
  MediaKind,
} from "./operations";
export type {
  BlockSpans,
  EditorClient,
  DeckState,
  EditAnswer,
  Finding,
  LayoutChoice,
  MarkSpans,
  Measurement,
  SlideSummary,
  UploadedMedia,
} from "./client";
export type { Change, TextPlan, TextRun } from "./text";
export { changeBetween, editableIn, planBlock, rangeOf, runsIn } from "./text";
export type { SlideGeometry, RegionBox, BlockBox, Rect } from "./geometry";
export { readGeometry, BLOCK_ATTRIBUTE, REGION_ATTRIBUTE, WIDTH_ATTRIBUTE } from "./geometry";
export { createBeacons, inkFor, marksFor, BEACON_INKS } from "./beacons";
export type { Frame, Viewer } from "./collab";
export { readFrames } from "./collab";
export type { Insertion } from "./placement";
export { landing, nudge, guides, arrival, insertion } from "./placement";
export type { BlockWidth, Step } from "./widths";
export { WIDTHS, boxAt, narrowing, shareAt, shareOf, stepped, widthOf } from "./widths";
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

/**
 * `localStorage`, when this document is allowed one.
 *
 * Reading it throws outright in a sandboxed frame and in a browser with storage
 * disabled, so the editor asks once and carries on without if the answer is no —
 * the only thing that costs is a viewing preference that does not survive a
 * reload.
 */
function safeStorage(document: Document): Storage | undefined {
  try {
    return document.defaultView?.localStorage ?? undefined;
  } catch {
    return undefined;
  }
}

/** Builds the editor into an element and reads the deck into it. */
export function mount(root: HTMLElement, options: MountOptions = {}): MountedEditor {
  applyStyles(root.ownerDocument);

  const client = options.client ?? createClient();
  const session = createSession(client);
  const bodyOf = (slide: number) => session.bodyOf(slide);

  const run = (op: Parameters<Session["run"]>[0]) => void session.run(op);
  const select = (slide: number) =>
    session.select({
      slide,
      block: undefined,
      range: undefined,
      text: undefined,
    });

  const outline = createOutline({ select, run });
  const canvas = createCanvas(
    {
      run,
      selectedBlock(block) {
        session.select({ block, range: undefined, text: undefined });
      },
      selected(text, at) {
        // Which appearance was picked is decided in the text a reader sees,
        // and then looked for in the Markdown. The inspector says so when the
        // two spell it differently.
        const body = bodyOf(session.state().selection.slide);
        const found = locateSelection(body, text, occurrenceInRendered(body, text, at));

        session.select({ text, range: "problem" in found ? undefined : found.range });
      },
    },
    {
      deckBase: options.deckBase ?? "slides",
      bodyOf,
      blocksOf: (slide) => session.blocksOf(slide),
      storage: safeStorage(root.ownerDocument),
    },
  );
  const inspector = createInspector(
    { run },
    { bodyOf, blocksOf: (slide) => session.blocksOf(slide) },
  );
  const diagnostics = createDiagnostics({ select });

  const storyboard = createStoryboard({ select, run: (op) => session.run(op) });
  const deckBase = options.deckBase ?? "slides";
  const arrange = createArrange(
    { run, foresee: (findings) => session.foresee(findings) },
    { measure: (measured) => client.measured(measured) },
  );
  const shortcuts = createShortcuts(session, canvas, {
    nudge: (block, key) => arrange.nudge(block, key),
    present: () => {
      const route = routeFor(deckBase, session.state().selection.slide);
      window.open(`${route}presenter/`, "_blank", "noopener");
    },
  });

  const surfaces: Surface[] = [
    createGrips({ storage: safeStorage(root.ownerDocument) }),
    outline,
    canvas,
    inspector,
    diagnostics,
    createTimeline({ run }),
    createRevisions(
      { reload: () => void session.open() },
      { deckBase: options.deckBase ?? "slides" },
    ),
    arrange,
    createResize(
      { run, foresee: (findings) => session.foresee(findings) },
      { measure: (measured) => client.measured(measured) },
    ),
    createFreeform({
      run,
      select: (block) => session.select({ block, range: undefined, text: undefined }),
    }),
    createMediaDrop({
      upload: (file) => client.upload(file),
      run: (op) => session.run(op),
    }),
    storyboard,
    createBeacons(),
    createPresence({
      reload: () => void session.open(),
      saw: (viewers) => session.saw(viewers),
      follow: (seat) => session.follow(seat),
    }),
    shortcuts,
  ];
  const frame = element(
    "div",
    { class: "slidx-editor" },
    surfaces.map((surface) => surface.root),
  );
  root.append(frame);

  const unsubscribe = session.subscribe((state) => {
    for (const surface of surfaces) surface.render(state);
  });

  const keys = (event: KeyboardEvent) => shortcuts.keydown(event);
  root.ownerDocument.addEventListener("keydown", keys);
  canvas.listen(keys);

  void session.open();

  return {
    session,
    destroy() {
      unsubscribe();
      root.ownerDocument.removeEventListener("keydown", keys);
      // Removing the frame is enough for a surface that is only its own DOM.
      // Presence holds a connection, and a connection survives its element.
      for (const surface of surfaces) surface.destroy?.();
      frame.remove();
    },
  };
}
