/**
 * The slide, rendered as it will really look.
 *
 * The canvas is an iframe pointed at the deck's own route, so the preview is
 * the page the build emits, produced by the same WebAssembly module, through
 * the same shell and the same theme. That is why slidx compiled the pipeline to
 * WebAssembly rather than a native addon in the first place — a preview drawn
 * by a second renderer would be a second source of truth about layout, which is
 * exactly the bug this architecture exists to prevent.
 *
 * It also means the deck's CSS is on the other side of a document boundary and
 * cannot reach the tool around it.
 *
 * # What is editable in place
 *
 * Every line of text whose words the pipeline can point at in the file: a
 * heading, a paragraph, a list item, a table cell, and the words inside a mark.
 * The addresses come with the deck — `slidx_edit` reports where each block and
 * each mark is — and [`text`](./text) walks the text a reader sees against them
 * until each run has a byte range. A line whose words are not written anywhere in
 * the source, because a reference or a footnote produced them, gets no range and
 * is not offered for editing at all. Refusing beats guessing: a guess here
 * splices the wrong bytes of somebody's talk.
 *
 * The Markdown view stays for arbitrary constructs, splitting a paragraph, or
 * writing a fence. Common headings, text, lists and quotes start in the visual
 * view; neither path weakens the rule that only the Rust writer composes source.
 *
 * # How a change reaches the page
 *
 * By replacing the slide element inside the frame with the one the route now
 * serves, rather than by reloading the frame — see [`live`](./live). Reloading
 * throws away the caret and the scroll position, which is the whole cost of
 * editing on the canvas: an author changes a word, the frame blinks, and they
 * are somewhere else on the slide. Nothing about the preview is weakened by it;
 * the markup still comes from the deck's own route, which is the page the build
 * emits. A slide with steps still reloads, for the reason `live` gives.
 */

import { element } from "./dom";
import { applyCanvasZoomStyles } from "./canvas-zoom-styles";
import { createContentMenu } from "./content-menu";
import type { BlockSpans } from "./client";
import { BLOCK_ATTRIBUTE } from "./geometry";
import { canPatch, caretIn, patch, restore, type Caret } from "./live";
import type { BlockKind, EditOp } from "./operations";
import type { Surface } from "./outline";
import { createSpeakerNotes } from "./speaker-notes";
import { changeBetween, editableIn, planBlock, rangeOf, type TextPlan } from "./text";

/**
 * Marks a document whose selection is already being reported.
 *
 * The slide is replaced in place when a change lands, so the wiring runs again
 * on a page that already has its listeners — and a second pair would report
 * every selection twice.
 */
const REPORTING_ATTRIBUTE = "data-slidx-reporting";
const SELECTED_ATTRIBUTE = "data-slidx-editor-selected";

/**
 * Editing affordances inside the real deck page.
 *
 * An outline does not change layout, so selecting or typing into a line never
 * nudges the slide. One quiet hairline replaces the browser's thick default
 * focus ring, with enough offset that the words keep breathing room.
 */
export const EDITING_STYLESHEET = `
[contenteditable] {
  outline: 1px solid transparent;
  outline-offset: 6px;
}

[contenteditable]:hover {
  outline-color: color-mix(in srgb, currentColor 18%, transparent);
}

[contenteditable]:focus {
  outline-color: color-mix(in srgb, var(--slidx-color-accent) 52%, transparent);
}

`;

export interface CanvasHandlers {
  run(op: EditOp): void | Promise<void>;
  selected(text: string, at: number): void;
  /** Selects a whole rendered block without making the browser write Markdown. */
  selectedBlock?(block: number | undefined): void;
}

/** The slide as the file has it, which is what a text edit names bytes of. */
export interface EditableSource {
  body(): string;
  blocks(): readonly BlockSpans[];
}

export interface CanvasOptions {
  /** The route the deck is served under, so the frame shows the real page. */
  deckBase: string;
  bodyOf(slide: number): string;
  /** Where the slide's blocks and marks are. Absent leaves the page read-only. */
  blocksOf?(slide: number): readonly BlockSpans[];
  /**
   * Where the scheme choice is remembered. `localStorage` when there is one.
   *
   * Injected rather than reached for, so a test says what was remembered and a
   * document without storage — a sandboxed frame, a browser with it disabled —
   * simply starts on the paper-first default instead of throwing on the first
   * click.
   */
  storage?: Pick<Storage, "getItem" | "setItem"> | undefined;
  /** Keeps every miniature in the outline on the palette the canvas shows. */
  schemeChanged?(scheme: Scheme): void;
  /** Injected so bringing the frame up to date is testable without a server. */
  fetch?: typeof globalThis.fetch;
}

