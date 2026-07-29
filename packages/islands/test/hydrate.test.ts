/**
 * Bringing a slide's islands to life, and taking them down again.
 *
 * This is the specification for the one property that matters more than every
 * feature in the package: **a speaker on stage cannot debug a framework**.
 * Every behaviour stated here is a way that could go wrong, and what the deck
 * does instead.
 *
 * The failure modes guarded here:
 *
 * - An island that throws while mounting would otherwise take its slide with
 *   it. The placeholder — the content the deck renders with no JavaScript at
 *   all — has to survive, and the rest of the slide has to present.
 * - A Three.js scene on slide 40 mounted at page load costs slide 1 a GL
 *   context and an animation loop it will never use.
 * - An island left mounted after its slide is gone accumulates: forty live
 *   WebGL contexts do not fit on a laptop, and the browser drops the *oldest*,
 *   so the slide that breaks is one already presented from.
 * - A step change, a re-render, or a scroll that re-fires visibility would
 *   stack a second component on top of the first.
 * - Props are text some other program wrote. Malformed JSON must cost the
 *   island its data, not the slide.
 *
 * Visibility and time are both injected. Lazy mounting is the behaviour least
 * testable against a real layout engine — happy-dom has no layout, so nothing
 * ever intersects — and a mount driven by a real import would be a test with a
 * sleep in it.
 */

import { describe, expect, it, vi } from "vitest";

import {
  ISLAND_ATTRIBUTE,
  PROPS_ATTRIBUTE,
  STATE_ATTRIBUTE,
  type IslandDefinition,
  type IslandHandle,
  type IslandProps,
} from "../src/contract";
import { hydrateIslands, type IslandController } from "../src/hydrate";
import { createRegistry } from "../src/registry";
import type { IslandProblem } from "../src/report";
import type { IslandVisibility } from "../src/visibility";

/** Visibility the test drives, standing in for a slide scrolling into view. */
function manualVisibility() {
  const watchers = new Map<HTMLElement, (visible: boolean) => void>();

  return {
    visibility: {
      observe(element, onChange) {
        watchers.set(element, onChange);
        return () => watchers.delete(element);
      },
    } satisfies IslandVisibility,
    show: (element: HTMLElement) => watchers.get(element)?.(true),
    hide: (element: HTMLElement) => watchers.get(element)?.(false),
    watching: () => watchers.size,
  };
}

/** A promise the test resolves by hand, so a mount can be caught mid-flight. */
function deferred<T>() {
  let settle!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolveIt, rejectIt) => {
    settle = resolveIt;
    reject = rejectIt;
  });
  return { promise, settle, reject };
}

interface Recorded {
  definition: IslandDefinition;
  mounts: { target: HTMLElement; props: IslandProps }[];
  unmounts: number;
}

/** An island that records what it was asked to do. */
function recording(
  name: string,
  behaviour: {
    mount?: (target: HTMLElement, props: IslandProps) => Promise<IslandHandle>;
    unmount?: () => void;
  } = {},
): Recorded {
  const recorded: Recorded = {
    mounts: [],
    unmounts: 0,
    definition: {
      name,
      async mount(target, props) {
        recorded.mounts.push({ target, props });
        if (behaviour.mount) return behaviour.mount(target, props);
        return {
          unmount: () => {
            recorded.unmounts += 1;
            behaviour.unmount?.();
          },
        };
      },
    },
  };
  return recorded;
}

interface Harness {
  root: HTMLElement;
  controller: IslandController;
  problems: IslandProblem[];
  show(element: HTMLElement): void;
  hide(element: HTMLElement): void;
  watching(): number;
}

/** A slide with some markup on it and a set of islands the deck opted into. */
function slide(html: string, definitions: IslandDefinition[]): Harness {
  const root = document.createElement("div");
  root.innerHTML = html;

  const problems: IslandProblem[] = [];
  const observer = manualVisibility();

  const controller = hydrateIslands(root, {
    registry: createRegistry(definitions),
    visibility: observer.visibility,
    report: (problem) => problems.push(problem),
  });

  return {
    root,
    controller,
    problems,
    show: observer.show,
    hide: observer.hide,
    watching: observer.watching,
  };
}

