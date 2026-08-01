/**
 * The deck as a visual sequence.
 *
 * Jump, insert, remove, reorder. Reordering is a `moveSlide` operation and not
 * a rewrite — the slide's bytes are the ones that were already in the file, so
 * a deck reordered from here diffs as moved lines. That is the whole reason
 * the operation exists rather than the editor sending a new body.
 *
 * Each card carries the deck's real page in a lazy iframe. It is not a sketch
 * of a slide and it is not a browser-side thumbnail renderer: the miniature is
 * the same route, renderer, theme, fonts and layout that the canvas opens. That
 * makes the outline useful for judging rhythm, density and colour across the
 * whole deck instead of merely navigating a list of titles.
 *
 * Rows are reconciled rather than rebuilt on every session update. Replacing a
 * thumbnail iframe when only the selection changed would make the outline
 * flash, lose lazy-loading progress and reload the deck once per slide on every
 * keystroke. A slide id is the stable identity; duplicate ids are disambiguated
 * in source order without pretending the index itself is identity.
 *
 * Diagnostics are shown on the card they belong to. The pipeline returns them
 * with every parse, so the outline is where an author sees which slide has a
 * problem without opening it.
 */

import { element } from "./dom";
import type { SlideSummary } from "./client";
import type { Scheme } from "./canvas";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";
import { createSlideMenu } from "./slide-menu";

export interface OutlineHandlers {
  select(slide: number): void;
  run(op: EditOp): void | Promise<void>;
  /** Continues a successful creation gesture into editing its first placeholder. */
  created?(): void;
}

export interface OutlineOptions {
  /** The real deck page for one slide. Absent keeps embedders text-only. */
  preview?(slide: number): string;
  /** The viewing palette shared with the main canvas. */
  scheme?: Scheme;
}

export interface Surface {
  root: HTMLElement;
  render(state: EditorState): void;
  /**
   * Let go of anything that outlives the element.
   *
   * Optional because most surfaces are their own DOM and nothing else: removing
   * the frame is all the teardown they need. It exists for the one that holds a
   * connection, which a removed element does not close.
   */
  destroy?(): void;
}

export interface OutlineSurface extends Surface {
  /** Changes every loaded miniature without changing deck content. */
  showScheme(scheme: Scheme): void;
}

export function createOutline(
  handlers: OutlineHandlers,
  options: OutlineOptions = {},
): OutlineSurface {
  const list = element("ol", { class: "slidx-outline-list" });
  let count = 0;
  let selected = 0;
  const add = createSlideMenu(async (kind) => {
    const before = count;
    const at = Math.min(selected + 1, count);

    await handlers.run({ op: "createSlide", at, kind });
    if (count > before) {
      handlers.select(at);
      handlers.created?.();
    }
  });
  const root = element("section", { class: "slidx-outline", "aria-label": "Slides" }, [
    element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["Slides"]), add.root]),
    list,
  ]);

  let dragging: number | undefined;
  let rows = new Map<string, OutlineRow>();
  let previousSelected: number | undefined;
  let scheme: Scheme = options.scheme ?? "light";

  return {
    root,
    destroy: () => add.destroy(),
    showScheme(next) {
      scheme = next;
      for (const found of rows.values()) found.showScheme(next);
    },
    render(state) {
      count = state.slides.length;
      selected = state.selection.slide;

      const next = new Map<string, OutlineRow>();
      const identities = rowIdentities(state.slides);
      const rendered = state.slides.map((slide, index) => {
        const identity = identities[index]!;
        const found =
          rows.get(identity) ??
          row(handlers, options, () => scheme, {
            start: (from) => {
              dragging = from;
            },
            drop: (to) => {
              if (dragging === undefined || dragging === to) return;
              void handlers.run({ op: "moveSlide", slide: dragging, to });
              dragging = undefined;
            },
            end: () => {
              dragging = undefined;
            },
          });

        next.set(identity, found);
        return { found, slide, index };
      });

      list.replaceChildren(...rendered.map(({ found }) => found.root));
      for (const { found, slide, index } of rendered) found.render(slide, index, state);
      if (previousSelected !== selected) {
        rendered[selected]?.found.root.scrollIntoView?.({ block: "nearest" });
        previousSelected = selected;
      }
      rows = next;
    },
  };
}

interface DragHandlers {
  start(from: number): void;
  drop(to: number): void;
  end(): void;
}

interface OutlineRow {
  root: HTMLElement;
  showScheme(scheme: Scheme): void;
  render(slide: SlideSummary, index: number, state: EditorState): void;
}

