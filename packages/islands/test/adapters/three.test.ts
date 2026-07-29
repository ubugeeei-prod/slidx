/**
 * Three.js as one slide's island.
 *
 * Three has no component model, so this adapter is not about mounting — it is
 * about the two things a 3D scene on a slide leaks, and the tests are almost
 * entirely about teardown.
 *
 * The failure modes guarded here:
 *
 * - An animation loop started on slide 5 that is still running on slide 40
 *   burns a core and a battery for a scene nobody is looking at.
 * - A WebGL context that is not forced closed survives until the canvas is
 *   collected. Browsers cap live contexts in single digits and drop the
 *   *oldest*, so the slide that goes blank is one already presented from.
 * - A scene factory that throws after the renderer exists would leak the
 *   context the adapter is here to protect.
 * - A frame callback that throws runs sixty times a second; the sixtieth
 *   exception is no more informative than the first and the deck is unusable
 *   by then.
 *
 * Both three and the frame clock are injected: three is an optional peer
 * dependency that is not installed here, and a test driven by real animation
 * frames is a test with a sleep in it.
 */

import { describe, expect, it } from "vitest";

import {
  threeIsland,
  type FrameScheduler,
  type ThreeIslandContext,
  type ThreeRenderer,
  type ThreeRuntime,
} from "../../src/adapters/three";

interface FakeRenderer extends ThreeRenderer {
  sizes: [number, number][];
  ratios: number[];
  disposes: number;
  contextLosses: number;
}

function fakeThree(): { runtime: ThreeRuntime; renderers: FakeRenderer[] } {
  const renderers: FakeRenderer[] = [];

  class WebGLRenderer implements FakeRenderer {
    readonly domElement = document.createElement("canvas");
    readonly sizes: [number, number][] = [];
    readonly ratios: number[] = [];
    disposes = 0;
    contextLosses = 0;

    constructor() {
      renderers.push(this);
    }

    setSize(width: number, height: number): void {
      this.sizes.push([width, height]);
    }

    setPixelRatio(ratio: number): void {
      this.ratios.push(ratio);
    }

    dispose(): void {
      this.disposes += 1;
    }

    forceContextLoss(): void {
      this.contextLosses += 1;
    }
  }

  return { renderers, runtime: { WebGLRenderer } };
}

/** A frame clock the test steps by hand. */
function manualFrames() {
  let next = 1;
  const queued = new Map<number, (timestamp: number) => void>();
  const cancelled: number[] = [];

  return {
    scheduler: {
      request(callback) {
        const handle = next++;
        queued.set(handle, callback);
        return handle;
      },
      cancel(handle) {
        cancelled.push(handle);
        queued.delete(handle);
      },
    } satisfies FrameScheduler,
    /** Runs whatever is queued, at `timestamp`. */
    tick(timestamp: number) {
      const pending = [...queued.entries()];
      queued.clear();
      for (const [, callback] of pending) callback(timestamp);
    },
    pending: () => queued.size,
    cancelled: () => cancelled.length,
  };
}

function target(): HTMLElement {
  return document.createElement("div");
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(fakeThree().runtime),
    });

    expect(island.name).toBe("three");
  });

  it("can be given a different token", () => {
    const island = threeIsland({
      name: "globe",
      scene: () => ({}),
      runtime: () => Promise.resolve(fakeThree().runtime),
    });

    expect(island.name).toBe("globe");
  });
});