/** Commands a keyboard or another piece of chrome can send to the canvas. */
export interface CanvasSurface extends Surface {
  /** Opens the source view and puts the caret into it. */
  showMarkdown(): void;
  /** Returns to the rendered slide. */
  showVisual(): void;
  /** Returns to the rendered slide and focuses its first editable line. */
  focusText(): void;
  /** Clears the browser selection after contextual text styling is finished. */
  finishTextSelection(): void;
  /** Opens the current slide's speaking surface and puts the caret into it. */
  focusNotes(): void;
  /** Selects the first placeholder once a newly created slide has loaded. */
  focusFresh(): void;
  /** Opens the visual add-content menu. */
  addContent(): void;
  /** Inserts one semantic block and selects its placeholder for immediate typing. */
  insertContent(kind: BlockKind): void;
  /** The light, dark, or automatic palette the author is previewing. */
  scheme(): Scheme;
  /** The palette currently visible after resolving an automatic choice. */
  palette(): Exclude<Scheme, "auto">;
  /** The current multiplier over the largest fully visible slide. */
  zoom(): CanvasZoom;
  zoomIn(): void;
  zoomOut(): void;
  zoomFit(): void;
  /** Receives commands even while focus is inside the preview document. */
  listen(listener: (event: KeyboardEvent) => void): void;
  /** Receives semantic slide copy and paste from the preview document too. */
  listenClipboard(
    copy: (event: ClipboardEvent) => void,
    paste: (event: ClipboardEvent) => void,
  ): void;
}

/**
 * Which palette the canvas shows, and why an author needs to say.
 *
 * A deck follows `prefers-color-scheme` by default, so it shows the room it is
 * in — and in the editor the "room" is the author's laptop. An author working
 * in dark mode therefore never sees the half a lit lecture theatre will project,
 * and one working in daylight never sees the other. Both halves are in the file
 * and only one of them is ever on screen.
 *
 * The override already exists in the output: `slidx_theme::css` emits the dark
 * palette twice, once under the media query and once under `[data-scheme]`, and
 * says out loud that the second is there "so a presenter can override it at the
 * venue when the automatic choice is wrong". Nothing had ever written that
 * attribute — not the editor, not the presenter view, not the runtime.
 *
 * # Why it is not stored in the deck
 *
 * Because it is a viewing choice, not deck content. Writing it into the
 * Markdown would put "how I happened to be looking at this on Tuesday" in the
 * diff, and hand it to a co-editor whose room is different. It lives in this
 * browser and nowhere else.
 */
export type Scheme = "light" | "dark" | "auto";

const SCHEMES: Scheme[] = ["light", "dark", "auto"];

/**
 * What the canvas shows before anybody chooses, and why it is not `auto`.
 *
 * A deck follows `prefers-color-scheme`, which is right for the deck: it is
 * shown in a room, and the room decides. It is wrong for the *editor*, where
 * the "room" is the author's laptop — an author in dark mode was authoring
 * against a palette no lecture theatre will project, and had no way to see the
 * one it will.
 *
 * So the canvas starts light, because a slide is a sheet of paper and that is
 * what an author is making. `auto` is still there, one press away, for the
 * author who wants the canvas to follow their machine — but it is a choice
 * rather than the thing that happens to them.
 *
 * This does not touch what a deck does anywhere else. A built deck, the
 * presenter view and a projector all still follow the room.
 */
const STARTS_AS: Scheme = "light";

const SCHEME_LABEL: Record<Scheme, string> = {
  auto: "Auto",
  light: "Light",
  dark: "Dark",
};

/** Where the choice is remembered, which is this browser and nothing else. */
const SCHEME_KEY = "slidx.canvas.scheme";

/** Multipliers over the largest slide that fits the current canvas. */
export type CanvasZoom = 1 | 1.25 | 1.5 | 2;

const ZOOMS: readonly CanvasZoom[] = [1, 1.25, 1.5, 2];
const ZOOM_KEY = "slidx.canvas.zoom";

/** The remembered detail level, with old or foreign values falling back to fit. */
export function startingZoom(storage: Pick<Storage, "getItem"> | undefined): CanvasZoom {
  const found = Number(storage?.getItem(ZOOM_KEY));

  return ZOOMS.includes(found as CanvasZoom) ? (found as CanvasZoom) : 1;
}

