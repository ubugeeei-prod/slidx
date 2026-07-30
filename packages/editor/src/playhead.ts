/**
 * Putting the canvas on a stop.
 *
 * Scrubbing is the whole reason a timeline over this model is worth building.
 * Each stop is a complete compiled snapshot rather than a delta, so there is no
 * animation state machine to run forward and no build to replay: showing stop
 * seven is an index, and it costs the same going backwards.
 *
 * That only holds if the frame is not reloaded, so the first way this asks is
 * the channel the presenter view already uses to keep a projector on the same
 * stop — one message, no request, no repaint of the page. Where the browser has
 * no channel, it falls back to `?step=N` on the frame's own URL, which is the
 * deck's own deep link and therefore the one path that cannot be wrong. That
 * reloads the frame, so it is the slower answer rather than the first one.
 *
 * The stop itself is tracked by `createStopCursor` — the runtime's DOM-free
 * stage — so clamping an out-of-range stop is decided in the one place that
 * already decides it for the deck, the presenter view, and every deep link.
 */

import {
  createMirror,
  createStopCursor,
  type MirrorTransport,
  type Stage,
} from "@ubugeeei/slidx-runtime";

export interface Playhead {
  /** The stop currently shown. */
  readonly stop: number;
  /** Stops on the slide, including the resting frame. */
  readonly stops: number;
  /**
   * False when no channel exists, so the surface can say the canvas reloads
   * rather than leaving an author to wonder why scrubbing flickers.
   */
  readonly mirrored: boolean;
  /** Points the playhead at a slide of `stops` stops, keeping it in range. */
  moveTo(slide: number, stops: number): number;
  /** Shows a stop, clamped to one the slide has. Returns the stop shown. */
  show(stop: number): number;
  /** Shows the stop `by` presses away. */
  step(by: number): number;
  /** Re-sends the current stop, for a frame that has just reloaded. */
  resend(): void;
}

export interface PlayheadOptions {
  /**
   * The canvas frame, looked up when it is needed rather than held.
   *
   * The canvas owns that element and replaces its `src` on every edit, so a
   * reference taken once would go stale. Passing a lookup keeps the two surfaces
   * from having to know about each other beyond the stop being shown.
   */
  frame?: () => HTMLIFrameElement | null;
  /** Pass `null` to disable mirroring, or a transport to substitute one. */
  transport?: MirrorTransport | null;
  /** Substituted in tests; otherwise the runtime's own clamping stage. */
  cursor?: (stops: number) => Stage;
}

export function createPlayhead(options: PlayheadOptions = {}): Playhead {
  const cursor = options.cursor ?? createStopCursor;
  // Never closed, for the same reason the deck's own page never closes its
  // channel: it lives as long as the document that opened it.
  const mirror = createMirror(
    options.transport === undefined ? {} : { transport: options.transport },
  );

  let slide = 0;
  let stage = cursor(1);

  function announce(): void {
    if (mirror.available) {
      mirror.send({ slide, step: stage.index });
      return;
    }

    const frame = options.frame?.() ?? null;
    const src = frame?.getAttribute("src");
    if (frame && src) frame.setAttribute("src", withStop(src, stage.index));
  }

  return {
    get stop() {
      return stage.index;
    },
    get stops() {
      return stage.stopCount;
    },
    get mirrored() {
      return mirror.available;
    },

    moveTo(next, stops) {
      // A new slide starts at its first stop; the same slide keeps the stop the
      // author was looking at, because an edit to stop four should leave them
      // looking at stop four.
      const keep = next === slide ? stage.index : 0;
      slide = next;
      stage = cursor(stops);

      return stage.apply(keep);
    },

    show(stop) {
      const shown = stage.apply(stop);
      announce();
      return shown;
    },

    step(by) {
      return this.show(stage.index + by);
    },

    resend: announce,
  };
}

/**
 * A frame URL pointing at one stop.
 *
 * `?step=0` is left off, matching how the deck writes its own URLs: a link
 * someone is about to share should not carry the stop it opens at anyway.
 *
 * The base is invented and thrown away — the frame's `src` is relative to the
 * editor's page and has to stay that way.
 */
export function withStop(src: string, stop: number): string {
  const url = new URL(src, "http://slidx.invalid/");

  if (stop === 0) url.searchParams.delete("step");
  else url.searchParams.set("step", String(stop));

  return `${url.pathname}${url.search}`;
}
