/**
 * Keeping two windows on the same slide.
 *
 * A presenter has the notes on a laptop and the deck on a projector. Those are
 * two windows, and every failure mode of this feature is the same failure:
 * they disagree, and the speaker finds out from the audience.
 *
 * The behaviours worth stating are about disagreement:
 *
 * - A window must not act on its own message, or advancing loops.
 * - A message that arrives late must not drag the deck backwards.
 * - A window opened mid-talk must catch up rather than start at slide one.
 * - The transport is not guaranteed. Where it is missing, mirroring is absent
 *   and the deck still presents — one working window beats two broken ones.
 */

import { describe, expect, it, vi } from "vite-plus/test";

import {
  createMirror,
  type DemoReport,
  type MirrorMessage,
  type MirrorTransport,
} from "../src/mirror";

/** An in-memory transport standing in for BroadcastChannel. */
function bus() {
  const listeners = new Set<(message: MirrorMessage) => void>();

  return {
    channel(): MirrorTransport {
      let own: ((message: MirrorMessage) => void) | undefined;
      return {
        post(message) {
          for (const listener of listeners) {
            if (listener !== own) listener(message);
          }
        },
        listen(handler) {
          own = handler;
          listeners.add(handler);
          return () => listeners.delete(handler);
        },
        close() {
          if (own) listeners.delete(own);
        },
      };
    },
    size: () => listeners.size,
  };
}

/**
 * A transport that echoes and delivers late, which the interface permits and
 * neither shipped transport does. Holds what was posted until `deliver`, so an
 * echo can land after the window has moved on.
 */
function loopback() {
  const listeners = new Set<(message: MirrorMessage) => void>();
  const posted: MirrorMessage[] = [];

  return {
    posted,
    deliver() {
      const queued = posted.splice(0);
      for (const message of queued) {
        for (const listener of listeners) listener(message);
      }
    },
    channel(): MirrorTransport {
      return {
        post: (message) => void posted.push(message),
        listen(handler) {
          listeners.add(handler);
          return () => listeners.delete(handler);
        },
        close() {},
      };
    },
  };
}

function pair() {
  const shared = bus();
  const presenter = createMirror({ transport: shared.channel() });
  const audience = createMirror({ transport: shared.channel() });
  return { presenter, audience, shared };
}

describe("staying in step", () => {
  it("carries a position from one window to the other", () => {
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.send({ slide: 2, step: 1 });

    expect(seen).toHaveBeenCalledWith({ slide: 2, step: 1 });
  });

  it("does not deliver a window its own message", () => {
    // Applying your own broadcast re-broadcasts it, and one keypress becomes
    // a loop between the two windows.
    const { presenter } = pair();
    const seen = vi.fn();
    presenter.subscribe(seen);

    presenter.send({ slide: 2, step: 0 });

    expect(seen).not.toHaveBeenCalled();
  });

  it("works in both directions", () => {
    // Either window can be the one being clicked.
    const { presenter, audience } = pair();
    const seen = vi.fn();
    presenter.subscribe(seen);

    audience.send({ slide: 5, step: 2 });

    expect(seen).toHaveBeenCalledWith({ slide: 5, step: 2 });
  });

  it("remembers the last position it sent", () => {
    const { presenter } = pair();
    presenter.send({ slide: 3, step: 1 });

    expect(presenter.position()).toEqual({ slide: 3, step: 1 });
  });

  it("remembers the last position it received", () => {
    const { presenter, audience } = pair();
    presenter.send({ slide: 3, step: 1 });

    expect(audience.position()).toEqual({ slide: 3, step: 1 });
  });
});

