/**
 * Switching a dead demo for the recording of it working.
 *
 * A live demo is the best thing in a talk and the most likely thing to fail.
 * The recovery everyone actually performs — alt-tab, hunt for the file, open a
 * player, apologise — costs the room two minutes and costs the speaker their
 * place in the talk. This module exists to make that one key.
 *
 * Three decisions carry the feature, and each of them is about the moment the
 * switch is used rather than about the switch itself:
 *
 * **The recording is loaded before it is needed.** The moment a fallback
 * becomes necessary is the moment the network stopped working, so a fallback
 * that has to be fetched then is not a fallback — it is a second thing that
 * fails. `slidx_render` ships both sides in the markup with `preload="auto"`,
 * and this module additionally calls `load()` on a recording the browser has
 * not started, because `preload` is a hint a browser is free to ignore on a
 * metered or low-battery machine. The one place a talk cannot afford that
 * heuristic is here.
 *
 * **Switching is one attribute write.** No element is created, no file is
 * requested, no promise has to resolve. Both sides are already laid out at the
 * same size, so the recording appears exactly where the live demo was. There
 * is nothing left in the path that can reject.
 *
 * **It never claims more than it has.** `ready()` reports what the browser
 * actually buffered rather than what the markup asked for, so a presenter view
 * can say "the fallback for slide 7 has not loaded" while there is still time
 * to care. A switch that reported success and then showed a spinner would be
 * worse than no fallback at all, because the speaker would have stopped
 * checking.
 *
 * Which side to show is deliberately the speaker's decision and not a probe's.
 * A request that returns 200 does not mean the demo works — the API behind it
 * can be down, the seed data wrong, the laptop on the wrong network — so a
 * liveness check would answer a question nobody asked while the speaker, who
 * can see the screen, already knows. One key is faster than a probe anyway.
 */

/** Attribute naming the side on screen. Mirrors `slidx_core::demo`. */
export const DEMO_ATTRIBUTE = "data-slidx-demo";

export type DemoSide = "live" | "fallback";

export interface DemoSwitch {
  /** The side currently painted. */
  side(): DemoSide;
  /**
   * Whether the recording holds enough data to start playing now.
   *
   * Read from the browser rather than remembered, because a recording can be
   * evicted between the check and the talk.
   */
  ready(): boolean;
  /** Shows a side. Does nothing when it is already the side on screen. */
  show(side: DemoSide): void;
  /** Swaps sides. This is what the key is bound to. */
  toggle(): void;
}

/**
 * `HAVE_CURRENT_DATA` — the recording can paint a frame.
 *
 * Deliberately not `HAVE_ENOUGH_DATA`: the question a speaker is asking is
 * "will something appear when I press the key", and a recording that has
 * buffered its opening is a recording that answers yes. Waiting for the
 * browser to predict an uninterrupted play-through would report a working
 * fallback as broken on any connection slow enough to need one.
 */
const HAVE_CURRENT_DATA = 2;

/**
 * Finds the declared demo on this page and prepares its recording.
 *
 * Returns `null` when there is no demo, and also when there is a demo with no
 * recording — in both cases there is nothing to switch to, and the caller is
 * expected to leave the key unbound rather than bind it to a no-op. A key that
 * visibly does nothing is a key a speaker presses again, harder, on stage.
 */
export function createDemoSwitch(root: ParentNode): DemoSwitch | null {
  const figure = root.querySelector(`[${DEMO_ATTRIBUTE}]`);
  const video = figure?.querySelector("video");

  if (!figure || !video) return null;

  // `preload="auto"` is advisory. This is the one asset in a deck where the
  // browser's judgement about whether the fetch is worth it is wrong.
  if (video.readyState === 0) video.load();

  const read = (): DemoSide =>
    figure.getAttribute(DEMO_ATTRIBUTE) === "fallback" ? "fallback" : "live";

  function show(side: DemoSide): void {
    // Re-showing the current side would rewind the recording to a frame the
    // speaker has already talked past.
    if (side === read()) return;

    figure.setAttribute(DEMO_ATTRIBUTE, side);

    if (side === "fallback") {
      // Autoplay policy rejects for reasons that have nothing to do with this
      // deck, and an unhandled rejection here would surface as an error thrown
      // into the slide at the worst moment available. The recording is on
      // screen either way, with controls, so a refused autoplay costs one
      // click rather than the demo.
      void Promise.resolve(video.play()).catch(() => {});
      return;
    }

    // Left running, a hidden recording keeps decoding behind the live demo.
    video.pause();
  }

  return {
    show,
    side: read,
    ready: () => video.readyState >= HAVE_CURRENT_DATA,
    toggle: () => show(read() === "live" ? "fallback" : "live"),
  };
}