describe("mounting", () => {
  it("puts the renderer's canvas on the slide", async () => {
    const three = fakeThree();
    const element = target();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await island.mount(element, {});

    expect(element.firstElementChild).toBe(three.renderers[0]?.domElement);
  });

  it("replaces the placeholder rather than drawing over it", async () => {
    // The placeholder is the still image the slide shows without JavaScript,
    // and the hydrator puts it back on unmount, so nothing is lost by clearing.
    const three = fakeThree();
    const element = target();
    element.innerHTML = "<img alt='a globe'>";
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await island.mount(element, {});

    expect(element.querySelector("img")).toBeNull();
    expect(element.children).toHaveLength(1);
  });

  it("hands the scene the module, the renderer, the element and the props", async () => {
    // The module is passed through so a scene builds its geometry without
    // importing three a second time.
    const three = fakeThree();
    const element = target();
    let seen: ThreeIslandContext | undefined;
    const island = threeIsland({
      scene: (context) => {
        seen = context;
        return {};
      },
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await island.mount(element, { radius: 2 });

    expect(seen?.three).toBe(three.runtime);
    expect(seen?.renderer).toBe(three.renderers[0]);
    expect(seen?.target).toBe(element);
    expect(seen?.props).toEqual({ radius: 2 });
  });

  it("falls back to the deck's reference size before layout", async () => {
    // happy-dom reports zero, as does an overview tile that has not been laid
    // out. A zero-sized drawing buffer reads as a context loss on some drivers.
    const three = fakeThree();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await island.mount(target(), {});

    expect(three.renderers[0]?.sizes).toEqual([[1920, 1080]]);
  });

  it("caps the pixel ratio", async () => {
    // Above 2× costs fill rate for a difference no projector resolves.
    const three = fakeThree();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await island.mount(target(), {});

    expect(three.renderers[0]?.ratios[0]).toBeLessThanOrEqual(2);
  });

  it("accepts a scene built asynchronously", async () => {
    const three = fakeThree();
    const island = threeIsland({
      scene: () => Promise.resolve({ frame: () => {} }),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await expect(island.mount(target(), {})).resolves.toBeDefined();
  });
});

describe("the animation loop", () => {
  it("runs the scene's frame callback", async () => {
    const three = fakeThree();
    const frames = manualFrames();
    const seen: number[] = [];
    const island = threeIsland({
      scene: () => ({ frame: (elapsed) => seen.push(elapsed) }),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
    });

    await island.mount(target(), {});
    frames.tick(1000);
    frames.tick(1016);

    expect(seen).toEqual([0, 16]);
  });

  it("measures elapsed time from the first frame, not from mount", async () => {
    // The gap between them is however long the framework import took, and a
    // scene that starts several seconds into its own animation looks broken.
    const three = fakeThree();
    const frames = manualFrames();
    const seen: number[] = [];
    const island = threeIsland({
      scene: () => ({ frame: (elapsed) => seen.push(elapsed) }),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
    });

    await island.mount(target(), {});
    frames.tick(9_000_000);

    expect(seen).toEqual([0]);
  });

  it("holds no frame callback open for a scene with nothing to animate", async () => {
    const three = fakeThree();
    const frames = manualFrames();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
    });

    await island.mount(target(), {});

    expect(frames.pending()).toBe(0);
  });

  it("stops the loop and reports once when a frame throws", async () => {
    // Sixty identical exceptions a second is not a better diagnosis than one,
    // and it makes the rest of the deck unusable.
    const three = fakeThree();
    const frames = manualFrames();
    const errors: unknown[] = [];
    const island = threeIsland({
      scene: () => ({
        frame: () => {
          throw new Error("bad uniform");
        },
      }),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
      onError: (error) => errors.push(error),
    });

    await island.mount(target(), {});
    frames.tick(0);
    frames.tick(16);

    expect(errors).toHaveLength(1);
    expect(frames.pending()).toBe(0);
  });
});

describe("unmounting", () => {
  it("cancels the pending frame", async () => {
    // A loop that survives its slide burns a core for forty slides.
    const three = fakeThree();
    const frames = manualFrames();
    const seen: number[] = [];
    const island = threeIsland({
      scene: () => ({ frame: (elapsed) => seen.push(elapsed) }),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
    });

    const handle = await island.mount(target(), {});
    handle.unmount();
    frames.tick(1000);

    expect(seen).toEqual([]);
    expect(frames.cancelled()).toBe(1);
  });

  it("disposes the scene", async () => {
    const three = fakeThree();
    let disposed = 0;
    const island = threeIsland({
      scene: () => ({
        dispose: () => {
          disposed += 1;
        },
      }),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    const handle = await island.mount(target(), {});
    handle.unmount();

    expect(disposed).toBe(1);
  });

  it("disposes the renderer and forces the context closed", async () => {
    // `dispose` alone leaves the context alive until the canvas is collected,
    // which is too late when the browser's budget is counted in single digits.
    const three = fakeThree();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    const handle = await island.mount(target(), {});
    handle.unmount();

    expect(three.renderers[0]?.disposes).toBe(1);
    expect(three.renderers[0]?.contextLosses).toBe(1);
  });

  it("takes the canvas back off the slide", async () => {
    const three = fakeThree();
    const element = target();
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    const handle = await island.mount(element, {});
    handle.unmount();

    expect(element.children).toHaveLength(0);
  });

  it("stops drawing before it disposes anything", async () => {
    // Reversed, a queued frame can land on a disposed renderer and throw out
    // of a callback nothing is watching.
    const three = fakeThree();
    const frames = manualFrames();
    const order: string[] = [];
    const island = threeIsland({
      scene: () => ({
        frame: () => order.push("frame"),
        dispose: () => order.push("dispose"),
      }),
      runtime: () => Promise.resolve(three.runtime),
      frames: frames.scheduler,
    });

    const handle = await island.mount(target(), {});
    handle.unmount();
    frames.tick(0);

    expect(order).toEqual(["dispose"]);
  });
});

describe("failing", () => {
  it("rejects when three cannot be loaded", async () => {
    const island = threeIsland({
      scene: () => ({}),
      runtime: () => Promise.reject(new Error("three is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("three is not installed");
  });

  it("releases the context when the scene factory throws", async () => {
    // The renderer already exists at that point. Failing without releasing it
    // would leak exactly the resource this adapter is here to protect.
    const three = fakeThree();
    const island = threeIsland({
      scene: () => {
        throw new Error("geometry is wrong");
      },
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await expect(island.mount(target(), {})).rejects.toThrow("geometry is wrong");
    expect(three.renderers[0]?.disposes).toBe(1);
    expect(three.renderers[0]?.contextLosses).toBe(1);
  });

  it("takes the canvas back off the slide when the scene factory throws", async () => {
    const three = fakeThree();
    const element = target();
    const island = threeIsland({
      scene: () => Promise.reject(new Error("geometry is wrong")),
      runtime: () => Promise.resolve(three.runtime),
      frames: manualFrames().scheduler,
    });

    await expect(island.mount(element, {})).rejects.toThrow("geometry is wrong");
    expect(element.children).toHaveLength(0);
  });
});
