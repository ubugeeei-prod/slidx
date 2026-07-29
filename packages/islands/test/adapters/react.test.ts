/**
 * React as one slide's island.
 *
 * React is an optional peer dependency and is not installed here, so both
 * modules the adapter needs are injected. That is also the point of the test:
 * `react` and `react-dom` are separately versioned packages, and a deck that
 * pins them apart should fail on the island rather than at import time.
 *
 * The failure modes guarded here:
 *
 * - A root that is never unmounted keeps its whole fibre tree, every effect's
 *   cleanup unrun and every subscription live, unreachable for collection.
 * - Props that never reach `createElement` produce a component rendered with
 *   nothing, which looks like an authoring mistake rather than an adapter one.
 * - A module namespace passed to `createElement` renders nothing and warns
 *   about an invalid element type, which is a long way from the real cause.
 */

import { describe, expect, it } from "vitest";

import { reactIsland, type ReactDomRuntime, type ReactRuntime } from "../../src/adapters/react";

interface FakeRoot {
  container: Element;
  rendered: unknown[];
  unmounts: number;
}

interface FakeElement {
  type: unknown;
  props: unknown;
}

function fakeReact(): {
  runtime: ReactRuntime;
  dom: ReactDomRuntime;
  roots: FakeRoot[];
} {
  const roots: FakeRoot[] = [];

  return {
    roots,
    runtime: {
      createElement(type, props): FakeElement {
        return { type, props };
      },
    },
    dom: {
      createRoot(container) {
        const root: FakeRoot = { container, rendered: [], unmounts: 0 };
        roots.push(root);
        return {
          render(node) {
            root.rendered.push(node);
          },
          unmount() {
            root.unmounts += 1;
          },
        };
      },
    },
  };
}

function target(): HTMLElement {
  return document.createElement("div");
}

function islandWith(react: ReturnType<typeof fakeReact>, component: () => unknown) {
  return reactIsland({
    component,
    runtime: () => Promise.resolve(react.runtime),
    dom: () => Promise.resolve(react.dom),
  });
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    expect(islandWith(fakeReact(), () => ({})).name).toBe("react");
  });

  it("can be given a different token", () => {
    const island = reactIsland({
      name: "react-chart",
      component: () => ({}),
      runtime: () => Promise.resolve(fakeReact().runtime),
      dom: () => Promise.resolve(fakeReact().dom),
    });

    expect(island.name).toBe("react-chart");
  });
});

describe("mounting", () => {
  it("creates a root on the island's element", async () => {
    const react = fakeReact();
    const element = target();

    await islandWith(react, () => ({})).mount(element, {});

    expect(react.roots[0]?.container).toBe(element);
  });

  it("renders the component with the slide's props", async () => {
    const react = fakeReact();
    const component = () => null;

    await islandWith(react, () => component).mount(target(), { title: "Q3" });

    expect(react.roots[0]?.rendered[0]).toEqual({
      type: component,
      props: { title: "Q3" },
    });
  });

  it("renders exactly once", async () => {
    const react = fakeReact();

    await islandWith(react, () => ({})).mount(target(), {});

    expect(react.roots[0]?.rendered).toHaveLength(1);
  });

  it("unwraps the default export of a dynamically imported component", async () => {
    const react = fakeReact();
    const component = () => null;

    await islandWith(react, () => Promise.resolve({ default: component })).mount(target(), {});

    expect((react.roots[0]!.rendered[0] as FakeElement).type).toBe(component);
  });

  it("accepts a function component written inline", async () => {
    // A React component is a function, so it must not be mistaken for a module
    // namespace and unwrapped.
    const react = fakeReact();
    const component = () => null;

    await islandWith(react, () => component).mount(target(), {});

    expect((react.roots[0]!.rendered[0] as FakeElement).type).toBe(component);
  });
});

describe("unmounting", () => {
  it("unmounts the root", async () => {
    const react = fakeReact();

    const handle = await islandWith(react, () => ({})).mount(target(), {});
    handle.unmount();

    expect(react.roots[0]?.unmounts).toBe(1);
  });
});

describe("failing", () => {
  it("rejects when react cannot be loaded", async () => {
    const react = fakeReact();
    const island = reactIsland({
      component: () => ({}),
      runtime: () => Promise.reject(new Error("react is not installed")),
      dom: () => Promise.resolve(react.dom),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("react is not installed");
  });

  it("rejects when react-dom cannot be loaded", async () => {
    // Separate loaders because they are separately versioned packages: a deck
    // that has one and not the other should say which.
    const react = fakeReact();
    const island = reactIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(react.runtime),
      dom: () => Promise.reject(new Error("react-dom is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("react-dom is not installed");
  });

  it("creates no root when the component cannot be loaded", async () => {
    // Failing before `createRoot` matters: a root created and then abandoned
    // is the leak this adapter exists to prevent.
    const react = fakeReact();
    const island = reactIsland({
      component: () => Promise.reject(new Error("chunk load failed")),
      runtime: () => Promise.resolve(react.runtime),
      dom: () => Promise.resolve(react.dom),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("chunk load failed");
    expect(react.roots).toHaveLength(0);
  });
});