export function startingScheme(storage: Pick<Storage, "getItem"> | undefined): Scheme {
  const found = storage?.getItem(SCHEME_KEY);

  return SCHEMES.includes(found as Scheme) ? (found as Scheme) : STARTS_AS;
}

/**
 * Puts the choice on the deck's own document, or takes it off for `auto`.
 *
 * Removing rather than writing `auto` is the whole of what makes automatic mean
 * automatic: the media query is what decides when no attribute is present, and
 * an attribute spelled `auto` would match neither override and leave the light
 * palette showing in a dark room.
 */
export function applyScheme(document: Document | null | undefined, choice: Scheme): void {
  const root = document?.documentElement;
  if (!root) return;

  if (choice === "auto") root.removeAttribute("data-scheme");
  else root.setAttribute("data-scheme", choice);
}

export function createCanvas(handlers: CanvasHandlers, options: CanvasOptions): CanvasSurface {
  const initialScheme = startingScheme(options.storage);
  const initialZoom = startingZoom(options.storage);
  const frame = element("iframe", { class: "slidx-canvas-frame", title: "Slide preview" });
  const source = element("textarea", {
    class: "slidx-canvas-source",
    spellcheck: false,
    "aria-label": "Slide Markdown",
  });

  const toggle = element("button", { type: "button", class: "slidx-canvas-toggle" }, ["Markdown"]);
  const scheme = element(
    "button",
    {
      type: "button",
      class: "slidx-canvas-scheme",
      title: "Show the slide light, dark, or as this machine is set",
      "data-scheme": initialScheme,
    },
    [SCHEME_LABEL[initialScheme]],
  );
  const content = createContentMenu((kind) => insert(kind));
  const notes = createSpeakerNotes({ run: (op) => handlers.run(op), done: () => frame.focus() });
  const zoomOut = element(
    "button",
    {
      type: "button",
      class: "slidx-canvas-zoom-out",
      "aria-label": "Zoom canvas out",
      title: "Zoom canvas out (⌘/Ctrl −)",
    },
    ["−"],
  ) as HTMLButtonElement;
  const zoomValue = element(
    "button",
    {
      type: "button",
      class: "slidx-canvas-zoom-value",
      "aria-label": "Fit canvas to workspace",
      title: "Fit canvas to workspace (⌘/Ctrl 0)",
    },
    ["Fit"],
  ) as HTMLButtonElement;
  const zoomIn = element(
    "button",
    {
      type: "button",
      class: "slidx-canvas-zoom-in",
      "aria-label": "Zoom canvas in",
      title: "Zoom canvas in (⌘/Ctrl +)",
    },
    ["+"],
  ) as HTMLButtonElement;
  const zoomControl = element(
    "div",
    { class: "slidx-canvas-zoom", role: "group", "aria-label": "Canvas zoom" },
    [zoomOut, zoomValue, zoomIn],
  );
  const tools = element("div", { class: "slidx-canvas-tools" }, [
    content.root,
    zoomControl,
    scheme,
    toggle,
  ]);
  const stage = element("div", { class: "slidx-canvas-stage" }, [frame, source]);
  const root = element("section", { class: "slidx-canvas", "aria-label": "Slide" }, [
    element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["Slide"]), tools]),
    stage,
    notes.root,
  ]);
  applyCanvasZoomStyles(root.ownerDocument);

  let slide = 0;
  let chosen: Scheme = initialScheme;
  let magnification: CanvasZoom = initialZoom;
  let editing = false;
  let shown = "";
  let refresh = 0;
  let keyListener: ((event: KeyboardEvent) => void) | undefined;
  let copyListener: ((event: ClipboardEvent) => void) | undefined;
  let pasteListener: ((event: ClipboardEvent) => void) | undefined;
  let listening: Document | undefined;
  let selectedBlock: number | undefined;
  let canEdit = true;
  /** The placeholder line to focus and select after a visual insertion lands. */
  let focusAfterInsert: { slide: number; block: number; ready: boolean } | undefined;
  /** A newly created slide whose first authored line should replace on typing. */
  let focusFreshOnLoad: { slide: number } | undefined;
  /** Where the author was before the change that is about to land. */
  let held: Caret | undefined;
  let pan = { x: 0.5, y: 0.5 };
  let centerAfterLoad = true;
  let viewportFrame = 0;
  let hasSlides = false;
  let loadedRoute: string | undefined;
  let routeRetry: number | undefined;
  const dismissContent = () => content.close();

  content.setEnabled(options.blocksOf !== undefined);

  function rememberPan(): void {
    const width = stage.scrollWidth;
    const height = stage.scrollHeight;
    if (width > stage.clientWidth) pan.x = (stage.scrollLeft + stage.clientWidth / 2) / width;
    else pan.x = 0.5;
    if (height > stage.clientHeight) pan.y = (stage.scrollTop + stage.clientHeight / 2) / height;
    else pan.y = 0.5;
  }

  function restorePan(): void {
    stage.scrollLeft = Math.max(0, pan.x * stage.scrollWidth - stage.clientWidth / 2);
    stage.scrollTop = Math.max(0, pan.y * stage.scrollHeight - stage.clientHeight / 2);
  }

  function publishViewport(): void {
    const editor = root.closest<HTMLElement>(".slidx-editor");
    if (!editor) return;

    const rect = stage.getBoundingClientRect();
    const viewport = root.ownerDocument.documentElement;
    editor.style.setProperty("--slidx-e-viewport-top", `${Math.max(0, rect.top)}px`);
    editor.style.setProperty(
      "--slidx-e-viewport-right",
      `${Math.max(0, viewport.clientWidth - rect.right)}px`,
    );
    editor.style.setProperty(
      "--slidx-e-viewport-bottom",
      `${Math.max(0, viewport.clientHeight - rect.bottom)}px`,
    );
    editor.style.setProperty("--slidx-e-viewport-left", `${Math.max(0, rect.left)}px`);
  }

  /** Re-measures editor-side handles once after a zoom, pan, or reflow. */
  function viewportChanged(): void {
    if (viewportFrame !== 0) return;
    const view = root.ownerDocument.defaultView;
    if (!view) return;

    viewportFrame = view.requestAnimationFrame(() => {
      viewportFrame = 0;
      publishViewport();
      view.dispatchEvent(new Event("resize"));
    });
  }

  function scrolled(): void {
    rememberPan();
    viewportChanged();
  }

  function showZoom(next: CanvasZoom, remember = true): void {
    rememberPan();
    magnification = next;
    stage.dataset.zoom = next === 1 ? "fit" : String(next);
    stage.style.setProperty("--slidx-e-canvas-zoom", `${next * 100}%`);
    zoomValue.textContent = next === 1 ? "Fit" : `${Math.round(next * 100)}%`;
    zoomValue.title =
      next === 1
        ? "Canvas fits the workspace (⌘/Ctrl 0)"
        : `${Math.round(next * 100)}% · reset to fit (⌘/Ctrl 0)`;
    zoomOut.disabled = next === ZOOMS[0];
    zoomIn.disabled = next === ZOOMS[ZOOMS.length - 1];
    if (remember) options.storage?.setItem(ZOOM_KEY, String(next));
    restorePan();
    viewportChanged();
  }

  function stepZoom(by: -1 | 1): void {
    const at = ZOOMS.indexOf(magnification);
    showZoom(ZOOMS[Math.max(0, Math.min(at + by, ZOOMS.length - 1))]!);
  }

  stage.addEventListener("scroll", scrolled, { passive: true });
  const Resize = root.ownerDocument.defaultView?.ResizeObserver;
  const observer = Resize
    ? new Resize(() => {
        restorePan();
        viewportChanged();
      })
    : undefined;
  observer?.observe(stage);
  zoomOut.addEventListener("click", () => stepZoom(-1));
  zoomValue.addEventListener("click", () => showZoom(1));
  zoomIn.addEventListener("click", () => stepZoom(1));
  showZoom(initialZoom, false);

  function fresh(route: string): string {
    refresh += 1;
    return `${route}?at=${Date.now()}-${refresh}`;
  }

  function clearRouteRetry(): void {
    if (routeRetry === undefined) return;
    root.ownerDocument.defaultView?.clearTimeout(routeRetry);
    routeRetry = undefined;
  }

  /**
   * Reasserts a slide route after the deck's file updates have settled.
   *
   * A whole-slide operation may touch several source files. Vite announces
   * each write to every deck iframe, and one of those reloads can abort the
   * canvas navigation that selected the newly created slide. The `src`
   * attribute still names the right page while the document inside it remains
   * on the old one, so the canvas verifies the document boundary rather than
   * trusting the attribute. Three bounded retries cover that reload wave
   * without leaving a poller behind.
   */
  function expectRoute(route: string, attempt = 0): void {
    clearRouteRetry();
    const view = root.ownerDocument.defaultView;
    if (!view) return;
    const delays = [180, 420, 900] as const;

    routeRetry = view.setTimeout(
      () => {
        routeRetry = undefined;
        if (!hasSlides || route !== routeFor(options.deckBase, slide)) return;
        if (showsRoute(frame.contentDocument?.URL, route, root.ownerDocument.baseURI)) return;

        frame.setAttribute("src", fresh(route));
        if (attempt + 1 < delays.length) expectRoute(route, attempt + 1);
      },
      delays[attempt] ?? delays[delays.length - 1],
    );
  }

  function navigate(route: string): void {
    frame.setAttribute("src", fresh(route));
    if (loadedRoute) expectRoute(route);
    else clearRouteRetry();
  }

  function showSource(next: boolean): void {
    editing = next;
    stage.setAttribute("data-editing", String(editing));
    toggle.textContent = editing ? "Visual" : "Markdown";
    zoomControl.hidden = editing;
    content.setEnabled(canEdit && !editing && options.blocksOf !== undefined);
    if (editing) {
      source.focus();
      return;
    }

    frame.focus();
  }

  function unbindListeners(page: Document | undefined): void {
    if (!page) return;
    if (keyListener) page.removeEventListener("keydown", keyListener);
    if (copyListener) page.removeEventListener("copy", copyListener);
    if (pasteListener) page.removeEventListener("paste", pasteListener);
  }

  function bindListeners(page: Document): void {
    if (listening === page) return;
    unbindListeners(listening);
    if (keyListener) page.addEventListener("keydown", keyListener);
    if (copyListener) page.addEventListener("copy", copyListener);
    if (pasteListener) page.addEventListener("paste", pasteListener);
    listening = page;
  }

  function focusFreshPlaceholder(page: Document): void {
    const target = focusFreshOnLoad;
    if (!target || target.slide !== slide) return;
    const first = page.querySelector<HTMLElement>("[contenteditable]");
    if (!first) return;

    focusAll(page, first);
    // File writes can reload a freshly created slide after it first appears.
    // Keep restoring the selection until the author actually starts using it.
    const used = () => {
      if (focusFreshOnLoad === target) focusFreshOnLoad = undefined;
    };
    first.addEventListener("beforeinput", used, { once: true });
    page.addEventListener("pointerdown", used, { once: true });
  }

  toggle.addEventListener("click", () => showSource(!editing));

  scheme.addEventListener("click", () => {
    content.close();
    chosen = SCHEMES[(SCHEMES.indexOf(chosen) + 1) % SCHEMES.length]!;
    scheme.textContent = SCHEME_LABEL[chosen];
    scheme.setAttribute("data-scheme", chosen);
    options.storage?.setItem(SCHEME_KEY, chosen);
    applyScheme(frame.contentDocument, chosen);
    options.schemeChanged?.(chosen);
  });

  /** Inserts beside the selection, then puts the new placeholder under the caret. */
  async function insert(kind: BlockKind): Promise<void> {
    if (!canEdit) return;
    const targetSlide = slide;
    const before = options.blocksOf?.(targetSlide) ?? [];
    const at =
      selectedBlock === undefined ? before.length : Math.min(selectedBlock + 1, before.length);
    const target = { slide: targetSlide, block: at, ready: false };
    focusAfterInsert = target;

    await handlers.run({ op: "insertBlock", slide: targetSlide, at, kind });

    const after = options.blocksOf?.(targetSlide) ?? [];
    if (after.length <= before.length) {
      if (focusAfterInsert === target) focusAfterInsert = undefined;
      return;
    }

    target.ready = true;
    if (slide === targetSlide) handlers.selectedBlock?.(at);
  }

  // On blur rather than on every keystroke: an operation per character would
  // write the file per character, and the undo stack is a list of operations.
  source.addEventListener("blur", () => {
    if (!canEdit || source.value === shown) return;
    void handlers.run({ op: "setBody", slide, body: source.value });
  });

  /** Makes the page in the frame editable, and puts the caret back if it moved. */
  function bind(): void {
    const page = frame.contentDocument;
    if (!page) return;

    bindListeners(page);

    const lines = attachEditing(
      page,
      slide,
      handlers,
      {
        body: () => options.bodyOf(slide),
        blocks: () => options.blocksOf?.(slide) ?? [],
      },
      canEdit,
    );
    markSelected(page, selectedBlock);
    page.addEventListener("pointerdown", dismissContent);

    if (focusAfterInsert?.slide === slide) {
      const target = focusAfterInsert;
      const inserted = target.block;
      const line = page.querySelector<HTMLElement>(
        `[${BLOCK_ATTRIBUTE}="${inserted}"] [contenteditable]`,
      );
      // A session update may render before the replacement page arrives. Keep
      // the request until the authored placeholder is actually in this frame.
      if (line) {
        focusAll(page, line);
        // Vite can follow the live replacement with one real frame reload. A
        // reload before the author types must restore this caret; the first
        // editing gesture retires the request so later renders never reselect
        // authored text.
        const used = () => {
          if (focusAfterInsert === target) focusAfterInsert = undefined;
        };
        line.addEventListener("beforeinput", used, { once: true });
        line.addEventListener("pointerdown", used, { once: true });
      }
    }

    focusFreshPlaceholder(page);

    const wanted = held;
    held = undefined;
    if (!wanted) return;

    const block = page.querySelector(`[${BLOCK_ATTRIBUTE}="${wanted.block}"]`);
    // The line may have gone — a block deleted, or a paragraph that stopped
    // being addressable. Then the caret has nowhere to be, which is what a
    // reload would have done anyway.
    const line = block ? [...block.querySelectorAll("[contenteditable]")][wanted.line] : undefined;
    if (line && lines.includes(line)) restore(page, line, wanted.at);
  }

  // Re-applied on every load, because the attribute lives on a document the
  // frame replaces whenever it navigates to another slide. Setting it once
  // would give an author one slide in the palette they asked for and the rest
  // in the one their laptop is set to.
  frame.addEventListener("load", () => {
    // Tests author iframe fixtures in place and deliberately remove `src`; a
    // real deck frame always has one, so it still gets strict route recovery.
    const fixture = !frame.hasAttribute("src");
    if (!hasSlides && !fixture) return;
    const route = routeFor(options.deckBase, slide);
    const documentURL = frame.contentDocument?.URL;
    if (!fixture && !showsRoute(documentURL, route, root.ownerDocument.baseURI)) {
      if (loadedRoute) navigate(route);
      return;
    }
    if (!fixture) loadedRoute = route;
    clearRouteRetry();
    applyScheme(frame.contentDocument, chosen);
    bind();
    viewportChanged();
    if (centerAfterLoad) {
      centerAfterLoad = false;
      pan = { x: 0.5, y: 0.5 };
      root.ownerDocument.defaultView?.requestAnimationFrame(restorePan);
    }
  });

  return {
    root,
    showMarkdown: () => showSource(true),
    showVisual: () => showSource(false),
    focusText() {
      showSource(false);
      if (!canEdit) {
        frame.focus();
        return;
      }
      const line = frame.contentDocument?.querySelector<HTMLElement>("[contenteditable]");
      line?.focus();
    },
    finishTextSelection() {
      frame.contentDocument?.getSelection()?.removeAllRanges();
      frame.focus();
    },
    focusNotes: () => notes.focus(),
    focusFresh() {
      if (!canEdit) return;
      showSource(false);
      focusFreshOnLoad = { slide };
      const page = frame.contentDocument;
      const route = routeFor(options.deckBase, slide);
      if (
        page &&
        (!frame.hasAttribute("src") || showsRoute(page.URL, route, root.ownerDocument.baseURI))
      ) {
        focusFreshPlaceholder(page);
      }
    },
    addContent() {
      if (canEdit && !editing) content.open();
    },
    insertContent: (kind) => insert(kind),
    scheme: () => chosen,
    palette: () =>
      chosen === "auto"
        ? frame.contentWindow?.matchMedia?.("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light"
        : chosen,
    zoom: () => magnification,
    zoomIn: () => stepZoom(1),
    zoomOut: () => stepZoom(-1),
    zoomFit: () => showZoom(1),
    listen(listener) {
      unbindListeners(listening);
      keyListener = listener;
      const page = frame.contentDocument;
      listening = undefined;
      if (page) bindListeners(page);
    },
    listenClipboard(copy, paste) {
      unbindListeners(listening);
      copyListener = copy;
      pasteListener = paste;
      const page = frame.contentDocument;
      listening = undefined;
      if (page) bindListeners(page);
    },
    destroy() {
      unbindListeners(listening);
      content.destroy();
      observer?.disconnect();
      stage.removeEventListener("scroll", scrolled);
      if (viewportFrame !== 0) root.ownerDocument.defaultView?.cancelAnimationFrame(viewportFrame);
      clearRouteRetry();
      frame.contentDocument?.removeEventListener("pointerdown", dismissContent);
      listening = undefined;
      keyListener = undefined;
      copyListener = undefined;
      pasteListener = undefined;
    },
    render(state) {
      const nextCanEdit = state.canEdit !== false;
      const accessChanged = canEdit !== nextCanEdit;
      canEdit = nextCanEdit;
      source.readOnly = !canEdit;
      content.setEnabled(canEdit && !editing && options.blocksOf !== undefined);
      notes.render(state);
      const moved = state.selection.slide !== slide;
      if (moved) centerAfterLoad = true;
      slide = state.selection.slide;
      if (moved && focusAfterInsert?.slide !== slide) focusAfterInsert = undefined;
      if (moved && focusFreshOnLoad?.slide !== slide) focusFreshOnLoad = undefined;
      if (
        focusAfterInsert?.ready &&
        handlers.selectedBlock &&
        (focusAfterInsert.slide !== slide || focusAfterInsert.block !== state.selection.block)
      ) {
        focusAfterInsert = undefined;
      }
      selectedBlock = state.selection.block;
      const page = frame.contentDocument;
      if (page) {
        markSelected(page, selectedBlock);
        if (accessChanged) bind();
      }

      const body = options.bodyOf(slide);
      // Only when it differs, so a re-render does not take the cursor away
      // from an author in the middle of a word.
      if (body !== shown) {
        shown = body;
        source.value = body;
      }

      // The empty state is drawn before the first deck response arrives. A
      // shared editor uses that response to establish browser-held read access
      // for the real slide route, so navigating the iframe any earlier would
      // be both a guaranteed 403 and a misleading console error.
      if (state.slides.length === 0) {
        hasSlides = false;
        loadedRoute = undefined;
        clearRouteRetry();
        frame.removeAttribute("src");
        frame.removeAttribute("data-source");
        return;
      }
      hasSlides = true;

      const next = routeFor(options.deckBase, slide);
      if (!moved && frame.getAttribute("data-source") === state.source) return;

      frame.setAttribute("data-source", state.source);

      // A slide the author is typing on keeps its document. Everything else —
      // another slide, a staged one, a page that cannot be read — reloads,
      // which is always correct and only costs the caret.
      if (!moved && canPatch(frame, state.slides[slide]?.stopCount ?? 1)) {
        held = caretIn(frame.contentDocument!);

        void patch(frame, next, options.fetch).then((patched) => {
          if (!patched) {
            navigate(next);
            return;
          }

          // Everything that measures inside this frame waits for a load — the
          // lines that become editable, the grips the arrange overlay draws,
          // the handles the resize overlay draws — and a slide replaced in
          // place never loads. Saying so once, through the frame, is what keeps
          // the overlays off the boxes the slide had before the change; an
          // overlay added later needs nothing here.
          frame.dispatchEvent(new Event("load"));
        });

        return;
      }

      navigate(next);
    },
  };
}