describe("messages arriving out of order", () => {
  it("ignores a position older than the one already shown", () => {
    // Two windows on a venue network do not deliver in order. A stale message
    // applied late drags the deck backwards mid-sentence.
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.send({ slide: 4, step: 0 });
    presenter.sendAt({ slide: 2, step: 0 }, 1);

    expect(seen).toHaveBeenCalledTimes(1);
    expect(audience.position()).toEqual({ slide: 4, step: 0 });
  });

  it("accepts a newer position", () => {
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.send({ slide: 2, step: 0 });
    presenter.send({ slide: 3, step: 0 });

    expect(seen).toHaveBeenLastCalledWith({ slide: 3, step: 0 });
  });

  it("delivers a repeated position only once", () => {
    // Re-asserting the current stop is normal — a resize, a URL sync. It must
    // not cost a DOM write in the other window.
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.send({ slide: 2, step: 1 });
    presenter.send({ slide: 2, step: 1 });

    expect(seen).toHaveBeenCalledTimes(1);
  });

  it("counts each sender separately, so one window cannot silence another", () => {
    // The stage failure this ordering rule caused, and the reason `from` is on
    // the wire.
    //
    // A deck is multi-page HTML, so every move reloads a window and restarts
    // its counter at one. The presenter view announces its position on load, so
    // a projector opening later can sync — with a single watermark that one
    // announcement raised the bar to 1, and a freshly loaded projector page can
    // only ever count to 1 as well. Every position the projector sent from then
    // on was dropped.
    //
    // The speaker drives from the projector, because that is where a clicker's
    // keys land. Their notes stopped following and nothing said so.
    const shared = bus();
    const presenter = createMirror({ transport: shared.channel(), id: "presenter" });
    const projector = createMirror({ transport: shared.channel(), id: "projector" });

    const followed = vi.fn();
    presenter.subscribe(followed);

    presenter.send({ slide: 0, step: 0 });
    projector.sendAt({ slide: 1, step: 0 }, 1);

    expect(followed).toHaveBeenCalledWith({ slide: 1, step: 0 });
  });

  it("still orders a single sender's own messages", () => {
    // Per-sender is not no ordering. A message that arrives late from the
    // window that sent it must still not drag the deck backwards.
    const shared = bus();
    const presenter = createMirror({ transport: shared.channel(), id: "presenter" });
    const audience = createMirror({ transport: shared.channel(), id: "audience" });

    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.sendAt({ slide: 5, step: 0 }, 5);
    presenter.sendAt({ slide: 2, step: 0 }, 1);

    expect(seen).toHaveBeenCalledTimes(1);
    expect(seen).toHaveBeenCalledWith({ slide: 5, step: 0 });
  });

  it("ignores its own message, even from a transport that echoes", () => {
    // Nothing in `MirrorTransport` forbids an echo. One would hand a window
    // back its own delayed position after a newer one and walk the deck
    // backwards, and hand it its own request to answer.
    const bounced = loopback();
    const solo = createMirror({ transport: bounced.channel(), id: "solo" });

    const seen = vi.fn();
    solo.subscribe(seen);

    solo.sendAt({ slide: 2, step: 0 }, 1);
    solo.sendAt({ slide: 5, step: 0 }, 5);
    bounced.deliver();

    expect(seen).not.toHaveBeenCalled();
    expect(solo.position()).toEqual({ slide: 5, step: 0 });

    // An echoed request would have it answer itself.
    solo.requestPosition();
    bounced.deliver();

    expect(bounced.posted).toEqual([]);
  });

  it("hears a window that names no sender, as it always did", () => {
    // A tab left open from an older build. Those messages share one watermark
    // between them, which is exactly the behaviour they shipped with.
    const shared = bus();
    const listener = createMirror({ transport: shared.channel(), id: "listener" });
    const raw = shared.channel();

    const seen = vi.fn();
    listener.subscribe(seen);
    raw.post({ type: "position", position: { slide: 4, step: 0 }, sequence: 1 });

    expect(seen).toHaveBeenCalledWith({ slide: 4, step: 0 });
  });
});

describe("a window that joins late", () => {
  /** A third window opening onto a talk that is already under way. */
  function latecomerTo(slide: number, step: number) {
    const shared = bus();
    const presenter = createMirror({ transport: shared.channel() });
    presenter.send({ slide, step });

    const latecomer = createMirror({ transport: shared.channel() });
    const seen = vi.fn();
    latecomer.subscribe(seen);

    return { presenter, latecomer, seen };
  }

  it("catches up rather than starting at slide one", () => {
    // Opening the presenter view mid-talk is normal — a laptop is unplugged,
    // a window is closed by accident. It must land where the talk is.
    const { latecomer, seen } = latecomerTo(6, 2);

    latecomer.requestPosition();

    expect(seen).toHaveBeenCalledWith({ slide: 6, step: 2 });
    expect(latecomer.position()).toEqual({ slide: 6, step: 2 });
  });

  it("knows nothing until it asks", () => {
    const { latecomer } = latecomerTo(6, 2);
    expect(latecomer.position()).toBeNull();
  });

  it("is not told again by a window that already agrees", () => {
    // Re-asserting a position everyone holds must not cost a DOM write.
    const { presenter, audience } = pair();
    presenter.send({ slide: 6, step: 2 });

    const seen = vi.fn();
    audience.subscribe(seen);
    audience.requestPosition();

    expect(seen).not.toHaveBeenCalled();
  });

  it("gets no answer when nobody has moved yet", () => {
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);

    presenter.requestPosition();

    expect(seen).not.toHaveBeenCalled();
  });
});

