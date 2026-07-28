/**
 * Applying a compiled frame to the DOM.
 *
 * The Rust compiler emits *complete state snapshots*, one per stop, so this
 * module never computes a delta. `apply(n)` writes the whole slide's state
 * from frame `n`, which is why jumping, rewinding, and deep-linking all take
 * the same code path and cannot disagree.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { ANCHOR_ATTRIBUTE } from "../src/anchor";
import { createStage, HIDDEN_ATTRIBUTE, STAGED_ATTRIBUTE } from "../src/stage";
import type { StepTimeline } from "../src/types";

/** Mirrors what `compile_timeline` emits for two sequential reveals. */
const TWO_REVEALS: StepTimeline = {
  frames: [
    {
      index: 0,
      states: [
        { target: '[data-slidx-step="1"]', visibility: "hidden" },
        { target: '[data-slidx-step="2"]', visibility: "hidden" },
      ],
    },
    {
      index: 1,
      states: [
        {
          target: '[data-slidx-step="1"]',
          visibility: "visible",
          effect: {
            kind: "entrance",
            preset: "fly-in",
            durationMs: 400,
            delayMs: 0,
            easing: "ease-out",
            origin: "left",
          },
        },
        { target: '[data-slidx-step="2"]', visibility: "hidden" },
      ],
    },
    {
      index: 2,
      states: [
        { target: '[data-slidx-step="1"]', visibility: "visible" },
        {
          target: '[data-slidx-step="2"]',
          visibility: "visible",
          effect: {
            kind: "entrance",
            preset: "fade",
            durationMs: 400,
            delayMs: 0,
            easing: "ease-out",
          },
        },
      ],
    },
  ],
};

function mount(): HTMLElement {
  const root = document.createElement("div");
  root.innerHTML =
    `<ul><li>one<span ${ANCHOR_ATTRIBUTE}="1" hidden></span></li>` +
    `<li>two<span ${ANCHOR_ATTRIBUTE}="2" hidden></span></li></ul>`;
  document.body.replaceChildren(root);
  return root;
}

function items(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll("li"));
}

function hidden(root: HTMLElement): boolean[] {
  return items(root).map((item) => item.hasAttribute(HIDDEN_ATTRIBUTE));
}

beforeEach(() => {
  document.body.replaceChildren();
});

describe("setting up", () => {
  it("marks every staged element so the theme can style it", () => {
    const root = mount();
    createStage(root, TWO_REVEALS);

    expect(items(root).every((item) => item.hasAttribute(STAGED_ATTRIBUTE))).toBe(true);
  });

  it("reports how many stops the slide has", () => {
    expect(createStage(mount(), TWO_REVEALS).stopCount).toBe(3);
  });

  it("starts on the resting frame", () => {
    const root = mount();
    createStage(root, TWO_REVEALS);

    expect(hidden(root)).toEqual([true, true]);
  });

  it("leaves a slide with no steps completely alone", () => {
    const root = document.createElement("div");
    root.innerHTML = "<p>Just prose.</p>";
    document.body.replaceChildren(root);

    const stage = createStage(root, { frames: [{ index: 0, states: [] }] });

    expect(stage.stopCount).toBe(1);
    expect(root.querySelector("p")?.hasAttribute(HIDDEN_ATTRIBUTE)).toBe(false);
  });

  it("tolerates a timeline naming a target that is not in the DOM", () => {
    // A stale deep link or a hand-written `steps:` selector that no longer
    // matches must not take the slide down.
    const root = mount();
    const stage = createStage(root, {
      frames: [{ index: 0, states: [{ target: ".gone", visibility: "hidden" }] }],
    });

    expect(() => stage.apply(0)).not.toThrow();
  });
});