function row(
  handlers: OutlineHandlers,
  options: OutlineOptions,
  scheme: () => Scheme,
  drag: DragHandlers,
): OutlineRow {
  let index = 0;
  const number = element("span", { class: "slidx-outline-number", "aria-hidden": "true" });
  const title = element("span", { class: "slidx-outline-title", "aria-hidden": "true" });
  const dot = element("span", { class: "slidx-dot", "aria-hidden": "true" });
  const caption = element("div", { class: "slidx-outline-caption" }, [number, title, dot]);

  const thumbnail = element("span", { class: "slidx-outline-thumbnail", "aria-hidden": "true" });
  const frame = options.preview
    ? element("iframe", {
        class: "slidx-outline-frame",
        loading: "lazy",
        tabindex: -1,
        "aria-hidden": "true",
      })
    : undefined;
  if (frame) {
    // The editor canvas deliberately starts as paper even when the machine is
    // dark. The overview must tell the same visual truth or a white slide in
    // the canvas becomes a dark slide one panel away.
    frame.addEventListener("load", () => {
      try {
        // Vite's document reload can briefly send sibling iframes to the page
        // that changed rather than the route their card owns. The `src`
        // attribute still says the right thing, so a normal render sees no
        // difference and leaves a slide-two caption over a slide-one picture.
        // Reassert the route at the document boundary where the mismatch is
        // observable, and do not colour the wrong page on its way out.
        if (recoverPreviewRoute(frame)) return;
        applyThumbnailScheme(frame.contentDocument, scheme());
      } catch {
        // An embed may serve its preview across an origin. It still gets the
        // real page; only this local viewing preference cannot cross the seam.
      }
    });
    thumbnail.append(frame);
  } else {
    thumbnail.setAttribute("data-empty", "true");
  }

  // A real button overlays the card rather than wrapping the iframe. Interactive
  // content inside a button is invalid and gives keyboard navigation two focus
  // stops for what is one action. The miniature never receives input.
  const open = element("button", { type: "button", class: "slidx-outline-open" });

  const remove = element(
    "button",
    { type: "button", class: "slidx-outline-remove", "aria-label": `Remove slide ${index + 1}` },
    ["×"],
  );
  const duplicate = element(
    "button",
    {
      type: "button",
      class: "slidx-outline-duplicate",
      "aria-label": `Duplicate slide ${index + 1}`,
      title: "Duplicate slide",
    },
    ["⧉"],
  );

  const actions = element("div", { class: "slidx-outline-actions" }, [duplicate, remove]);
  const item = element(
    "li",
    {
      class: "slidx-outline-row",
      draggable: true,
    },
    [thumbnail, caption, open, actions],
  );

  open.addEventListener("click", () => handlers.select(index));
  duplicate.addEventListener("click", () => {
    void handlers.run({ op: "duplicateSlide", slide: index });
  });
  remove.addEventListener("click", () => {
    void handlers.run({ op: "removeSlide", slide: index });
  });

  item.addEventListener("dragstart", () => drag.start(index));
  item.addEventListener("dragover", (event) => event.preventDefault());
  item.addEventListener("drop", (event) => {
    event.preventDefault();
    drag.drop(index);
  });
  item.addEventListener("dragend", () => drag.end());

  return {
    root: item,
    showScheme: (next) => applyThumbnailScheme(frame?.contentDocument, next),
    render(slide, nextIndex, state) {
      index = nextIndex;
      const label = slide.title ?? `Slide ${index + 1}`;
      const worst = severityOn(state, index);
      const route = options.preview?.(index);

      number.textContent = String(index + 1);
      title.textContent = label;
      dot.hidden = worst === undefined;
      dot.title = worst ?? "";
      open.setAttribute("aria-label", `${index + 1} ${label}`);
      remove.setAttribute("aria-label", `Remove slide ${index + 1}`);
      duplicate.setAttribute("aria-label", `Duplicate slide ${index + 1}`);
      item.setAttribute("aria-current", String(index === state.selection.slide));
      item.setAttribute("data-slide", String(index));
      if (worst) item.setAttribute("data-severity", worst);
      else item.removeAttribute("data-severity");

      if (frame && route) {
        frame.title = `Preview of slide ${index + 1}: ${label}`;
        frame.dataset.preview = route;
        // A detached surface is a DOM test or an embed being assembled. Giving
        // its iframe a URL starts a network request even though nobody can see
        // it, and some DOM implementations do so before the element is ever
        // connected. The first connected render starts the real lazy load.
        if (item.isConnected && frame.getAttribute("src") !== route) frame.src = route;
      }
    },
  };
}

/** Restores a miniature that a document-wide dev reload moved off its own route. */
export function recoverPreviewRoute(frame: HTMLIFrameElement): boolean {
  const route = frame.dataset.preview;
  const showing = frame.contentDocument?.URL;
  if (!route || !showing) return false;

  const wanted = new URL(route, frame.ownerDocument.baseURI);
  const current = new URL(showing, frame.ownerDocument.baseURI);
  if (wanted.origin === current.origin && wanted.pathname === current.pathname) return false;

  frame.src = route;
  return true;
}

/** Makes an overview miniature agree with the canvas viewing choice. */
export function applyThumbnailScheme(document: Document | null | undefined, scheme: Scheme): void {
  const root = document?.documentElement;
  if (!root) return;

  if (scheme === "auto") root.removeAttribute("data-scheme");
  else root.setAttribute("data-scheme", scheme);
}

/** Stable row keys, including the uncommon but valid case of repeated ids. */
function rowIdentities(slides: readonly SlideSummary[]): string[] {
  const seen = new Map<string, number>();

  return slides.map((slide) => {
    const occurrence = seen.get(slide.id) ?? 0;
    seen.set(slide.id, occurrence + 1);
    return `${slide.id}\u0000${occurrence}`;
  });
}

/** The worst thing the linter said about one slide, or nothing. */
function severityOn(state: EditorState, index: number): string | undefined {
  const found = state.diagnostics.filter((finding) => finding.slideIndex === index);
  if (found.length === 0) return undefined;

  return found.some((finding) => finding.severity === "error") ? "error" : "warning";
}
