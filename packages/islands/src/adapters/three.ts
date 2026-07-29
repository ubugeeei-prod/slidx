/**
 * Three.js, as one slide's island.
 *
 * Three has no component model, so unlike the other adapters this one is not
 * about mounting — it is about the two things a 3D scene on a slide gets wrong
 * and cannot recover from:
 *
 * - **The animation loop.** A `requestAnimationFrame` chain started on slide 5
 *   keeps running on slide 40 unless something cancels it, burning a core and
 *   a battery for a scene nobody is looking at.
 * - **The GL context.** Browsers cap live WebGL contexts somewhere around a
 *   dozen and drop the oldest when the cap is hit, so a deck that leaks one
 *   per scene does not fail on the scene that leaked — it blanks an earlier
 *   slide the speaker already presented from.
 *
 * So the adapter owns the renderer and the loop, and the author's factory owns
 * only the scene. Everything the factory allocates it disposes; everything
 * this file allocates, this file disposes.
 *
 * Three is an optional peer dependency and is not installed here, so its API is
 * declared structurally and imported through a variable specifier.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer } from "./peer";

/** The part of `THREE.WebGLRenderer` this adapter uses. */
export interface ThreeRenderer {
  readonly domElement: HTMLCanvasElement;
  setSize(width: number, height: number, updateStyle?: boolean): void;
  setPixelRatio(ratio: number): void;
  dispose(): void;
  /** Present on WebGLRenderer; optional here so a scene can be tested against a stub. */
  forceContextLoss?(): void;
}

export interface ThreeRendererOptions {
  antialias?: boolean;
  alpha?: boolean;
}

/** The part of the `three` module this adapter uses. */
export interface ThreeRuntime {
  WebGLRenderer: new (parameters?: ThreeRendererOptions) => ThreeRenderer;
}

/** What the scene factory is handed. */
export interface ThreeIslandContext {
  /** The module, so a scene builds its geometry without importing three a second time. */
  readonly three: ThreeRuntime;
  /** Already sized and attached. The scene renders through it and must not dispose it. */
  readonly renderer: ThreeRenderer;
  readonly target: HTMLElement;
  readonly props: IslandProps;
}

export interface ThreeScene {
  /** Called once per animation frame while the slide is on screen. */
  frame?(elapsedMs: number): void;
  /** Releases geometries, materials, and textures. The renderer is not the scene's to release. */
  dispose?(): void;
}

/** Injected so the loop is testable without a real frame clock. */
export interface FrameScheduler {
  request(callback: (timestamp: number) => void): number;
  cancel(handle: number): void;
}

export interface ThreeIslandOptions {
  /** Builds the scene. Runs after three has loaded and the renderer exists. */
  scene: (context: ThreeIslandContext) => ThreeScene | Promise<ThreeScene>;
  /** The token a slide selects this with. */
  name?: string;
  /** Substitutes the framework module. Exists so this is testable where three is not installed. */
  runtime?: () => Promise<ThreeRuntime>;
  /** Substitutes the frame clock, for the same reason. */
  frames?: FrameScheduler;
  /** Passed straight to `WebGLRenderer`. */
  renderer?: ThreeRendererOptions;
  /**
   * Where a throwing frame goes.
   *
   * The loop is stopped either way — sixty identical exceptions a second is
   * not a better diagnosis than one, and it makes the rest of the deck
   * unusable.
   */
  onError?: (error: unknown) => void;
}

/**
 * The canvas size when the element has not been laid out yet.
 *
 * An island can be mounted before the slide has width — a hidden overview
 * tile, the print shell, a headless export. A zero-sized drawing buffer is
 * treated as a context loss by some drivers, so the deck's own reference
 * resolution is a safer default than the measurement.
 */
const FALLBACK_WIDTH = 1920;
const FALLBACK_HEIGHT = 1080;

/**
 * Above 2× costs fill rate for a difference no projector resolves. Phones and
 * high-DPI laptops both report more, and a 3× buffer on a 3D slide is the
 * difference between 60fps and visible judder on venue hardware.
 */
const MAX_PIXEL_RATIO = 2;

export function threeIsland(options: ThreeIslandOptions): IslandDefinition {
  const loadRuntime = options.runtime ?? (() => importPeer<ThreeRuntime>("three"));
  const frames = options.frames ?? animationFrames();
  const onError = options.onError ?? defaultOnError;

  return {
    name: options.name ?? "three",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      const three = await loadRuntime();
      const renderer = new three.WebGLRenderer(options.renderer ?? { antialias: true });

      renderer.setPixelRatio(Math.min(pixelRatio(), MAX_PIXEL_RATIO));
      renderer.setSize(
        target.clientWidth || FALLBACK_WIDTH,
        target.clientHeight || FALLBACK_HEIGHT,
      );

      // Replaces rather than appends: the placeholder is the still image the
      // slide shows without JavaScript, and it is put back by the hydrator on
      // unmount, so nothing is lost by clearing it here.
      target.replaceChildren(renderer.domElement);

      let scene: ThreeScene;
      try {
        scene = await options.scene({ three, renderer, target, props });
      } catch (error) {
        // The renderer exists and has a context. Failing without releasing it
        // would leak exactly the resource this adapter is here to protect.
        release(renderer);
        throw error;
      }

      const loop = startLoop(scene, frames, onError);

      return {
        unmount: () => {
          // Ordered: stop drawing, then let the scene drop its geometry, then
          // take the context down. Reversed, a frame can land on a disposed
          // renderer and throw out of a callback nothing is watching.
          loop.stop();
          scene.dispose?.();
          release(renderer);
        },
      };
    },
  };
}

function startLoop(
  scene: ThreeScene,
  frames: FrameScheduler,
  onError: (error: unknown) => void,
): { stop(): void } {
  // A scene with nothing to animate should not hold a frame callback open.
  if (!scene.frame) return { stop: () => {} };

  let running = true;
  let start: number | null = null;
  let pending = 0;

  const tick = (timestamp: number): void => {
    if (!running) return;

    // Elapsed from the first frame rather than from mount: the gap between
    // them is however long the framework import took, and a scene that starts
    // several seconds into its own animation looks broken.
    start ??= timestamp;

    try {
      scene.frame?.(timestamp - start);
    } catch (error) {
      running = false;
      onError(error);
      return;
    }

    pending = frames.request(tick);
  };

  pending = frames.request(tick);

  return {
    stop() {
      running = false;
      frames.cancel(pending);
    },
  };
}

/** Drops the drawing buffer as well as the JavaScript side of the renderer. */
function release(renderer: ThreeRenderer): void {
  renderer.dispose();

  // `dispose` frees three's own resources but leaves the context alive until
  // the canvas is collected, which is too late when the browser's context
  // budget is counted in single digits.
  renderer.forceContextLoss?.();
  renderer.domElement.remove();
}

/** `globalThis` rather than `window`: a deck is also rendered where there is no window. */
function pixelRatio(): number {
  const value = (globalThis as { devicePixelRatio?: number }).devicePixelRatio;
  return typeof value === "number" && value > 0 ? value : 1;
}

function animationFrames(): FrameScheduler {
  return {
    request: (callback) => requestAnimationFrame(callback),
    cancel: (handle) => {
      cancelAnimationFrame(handle);
    },
  };
}

function defaultOnError(error: unknown): void {
  console.error("slidx islands: a three.js frame threw; the animation loop was stopped", error);
}