describe("advancing", () => {
  it("reveals one element per stop", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(1);
    expect(hidden(root)).toEqual([false, true]);

    stage.apply(2);
    expect(hidden(root)).toEqual([false, false]);
  });

  it("plays the effect carried by the frame it lands on", () => {
    const root = mount();
    createStage(root, TWO_REVEALS).apply(1);

    const first = items(root)[0]!;
    expect(first.getAttribute("data-slidx-effect")).toBe("fly-in");
    expect(first.getAttribute("data-slidx-effect-kind")).toBe("entrance");
  });

  it("passes the effect's timing to CSS rather than scripting it", () => {
    // Keeping animation in CSS is what lets it run on the compositor, which
    // is the difference between smooth and juddering on venue hardware.
    const root = mount();
    createStage(root, TWO_REVEALS).apply(1);

    const style = items(root)[0]!.getAttribute("style") ?? "";
    expect(style).toContain("--slidx-effect-duration: 400ms");
    expect(style).toContain("--slidx-effect-delay: 0ms");
    expect(style).toContain("--slidx-effect-origin: left");
  });

  it("does not replay an effect on a later stop", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(1);
    stage.apply(2);

    expect(items(root)[0]!.hasAttribute("data-slidx-effect")).toBe(false);
    expect(items(root)[1]!.getAttribute("data-slidx-effect")).toBe("fade");
  });
});

describe("going back", () => {
  it("restores the earlier state exactly", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(2);
    stage.apply(1);

    expect(hidden(root)).toEqual([false, true]);
  });

  it("does not replay entrances", () => {
    // Stepping back to check something should not re-run the animation the
    // audience already watched.
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(2);
    stage.apply(1);

    expect(items(root)[0]!.hasAttribute("data-slidx-effect")).toBe(false);
  });

  it("reaches the same state whether it arrived forwards or backwards", () => {
    const forward = mount();
    createStage(forward, TWO_REVEALS).apply(1);
    const forwardState = hidden(forward);

    const backward = mount();
    const stage = createStage(backward, TWO_REVEALS);
    stage.apply(2);
    stage.apply(1);

    expect(hidden(backward)).toEqual(forwardState);
  });
});

describe("jumping", () => {
  it("lands on any stop directly", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(2);
    expect(hidden(root)).toEqual([false, false]);
  });

  it("clamps an index past the end", () => {
    // A deep link written before the slide was edited must land somewhere real.
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    expect(stage.apply(99)).toBe(2);
    expect(hidden(root)).toEqual([false, false]);
  });

  it("clamps a negative index", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    expect(stage.apply(-5)).toBe(0);
  });

  it("reports the stop it settled on", () => {
    const stage = createStage(mount(), TWO_REVEALS);

    expect(stage.apply(1)).toBe(1);
    expect(stage.index).toBe(1);
  });

  it("re-applying the current stop does not disturb the slide", () => {
    // Mirrored windows, resize handlers, and URL syncs all re-assert the
    // current position. Doing that mid-animation must not cut it short.
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(1);
    const mid = root.innerHTML;
    stage.apply(1);

    expect(root.innerHTML).toBe(mid);
  });

  it("clamping onto the stop already shown is also a no-op", () => {
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.apply(2);
    const settled = root.innerHTML;

    expect(stage.apply(99)).toBe(2);
    expect(root.innerHTML).toBe(settled);
  });
});

describe("printing", () => {
  it("shows everything, with no animation", () => {
    // A handout that hides content the audience saw is worse than one that
    // shows a little more.
    const root = mount();
    const stage = createStage(root, TWO_REVEALS);

    stage.applyPrint();

    expect(hidden(root)).toEqual([false, false]);
    expect(items(root).some((item) => item.hasAttribute("data-slidx-effect"))).toBe(false);
  });

  it("shows an element that was revealed and then hidden again", () => {
    const root = mount();
    const stage = createStage(root, {
      frames: [
        { index: 0, states: [{ target: '[data-slidx-step="1"]', visibility: "hidden" }] },
        { index: 1, states: [{ target: '[data-slidx-step="1"]', visibility: "visible" }] },
        { index: 2, states: [{ target: '[data-slidx-step="1"]', visibility: "hidden" }] },
      ],
    });

    stage.applyPrint();
    expect(items(root)[0]!.hasAttribute(HIDDEN_ATTRIBUTE)).toBe(false);
  });
});
