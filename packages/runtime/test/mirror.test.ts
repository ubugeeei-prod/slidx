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

import { describe, expect, it, vi } from "vitest";

import { createMirror, type MirrorMessage, type MirrorTransport } from "../src/mirror";

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