function island(harness: Harness, index = 0): HTMLElement {
  return harness.root.querySelectorAll<HTMLElement>(`[${ISLAND_ATTRIBUTE}]`)[index]!;
}

describe("finding islands", () => {
  it("manages every marked element, in document order", () => {
    const vue = recording("vue");
    const react = recording("react");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vue"></div><div ${ISLAND_ATTRIBUTE}="react"></div>`,
      [vue.definition, react.definition],
    );

    expect(harness.controller.islands).toEqual([island(harness, 0), island(harness, 1)]);
  });

  it("finds an island nested inside the slide's markup", () => {
    const vue = recording("vue");
    const harness = slide(
      `<section><figure><div ${ISLAND_ATTRIBUTE}="vue"></div></figure></section>`,
      [vue.definition],
    );

    expect(harness.controller.islands).toHaveLength(1);
  });

  it("does nothing on a slide with no islands", () => {
    const harness = slide("<p>just a slide</p>", []);

    expect(harness.controller.islands).toEqual([]);
    expect(harness.problems).toEqual([]);
  });

  it("has no state for an element it is not managing", () => {
    const harness = slide("<p>just a slide</p>", []);

    expect(harness.controller.stateOf(harness.root)).toBeUndefined();
  });
});

describe("mounting lazily", () => {
  it("mounts nothing while the slide is off screen", () => {
    // The whole point of an island: slide 1 must not pay for slide 40.
    const three = recording("three");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="three"></div>`, [three.definition]);

    expect(three.mounts).toHaveLength(0);
    expect(harness.controller.stateOf(island(harness))).toBe("idle");
  });

  it("mounts when the slide comes on screen", async () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(vue.mounts).toHaveLength(1);
    expect(harness.controller.stateOf(island(harness))).toBe("mounted");
  });

  it("mounts into the island's own element", () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));

    expect(vue.mounts[0]?.target).toBe(island(harness));
  });

  it("mounts only the island that came on screen", () => {
    const vue = recording("vue");
    const three = recording("three");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vue"></div><div ${ISLAND_ATTRIBUTE}="three"></div>`,
      [vue.definition, three.definition],
    );

    harness.show(island(harness, 0));

    expect(vue.mounts).toHaveLength(1);
    expect(three.mounts).toHaveLength(0);
  });

  it("reflects the lifecycle in an attribute a theme can style", async () => {
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    expect(island(harness).hasAttribute(STATE_ATTRIBUTE)).toBe(false);

    harness.show(island(harness));
    expect(island(harness).getAttribute(STATE_ATTRIBUTE)).toBe("mounting");

    pending.settle({ unmount: () => {} });
    await harness.controller.settled();
    expect(island(harness).getAttribute(STATE_ATTRIBUTE)).toBe("mounted");
  });
});

describe("mounting idempotently", () => {
  it("ignores a second visibility report for an island already mounted", async () => {
    // A step change or a re-render re-asserts visibility. Stacking a second
    // component on the first would double every event handler on the slide.
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.show(island(harness));
    await harness.controller.settled();

    expect(vue.mounts).toHaveLength(1);
  });

  it("ignores a visibility report while a mount is still in flight", async () => {
    // The framework import is the slow part, and it is exactly the window in
    // which a scroll fires visibility again.
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    harness.show(island(harness));
    harness.show(island(harness));

    pending.settle({ unmount: () => {} });
    await harness.controller.settled();

    expect(vue.mounts).toHaveLength(1);
  });

  it("does not unmount an island that was never mounted", () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.hide(island(harness));

    expect(vue.unmounts).toBe(0);
    expect(harness.controller.stateOf(island(harness))).toBe("idle");
  });
});

