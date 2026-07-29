/**
 * Applying a compiled frame to the DOM.
 *
 * The Rust compiler emits *complete state snapshots*, one per stop, so this
 * module never computes a delta. `apply(n)` writes the whole slide's state
 * from frame `n`, which is why jumping, rewinding, and deep-linking all take
 * the same code path and cannot disagree.
 */

import { beforeEach, describe, expect, it } from "vite-plus/test";

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

/**
 * Changing an element that is already on screen.
 *
 * The snapshot model does the work here: `content` and `properties` are
 * absolute values on each frame, not instructions to mutate. Stepping back is
 * reading an earlier frame, so the runtime keeps no history at all — and the
 * value the audience saw two clicks ago comes back exactly.
 */
const CHANGING_VALUE: StepTimeline = {
  frames: [
    { index: 0, states: [{ target: '[data-slidx-mark="count"]', visibility: "visible" }] },
    {
      index: 1,
      states: [
        {
          target: '[data-slidx-mark="count"]',
          visibility: "visible",
          content: "42",
          properties: { color: "success" },
        },
      ],
    },
  ],
};

function mountMark(): HTMLElement {
  const root = document.createElement("div");
  root.innerHTML = `<p>The answer is <span data-slidx-mark="count">10</span>.</p>`;
  document.body.replaceChildren(root);
  return root;
}

function mark(root: HTMLElement): HTMLElement {
  return root.querySelector<HTMLElement>("[data-slidx-mark]")!;
}

describe("changing an element in place", () => {
  it("starts with the text in the markup", () => {
    const root = mountMark();
    createStage(root, CHANGING_VALUE);

    expect(mark(root).textContent).toBe("10");
  });

  it("writes the text the stop calls for", () => {
    const root = mountMark();
    createStage(root, CHANGING_VALUE).apply(1);

    expect(mark(root).textContent).toBe("42");
  });

  it("changes one element rather than swapping two", () => {
    // Anything holding a reference to the mark — a step, the editor, a
    // measurement — must still be pointing at the same node afterwards.
    const root = mountMark();
    const before = mark(root);
    createStage(root, CHANGING_VALUE).apply(1);

    expect(mark(root)).toBe(before);
    expect(root.querySelectorAll("[data-slidx-mark]")).toHaveLength(1);
  });

  it("restores the markup's own text on the way back", () => {
    const root = mountMark();
    const stage = createStage(root, CHANGING_VALUE);

    stage.apply(1);
    stage.apply(0);

    expect(mark(root).textContent).toBe("10");
  });

  it("applies properties as data attributes for the theme to interpret", () => {
    const root = mountMark();
    createStage(root, CHANGING_VALUE).apply(1);

    expect(mark(root).getAttribute("data-slidx-color")).toBe("success");
  });

  it("removes a property when stepping back past the stop that set it", () => {
    const root = mountMark();
    const stage = createStage(root, CHANGING_VALUE);

    stage.apply(1);
    stage.apply(0);

    expect(mark(root).hasAttribute("data-slidx-color")).toBe(false);
  });

  it("leaves the attributes the pipeline owns alone", () => {
    const root = mountMark();
    const stage = createStage(root, CHANGING_VALUE);

    stage.apply(1);
    stage.apply(0);

    expect(mark(root).getAttribute("data-slidx-mark")).toBe("count");
    expect(mark(root).hasAttribute("data-slidx-staged")).toBe(true);
  });

  it("never interprets patched content as markup", () => {
    // A timeline is data. Letting it inject HTML would turn a deck into a
    // script vector, which matters as soon as decks are shared or generated.
    const root = mountMark();
    createStage(root, {
      frames: [
        { index: 0, states: [{ target: '[data-slidx-mark="count"]', visibility: "visible" }] },
        {
          index: 1,
          states: [
            {
              target: '[data-slidx-mark="count"]',
              visibility: "visible",
              content: "<img src=x onerror=alert(1)>",
            },
          ],
        },
      ],
    }).apply(1);

    expect(mark(root).querySelector("img")).toBeNull();
    expect(mark(root).textContent).toBe("<img src=x onerror=alert(1)>");
  });

  it("shows the final value when printing", () => {
    const root = mountMark();
    createStage(root, CHANGING_VALUE).applyPrint();

    expect(mark(root).textContent).toBe("42");
  });
});
