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
import { createAppbar } from "./appbar";
import { createBeacons } from "./beacons";
import { createCanvas, routeFor, startingScheme } from "./canvas";
import { createClient, type EditorClient } from "./client";
import { createPresence } from "./collab";
import { createCommandPalette } from "./command-palette";
import { createDiagnostics } from "./diagnostics";
import { element } from "./dom";
import { createFreeform } from "./freeform";
import { visualOf } from "./freeform-color";
import { readGeometry } from "./geometry";
import { createInspector } from "./inspector";
import { createMediaDrop } from "./media-drop";
import { createOutline, type Surface } from "./outline";
import { createPanelResize } from "./panel-resize";
import { createResize } from "./resize";
import { createRevisions } from "./revisions";
import { occurrenceInRendered, locateSelection } from "./selection";
import { createSession, type Session } from "./session";
import { createShareControl } from "./share-control";
import { createShortcuts } from "./shortcuts";
import { createStoryboard } from "./storyboard";
import { applyStyles } from "./styles";
import { createTextBar } from "./text-bar";
import { createTimeline } from "./timeline";
import { createWorkspaceFocus } from "./workspace-focus";
import type { SlideKind } from "./operations";

export type {
  EditOp,
  Edit,
  MarkAttributes,
  BlockAttributes,
  ByteSpan,
  BlockKind,
  SlideKind,
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
  ThemeChoice,
  ThemePaletteChoice,
  TransitionChoice,
  UploadedMedia,
  SharingInfo,
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
export type { BlockWidth, ShareWidth, Step } from "./widths";
export { WIDTHS, boxAt, narrowing, shareAt, shareOf, stepped, widthOf } from "./widths";
export type { EditorState, Selection, Session } from "./session";
export { createClient } from "./client";
export { createSession } from "./session";
export { createHistory } from "./history";
export { locateSelection, occurrenceInRendered } from "./selection";
export { routeFor } from "./canvas";

export interface DeliveryRoutes {
  audience: string;
  presenter: string;
  print: string;
}

/** Every finished-deck surface, derived from the same deck and slide route. */
export function deliveryRoutes(base: string, slide: number): DeliveryRoutes {
  const audience = routeFor(base, slide);
  const deck = routeFor(base, 0);

  return {
    audience,
    presenter: `${audience}presenter/`,
    print: `${deck}print/`,
  };
}

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
  const storage = safeStorage(root.ownerDocument);

  const run = (op: Parameters<Session["run"]>[0]) => session.run(op);
  const select = (slide: number) =>
    session.select({
      slide,
      block: undefined,
      range: undefined,
      text: undefined,
    });

  let outline: ReturnType<typeof createOutline> | undefined;
  let inspector: ReturnType<typeof createInspector> | undefined;
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
      storage,
      schemeChanged: (scheme) => {
        outline?.showScheme(scheme);
        inspector?.render(session.state());
      },
    },
  );
  const deckBase = options.deckBase ?? "slides";
  const deliver = (target: keyof DeliveryRoutes) => {
    const routes = deliveryRoutes(deckBase, session.state().selection.slide);
    window.open(routes[target], "_blank", "noopener");
  };
  const present = () => deliver("presenter");
  const audience = () => deliver("audience");
  const print = () => deliver("print");
  outline = createOutline(
    { select, run, created: () => canvas.focusFresh() },
    { preview: (slide) => routeFor(deckBase, slide), scheme: startingScheme(storage) },
  );
  const canvasFrame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
  inspector = createInspector(
    {
      run,
      selectBlock: (block) => session.select({ block, range: undefined, text: undefined }),
    },
    {
      bodyOf,
      blocksOf: (slide) => session.blocksOf(slide),
      geometry: () => readGeometry(canvasFrame),
      visualOf: (block) => visualOf(canvasFrame, block),
      scheme: () => canvas.palette(),
    },
  );
  // The session announces an edit before the live canvas swap finishes. Any
  // inspector field derived from rendered CSS therefore needs the frame's load
  // boundary as well as session state; otherwise a newly pinned frame or chosen
  // colour is visible on the slide while its control still describes the old
  // page until some unrelated action happens.
  const canvasRendered = () => inspector?.render(session.state());
  canvasFrame.addEventListener("load", canvasRendered);
  const diagnostics = createDiagnostics({ select });

  const storyboard = createStoryboard({ select, run: (op) => session.run(op) });
  const arrange = createArrange(
    { run, foresee: (findings) => session.foresee(findings) },
    { measure: (measured) => client.measured(measured) },
  );
  const createSlide = (kind: SlideKind) => {
    const before = session.state().slides.length;
    const at = Math.min(session.state().selection.slide + 1, before);

    void session.run({ op: "createSlide", at, kind }).then(() => {
      if (session.state().slides.length <= before) return;
      session.select({ slide: at, block: undefined, range: undefined, text: undefined });
      canvas.focusFresh();
    });
  };
  const addSlide = () => createSlide("title-body");
  const workspace = createWorkspaceFocus({
    changed: () => root.ownerDocument.defaultView?.dispatchEvent(new Event("resize")),
    storage,
  });
  const shortcuts = createShortcuts(session, canvas, {
    addSlide,
    focusCanvas: () => workspace.toggle(),
    canvasFocused: () => workspace.active(),
    nudge: (block, key) => arrange.nudge(block, key),
    present,
  });
  const commands = createCommandPalette(session, canvas, {
    addSlide,
    createSlide,
    focusCanvas: () => workspace.toggle(),
    canvasFocused: () => workspace.active(),
    present,
    audience,
    print,
  });
  const presence = createPresence({
    reload: () => void session.open(),
    saw: (viewers) => session.saw(viewers),
    follow: (seat) => session.follow(seat),
  });
  const share = createShareControl({
    load: () => client.sharing?.() ?? Promise.resolve(null),
  });
  const textBar = createTextBar(
    {
      run,
      done: () => {
        canvas.finishTextSelection();
        session.select({ text: undefined, range: undefined });
      },
    },
    { bodyOf, blocksOf: (slide) => session.blocksOf(slide) },
  );
  canvas.root.querySelector(".slidx-panel-head")!.append(textBar.root);

  const surfaces: Surface[] = [
    createAppbar(
      {
        undo: () => void session.undo(),
        redo: () => void session.redo(),
        present,
        audience,
        print,
      },
      { accessories: [commands.trigger, workspace.root, share.root, presence.root] },
    ),
    outline,
    canvas,
    textBar,
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
    createPanelResize({ storage }),
    createBeacons(),
    presence,
    share,
    commands,
    shortcuts,
  ];
  const frame = element(
    "div",
    { class: "slidx-editor" },
    // These accessory surfaces are already mounted inside their owning chrome.
    // They remain here for state and teardown, but appending their roots again
    // would move them out of that context.
    surfaces
      .filter((surface) => surface !== presence && surface !== share && surface !== textBar)
      .map((surface) => surface.root),
  );
  root.append(frame);
  workspace.connect();

  const unsubscribe = session.subscribe((state) => {
    frame.dataset.access = state.canEdit === false ? "read" : "write";
    for (const surface of surfaces) surface.render(state);
  });

  const keys = (event: KeyboardEvent) => {
    commands.keydown(event);
    if (!event.defaultPrevented) shortcuts.keydown(event);
  };
  const copy = (event: ClipboardEvent) => shortcuts.copy(event);
  const paste = (event: ClipboardEvent) => shortcuts.paste(event);
  root.ownerDocument.addEventListener("keydown", keys);
  root.ownerDocument.addEventListener("copy", copy);
  root.ownerDocument.addEventListener("paste", paste);
  canvas.listen(keys);
  canvas.listenClipboard(copy, paste);

  void session.open();

  return {
    session,
    destroy() {
      unsubscribe();
      root.ownerDocument.removeEventListener("keydown", keys);
      root.ownerDocument.removeEventListener("copy", copy);
      root.ownerDocument.removeEventListener("paste", paste);
      canvasFrame.removeEventListener("load", canvasRendered);
      // Removing the frame is enough for a surface that is only its own DOM.
      // Presence holds a connection, and a connection survives its element.
      for (const surface of surfaces) surface.destroy?.();
      frame.remove();
    },
  };
}