describe("unmounting on leaving", () => {
  it("unmounts when the slide goes off screen", async () => {
    // A GL context or an interval that survives forty slides exhausts the
    // machine, and the deck gets the blame.
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));

    expect(vue.unmounts).toBe(1);
    expect(harness.controller.stateOf(island(harness))).toBe("idle");
  });

  it("clears the lifecycle attribute once the island is down", async () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));

    expect(island(harness).hasAttribute(STATE_ATTRIBUTE)).toBe(false);
  });

  it("restores the placeholder the slide had without JavaScript", async () => {
    const vue = recording("vue", {
      mount: (target) => {
        target.innerHTML = "<canvas></canvas>";
        return Promise.resolve({ unmount: () => {} });
      },
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"><img alt="a chart"></div>`, [
      vue.definition,
    ]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));

    expect(island(harness).innerHTML).toBe('<img alt="a chart">');
  });

  it("mounts again when the slide comes back", async () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));
    harness.show(island(harness));
    await harness.controller.settled();

    expect(vue.mounts).toHaveLength(2);
    expect(vue.unmounts).toBe(1);
  });

  it("unmounts an island whose slide left while it was still loading", async () => {
    // The framework import outlives the slide more often than it looks: a cold
    // cache and a fast talker is all it takes. A scene mounted after its slide
    // has gone would hold its context until the deck closed.
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    harness.hide(island(harness));

    let unmounted = 0;
    pending.settle({
      unmount: () => {
        unmounted += 1;
      },
    });
    await harness.controller.settled();

    expect(unmounted).toBe(1);
    expect(harness.controller.stateOf(island(harness))).toBe("idle");
  });

  it("reports an unmount that throws and stops driving the island", async () => {
    // Whatever the integration held, it still holds it. Mounting a second one
    // on top of a teardown that did not finish makes the leak worse.
    const vue = recording("vue", {
      unmount: () => {
        throw new Error("teardown exploded");
      },
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));

    expect(harness.problems.map((problem) => problem.kind)).toEqual(["unmount-failed"]);
    expect(harness.controller.stateOf(island(harness))).toBe("failed");

    harness.show(island(harness));
    await harness.controller.settled();
    expect(vue.mounts).toHaveLength(1);
  });
});