/** Focuses a fresh placeholder and selects it, so the next keystroke replaces it. */
function focusAll(document: Document, line: HTMLElement): void {
  line.focus();
  const selection = document.getSelection();
  if (!selection) return;

  const range = document.createRange();
  range.selectNodeContents(line);
  selection.removeAllRanges();
  selection.addRange(range);
}

/** The deck's own URL for one slide. Slides are one-based in a URL. */
export function routeFor(base: string, slide: number): string {
  const prefix = base ? `/${base}` : "";

  return slide === 0 ? `${prefix}/` : `${prefix}/${slide + 1}/`;
}

/** Whether the document inside a frame is on the slide its `src` names. */
export function showsRoute(documentURL: string | undefined, route: string, base: string): boolean {
  if (!documentURL) return false;

  try {
    return new URL(documentURL, base).pathname === new URL(route, base).pathname;
  } catch {
    return false;
  }
}

/**
 * Makes the rendered slide answer to a cursor, and says which lines it opened.
 *
 * Takes a document rather than the frame so it can be driven without one. The
 * source is read now rather than at commit time, because now is the moment the
 * page and the file are known to agree: this runs on the load, or on the swap,
 * that the render caused.
 */
export function attachEditing(
  document: Document,
  slide: number,
  handlers: CanvasHandlers,
  source: EditableSource,
  canEdit = true,
): Element[] {
  const body = document.querySelector(".slidx-slide-body");
  if (!body) return [];
  applyEditingStyles(document);

  const text = source.body();
  const blocks = source.blocks();
  const opened: Element[] = [];

  for (const wrapper of body.querySelectorAll(`[${BLOCK_ATTRIBUTE}]`)) {
    const index = Number(wrapper.getAttribute(BLOCK_ATTRIBUTE));
    const spans = Number.isInteger(index) ? blocks[index] : undefined;
    if (spans === undefined) continue;

    for (const [line, plan] of planBlock(text, spans, editableIn(wrapper))) {
      if (canEdit) {
        open(line, plan, slide, handlers);
        opened.push(line);
      } else {
        line.removeAttribute("contenteditable");
      }
    }
  }

  // The body is looked up again on every report rather than closed over: the
  // slide is replaced in place when a change lands, and an offset measured
  // against the element that used to be there is measured against nothing.
  const report = () => {
    const selection = document.getSelection();
    const text = selection?.toString() ?? "";
    const showing = document.querySelector(".slidx-slide-body");
    if (text.trim().length === 0 || !showing) return;

    handlers.selected(text, offsetIn(showing, selection!));
  };

  // Once per document, not once per swap: the listeners are on the document and
  // the slide inside it is what gets replaced.
  if (!document.body.hasAttribute(REPORTING_ATTRIBUTE)) {
    document.body.setAttribute(REPORTING_ATTRIBUTE, "");
    document.addEventListener("pointerdown", (event) => {
      // The page is an iframe, so its Element constructor is not the editor
      // window's Element constructor. `instanceof Element` works in a synthetic
      // document and fails in the real canvas. `closest` is the capability this
      // gesture needs and crosses that realm boundary without guessing.
      const target = event.target as { closest?(selector: string): Element | null } | null;
      const wrapper = target?.closest?.(`[${BLOCK_ATTRIBUTE}]`);
      const index = Number(wrapper?.getAttribute(BLOCK_ATTRIBUTE));
      handlers.selectedBlock?.(Number.isInteger(index) ? index : undefined);
    });
    document.addEventListener("mouseup", report);
    document.addEventListener("keyup", report);
  }

  return opened;
}