describe("when there is no transport", () => {
  it("still presents", () => {
    // One working window beats two broken ones. Mirroring is an enhancement.
    const mirror = createMirror({ transport: null });

    expect(() => mirror.send({ slide: 1, step: 0 })).not.toThrow();
    expect(mirror.available).toBe(false);
  });

  it("reports itself as unavailable so the UI can say so", () => {
    expect(createMirror({ transport: null }).available).toBe(false);
    expect(createMirror({ transport: bus().channel() }).available).toBe(true);
  });
});

describe("closing", () => {
  it("stops delivering", () => {
    const { presenter, audience } = pair();
    const seen = vi.fn();
    audience.subscribe(seen);
    audience.close();

    presenter.send({ slide: 2, step: 0 });

    expect(seen).not.toHaveBeenCalled();
  });

  it("releases the transport", () => {
    const shared = bus();
    const mirror = createMirror({ transport: shared.channel() });
    expect(shared.size()).toBe(1);

    mirror.close();
    expect(shared.size()).toBe(0);
  });

  it("lets one subscriber leave without affecting another", () => {
    const { presenter, audience } = pair();
    const staying = vi.fn();
    const leaving = vi.fn();

    audience.subscribe(staying);
    const unsubscribe = audience.subscribe(leaving);
    unsubscribe();

    presenter.send({ slide: 2, step: 0 });

    expect(staying).toHaveBeenCalled();
    expect(leaving).not.toHaveBeenCalled();
  });
});

describe("what the projector says about its recording", () => {
  it("reaches the other window", () => {
    // The presenter view has no `<video>` to read: its next-slide preview is
    // deliberately inert, and a page that fetched the same file would be
    // proving something true about the wrong machine.
    const wire = bus();
    const projector = createMirror({ transport: wire.channel(), id: "projector" });
    const presenter = createMirror({ transport: wire.channel(), id: "presenter" });

    const seen: DemoReport[] = [];
    presenter.subscribeDemo((report) => seen.push(report));

    projector.reportDemo({ ready: false, side: "live" });
    projector.reportDemo({ ready: true, side: "live" });

    expect(seen).toEqual([
      { ready: false, side: "live" },
      { ready: true, side: "live" },
    ]);
  });

  it("is not ordered, because the latest is always the truest", () => {
    // A position needs ordering because two windows both move and a stale one
    // must not win. A readiness report is a fact about one element in one
    // document, and giving it a sequence would let a reload silence it — which
    // is the bug #255 was.
    const wire = bus();
    const projector = createMirror({ transport: wire.channel(), id: "projector" });
    const presenter = createMirror({ transport: wire.channel(), id: "presenter" });

    const seen: DemoReport[] = [];
    presenter.subscribeDemo((report) => seen.push(report));

    projector.reportDemo({ ready: true, side: "live" });
    projector.reportDemo({ ready: false, side: "live" });

    expect(seen.at(-1)).toEqual({ ready: false, side: "live" });
  });

  it("does not disturb the position the deck is on", () => {
    // Two kinds on one channel. A readiness report that moved a slide would be
    // the worst possible way to learn this feature exists.
    const wire = bus();
    const projector = createMirror({ transport: wire.channel(), id: "projector" });
    const presenter = createMirror({ transport: wire.channel(), id: "presenter" });

    projector.send({ slide: 3, step: 0 });
    projector.reportDemo({ ready: true, side: "fallback" });

    expect(presenter.position()).toEqual({ slide: 3, step: 0 });
  });

  it("tells a window nothing about its own recording", () => {
    const wire = bus();
    const projector = createMirror({ transport: wire.channel(), id: "projector" });

    const seen: DemoReport[] = [];
    projector.subscribeDemo((report) => seen.push(report));

    projector.reportDemo({ ready: true, side: "live" });

    expect(seen).toEqual([]);
  });
});
