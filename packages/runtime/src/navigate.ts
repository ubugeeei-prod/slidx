/**
 * Moving through a deck.
 *
 * A deck is multi-page HTML, so "next" means one of two things and this module
 * is the only place that knows which: advance a stop on this page, or leave
 * for another page. Everything else — the keyboard, a mirror, a deep link —
 * goes through the same two operations, so they cannot disagree about where
 * a boundary is.
 *
 * Leaving the page is a real navigation rather than a route change. That is
 * what keeps a slide URL shareable, indexable, and openable with no script,
 * and it is why there is no router here.
 */

import type { Position } from "./mirror";
import type { Stage } from "./stage";

/** The end of a slide, as a deep link can name it. */
export const LAST_STEP = "last";

export interface Navigator {
  /** The stop currently shown. */
  readonly step: number;
  next(): void;
  previous(): void;
  first(): void;
  last(): void;
  /** Shows a position, following it off this page if it names another slide. */
  show(position: Position): void;
  /** Handles a key event. Consumes only the keys it acts on. */
  handleKey(event: KeyboardEvent): void;
  /** Announced on every move this window originates. */
  subscribe(handler: (position: Position) => void): () => void;
}

export interface NavigatorOptions {
  stage: Stage;
  /** Zero-based index of the slide this page shows. */
  slide: number;
  slideCount: number;
  /** Stop to open at. `"last"` opens at the end. */
  step?: number | typeof LAST_STEP | undefined;
  /** URL of a slide, optionally at a stop. */
  hrefFor: (slide: number, step?: number | typeof LAST_STEP) => string;
  /** Leaves for another page. Injected so tests need no real navigation. */
  navigate?: (href: string) => void;
  /** Rewrites the current URL without adding history. */
  replaceUrl?: (href: string) => void;
}

/** Keys a presentation remote sends, plus the ones a keyboard user expects. */
const FORWARD = new Set(["ArrowRight", "ArrowDown", "PageDown", " ", "Enter"]);
const BACKWARD = new Set(["ArrowLeft", "ArrowUp", "PageUp", "Backspace"]);

export function createNavigator(options: NavigatorOptions): Navigator {
  const { stage, slide, slideCount, hrefFor } = options;

  const navigate = options.navigate ?? ((href: string) => globalThis.location.assign(href));
  const replaceUrl =
    options.replaceUrl ?? ((href: string) => globalThis.history?.replaceState(null, "", href));

  const handlers = new Set<(position: Position) => void>();

  // A stop the slide no longer has must land somewhere real: links outlive
  // the edit that shortened a slide.
  const opening = options.step === LAST_STEP ? stage.stopCount - 1 : Math.max(0, options.step ?? 0);
  let step = stage.apply(opening);

  /** Shows a stop on this page, and tells anyone listening. */
  const go = (next: number, announce: boolean) => {
    const applied = stage.apply(next);
    if (applied === step) return;

    step = applied;
    syncUrl();
    if (announce) {
      for (const handler of handlers) handler({ slide, step });
    }
  };

  /**
   * Keeps the URL on the current stop.
   *
   * `replaceState` rather than `pushState`: a slide with eight builds would
   * otherwise put eight entries in the history, and the back button would
   * walk the build instead of leaving the slide.
   */
  const syncUrl = () => {
    // `?step=0` is noise in a URL someone is about to share.
    replaceUrl(hrefFor(slide, step === 0 ? undefined : step));
  };

  return {
    get step() {
      return step;
    },

    next() {
      if (step < stage.stopCount - 1) return go(step + 1, true);
      if (slide + 1 < slideCount) navigate(hrefFor(slide + 1));
    },

    previous() {
      if (step > 0) return go(step - 1, true);
      // The *end* of the previous slide. Landing on its start would replay a
      // build the audience already watched.
      if (slide > 0) navigate(hrefFor(slide - 1, LAST_STEP));
    },

    first: () => go(0, true),
    last: () => go(stage.stopCount - 1, true),

    show(position) {
      if (position.slide !== slide) {
        navigate(hrefFor(position.slide, position.step === 0 ? undefined : position.step));
        return;
      }
      // Not announced: echoing a position back to whoever sent it would make
      // two windows volley one move forever.
      go(position.step, false);
    },

    handleKey(event) {
      // A modifier means the browser's shortcut, not ours. Cmd-ArrowRight is
      // "forward in history", and a deck that eats it is a deck the browser
      // no longer works in.
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (isTyping(event.target)) return;

      if (FORWARD.has(event.key)) this.next();
      else if (BACKWARD.has(event.key)) this.previous();
      else if (event.key === "Home") this.first();
      else if (event.key === "End") this.last();
      else return;

      event.preventDefault();
    },

    subscribe(handler) {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
}

/**
 * True when the key belongs to a field rather than to the deck.
 *
 * The editor and the audience channel both put inputs on the page, and a
 * space bar that advances the slide while someone is typing a question is a
 * bug they will report as the deck being haunted.
 */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;

  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}