/** Marks one block in the editor-only copy of the deck page. */
function markSelected(document: Document, selected: number | undefined): void {
  for (const block of document.querySelectorAll(`[${SELECTED_ATTRIBUTE}]`)) {
    block.removeAttribute(SELECTED_ATTRIBUTE);
  }

  if (selected === undefined) return;
  document
    .querySelector(`[${BLOCK_ATTRIBUTE}="${selected}"]`)
    ?.setAttribute(SELECTED_ATTRIBUTE, "");
}

/** Puts the editor-only hairlines into one canvas document, once. */
function applyEditingStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-editing]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-editing", "");
  style.textContent = EDITING_STYLESHEET;
  document.head.append(style);
}

/**
 * Lets one line be typed in, and commits what it says when it is left.
 *
 * On blur rather than on every keystroke, for the reason the Markdown view has:
 * an operation per character would write the file per character, and the undo
 * stack is a list of operations.
 *
 * Nothing intercepts a paste. Only the line's text is ever read, so markup a
 * browser drops into it changes nothing that is sent and is gone on the next
 * render — and an editor that fought the clipboard would break dictation and an
 * input method with it.
 */
function open(line: Element, plan: TextPlan, slide: number, handlers: CanvasHandlers): void {
  line.setAttribute("contenteditable", "true");

  line.addEventListener("keydown", (event) => {
    // Enter commits rather than starting a second line: this is one line of the
    // file, and a second one here would have nowhere to go. Splitting a
    // paragraph is still a thing the Markdown view does.
    if ((event as KeyboardEvent).key !== "Enter") return;

    event.preventDefault();
    commit(line, plan, slide, handlers);
  });

  line.addEventListener("blur", () => commit(line, plan, slide, handlers));
}

function commit(line: Element, plan: TextPlan, slide: number, handlers: CanvasHandlers): void {
  const change = changeBetween(plan.plain, line.textContent ?? "");
  if (change === undefined) return;

  // A line emptied by accident is left alone. Deleting a block is an operation
  // of its own, and a stray Backspace on the last word of a heading should not
  // be the way to reach it.
  if (line.textContent?.trim().length === 0) return;

  const range = rangeOf(plan, change);
  if (range === undefined) return;

  void handlers.run({ op: "setText", slide, range, text: change.text });
}

/**
 * How far into the rendered slide a selection starts.
 *
 * Counted in the text a reader sees, which is what decides *which* appearance
 * of a repeated phrase was picked.
 */
function offsetIn(body: Element, selection: Selection): number {
  const range = selection.getRangeAt(0).cloneRange();
  range.selectNodeContents(body);
  range.setEnd(selection.getRangeAt(0).startContainer, selection.getRangeAt(0).startOffset);

  return range.toString().length;
}