describe("an island that fails to mount", () => {
  it("does not throw out of hydration", () => {
    const vue = recording("vue", { mount: () => Promise.reject(new Error("no")) });

    expect(() => {
      const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);
      harness.show(island(harness));
    }).not.toThrow();
  });

  it("leaves its placeholder content in place", async () => {
    // The single most important behaviour in the package. A broken island is
    // a slide that still reads, not a hole where a chart should be.
    const vue = recording("vue", {
      mount: (target) => {
        target.innerHTML = "<half-built>";
        return Promise.reject(new Error("no"));
      },
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"><p>Revenue grew 12%</p></div>`, [
      vue.definition,
    ]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(island(harness).innerHTML).toBe("<p>Revenue grew 12%</p>");
  });

  it("leaves the rest of the slide presenting", async () => {
    const broken = recording("vue", { mount: () => Promise.reject(new Error("no")) });
    const working = recording("react");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vue"></div><div ${ISLAND_ATTRIBUTE}="react"></div>`,
      [broken.definition, working.definition],
    );

    harness.show(island(harness, 0));
    harness.show(island(harness, 1));
    await harness.controller.settled();

    expect(harness.controller.stateOf(island(harness, 0))).toBe("failed");
    expect(harness.controller.stateOf(island(harness, 1))).toBe("mounted");
  });

  it("survives a mount that throws synchronously", async () => {
    // An integration can fail before it ever returns a promise — a missing
    // export reads as `undefined is not a function`.
    const vue: IslandDefinition = {
      name: "vue",
      mount: () => {
        throw new Error("not a function");
      },
    };
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(harness.controller.stateOf(island(harness))).toBe("failed");
  });

  it("reports the failure with the thrown value attached", async () => {
    const cause = new Error("vite could not resolve vue");
    const vue = recording("vue", { mount: () => Promise.reject(cause) });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(harness.problems[0]?.kind).toBe("mount-failed");
    expect(harness.problems[0]?.cause).toBe(cause);
    expect(harness.problems[0]?.name).toBe("vue");
    expect(harness.problems[0]?.element).toBe(island(harness));
  });

  it("marks the element failed so a theme can style it deliberately", async () => {
    const vue = recording("vue", { mount: () => Promise.reject(new Error("no")) });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(island(harness).getAttribute(STATE_ATTRIBUTE)).toBe("failed");
  });

  it("is not retried when the slide comes back", async () => {
    // A mount that failed once fails again. Retrying floods the console a
    // speaker might be reading and re-runs whatever side effect broke it.
    const vue = recording("vue", { mount: () => Promise.reject(new Error("no")) });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.hide(island(harness));
    harness.show(island(harness));
    await harness.controller.settled();

    expect(vue.mounts).toHaveLength(1);
    expect(harness.problems).toHaveLength(1);
  });

  it("reports an island that resolves without a way to unmount it", async () => {
    // The component works; only teardown is lost. Tearing it down would be the
    // worse trade, so this is a leak reported once, not a failure.
    const vue = recording("vue", {
      mount: () => Promise.resolve(undefined as unknown as IslandHandle),
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(harness.problems[0]?.kind).toBe("invalid-handle");
    expect(harness.controller.stateOf(island(harness))).toBe("mounted");
  });

  it("survives leaving a slide whose handle could not unmount", async () => {
    const vue = recording("vue", {
      mount: () => Promise.resolve({} as IslandHandle),
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();

    expect(() => harness.hide(island(harness))).not.toThrow();
  });
});

describe("an island the deck did not register", () => {
  it("reports it while the deck loads, not when the slide is reached", () => {
    // A typo discovered on stage is discovered too late. Resolving the name is
    // cheap, so it happens at hydration even though mounting does not.
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vuu"></div>`, [recording("vue").definition]);

    expect(harness.problems[0]?.kind).toBe("unknown-island");
    expect(harness.problems[0]?.message).toContain("registered: vue");
  });

  it("leaves its placeholder content in place", () => {
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vuu"><p>Revenue grew 12%</p></div>`, [
      recording("vue").definition,
    ]);

    expect(island(harness).innerHTML).toBe("<p>Revenue grew 12%</p>");
  });

  it("is not managed and not watched", () => {
    // Watching an island that can never mount would spend a layout
    // subscription on a visibility change that could not mean anything.
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vuu"></div>`, [recording("vue").definition]);

    expect(harness.controller.islands).toEqual([]);
    expect(harness.watching()).toBe(0);
  });

  it("reports an element marked as an island that names none", () => {
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}=""></div>`, []);

    expect(harness.problems[0]?.kind).toBe("missing-name");
    expect(harness.problems[0]?.name).toBe("");
  });

  it("reports a name that is only whitespace as missing rather than unknown", () => {
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="  "></div>`, []);

    expect(harness.problems[0]?.kind).toBe("missing-name");
  });

  it("does not stop the islands around it from mounting", () => {
    const vue = recording("vue");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vuu"></div><div ${ISLAND_ATTRIBUTE}="vue"></div>`,
      [vue.definition],
    );

    harness.show(island(harness, 1));

    expect(vue.mounts).toHaveLength(1);
  });
});

describe("props", () => {
  it("hands the parsed props to the integration", () => {
    const vue = recording("vue");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vue" ${PROPS_ATTRIBUTE}='{"title":"Q3","count":4}'></div>`,
      [vue.definition],
    );

    harness.show(island(harness));

    expect(vue.mounts[0]?.props).toEqual({ title: "Q3", count: 4 });
  });

  it("hands over empty props when the attribute is absent", () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));

    expect(vue.mounts[0]?.props).toEqual({});
  });

  it("mounts with empty props rather than throwing on malformed JSON", () => {
    // A chart with no data still leaves the slide readable. A chart that
    // refused to mount leaves a hole.
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue" ${PROPS_ATTRIBUTE}="{nope}"></div>`, [
      vue.definition,
    ]);

    harness.show(island(harness));

    expect(vue.mounts[0]?.props).toEqual({});
    expect(harness.problems[0]?.kind).toBe("invalid-props");
  });

  it("names the island in the props problem", () => {
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue" ${PROPS_ATTRIBUTE}="{nope}"></div>`, [
      recording("vue").definition,
    ]);

    expect(harness.problems[0]?.message).toContain("vue:");
  });

  it("parses props while the deck loads, not when the slide is reached", () => {
    // Same reason as an unknown name: a malformed attribute is an authoring
    // mistake, and finding it on stage is finding it too late.
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue" ${PROPS_ATTRIBUTE}="{nope}"></div>`, [
      recording("vue").definition,
    ]);

    expect(harness.problems).toHaveLength(1);
  });

  it("still reports bad props on an island that never becomes visible", () => {
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue" ${PROPS_ATTRIBUTE}="[]"></div>`, [
      recording("vue").definition,
    ]);

    expect(harness.problems[0]?.kind).toBe("invalid-props");
    expect(harness.controller.stateOf(island(harness))).toBe("idle");
  });
});

describe("tearing the deck down", () => {
  it("unmounts everything that is mounted", async () => {
    const vue = recording("vue");
    const three = recording("three");
    const harness = slide(
      `<div ${ISLAND_ATTRIBUTE}="vue"></div><div ${ISLAND_ATTRIBUTE}="three"></div>`,
      [vue.definition, three.definition],
    );

    harness.show(island(harness, 0));
    harness.show(island(harness, 1));
    await harness.controller.settled();
    harness.controller.destroy();

    expect(vue.unmounts).toBe(1);
    expect(three.unmounts).toBe(1);
  });

  it("stops watching for visibility", async () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.controller.destroy();
    harness.show(island(harness));
    await harness.controller.settled();

    expect(harness.watching()).toBe(0);
    expect(vue.mounts).toHaveLength(0);
  });

  it("unmounts an island whose framework was still loading", async () => {
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    harness.controller.destroy();

    let unmounted = 0;
    pending.settle({
      unmount: () => {
        unmounted += 1;
      },
    });
    await harness.controller.settled();

    expect(unmounted).toBe(1);
  });

  it("is safe to call twice", async () => {
    const vue = recording("vue");
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.controller.destroy();
    harness.controller.destroy();

    expect(vue.unmounts).toBe(1);
  });

  it("restores every placeholder", async () => {
    const vue = recording("vue", {
      mount: (target) => {
        target.innerHTML = "<canvas></canvas>";
        return Promise.resolve({ unmount: () => {} });
      },
    });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"><p>fallback</p></div>`, [vue.definition]);

    harness.show(island(harness));
    await harness.controller.settled();
    harness.controller.destroy();

    expect(island(harness).innerHTML).toBe("<p>fallback</p>");
  });
});

describe("reporting", () => {
  it("does not let a throwing reporter take the slide down", async () => {
    // The reporter belongs to whoever embedded the deck, and it runs on the
    // path that exists to survive failure.
    const root = document.createElement("div");
    root.innerHTML = `<div ${ISLAND_ATTRIBUTE}="vuu"></div>`;

    expect(() =>
      hydrateIslands(root, {
        registry: createRegistry(),
        report: () => {
          throw new Error("the overlay is broken too");
        },
      }),
    ).not.toThrow();
  });

  it("warns on the console by default", () => {
    // `warn` rather than `error`: none of these stop the deck, and a red
    // console during a live talk reads as something worse than it is.
    const root = document.createElement("div");
    root.innerHTML = `<div ${ISLAND_ATTRIBUTE}="vuu"></div>`;
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    hydrateIslands(root, { registry: createRegistry() });

    expect(warn).toHaveBeenCalledOnce();
    expect(warn.mock.calls[0]?.[0]).toContain("slidx islands:");
    warn.mockRestore();
  });
});

describe("settling", () => {
  it("resolves immediately when nothing is in flight", async () => {
    const harness = slide("<p>nothing here</p>", []);

    await expect(harness.controller.settled()).resolves.toBeUndefined();
  });

  it("waits for a mount that has not finished", async () => {
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));

    let settled = false;
    const waiting = harness.controller.settled().then(() => {
      settled = true;
    });

    await Promise.resolve();
    expect(settled).toBe(false);

    pending.settle({ unmount: () => {} });
    await waiting;
    expect(settled).toBe(true);
  });

  it("waits for a failing mount too", async () => {
    const pending = deferred<IslandHandle>();
    const vue = recording("vue", { mount: () => pending.promise });
    const harness = slide(`<div ${ISLAND_ATTRIBUTE}="vue"></div>`, [vue.definition]);

    harness.show(island(harness));
    pending.reject(new Error("no"));
    await harness.controller.settled();

    expect(harness.controller.stateOf(island(harness))).toBe("failed");
  });
});
