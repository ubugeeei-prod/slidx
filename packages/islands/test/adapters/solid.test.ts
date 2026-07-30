/**
 * Solid as one slide's island.
 *
 * The framework is injected because it is an optional peer. These tests pin
 * the two-module boundary, component/props handoff, and disposer that prevents
 * reactive owners from surviving the slide that created them.
 */

import { describe, expect, it } from "vite-plus/test";

import { solidIsland, type SolidRuntime, type SolidWebRuntime } from "../../src/adapters/solid";

interface FakeRoot {
  target: Element;
  rendered: unknown[];
  disposals: number;
}

function fakeSolid(): {
  runtime: SolidRuntime;
  web: SolidWebRuntime;
  roots: FakeRoot[];
} {
  const roots: FakeRoot[] = [];

  return {
    roots,
    runtime: {
      createComponent(component, props) {
        return { component, props };
      },
    },
    web: {
      render(code, target) {
        const root: FakeRoot = { target, rendered: [code()], disposals: 0 };
        roots.push(root);
        return () => {
          root.disposals += 1;
        };
      },
    },
  };
}

function target(): HTMLElement {
  return document.createElement("div");
}

function islandWith(solid: ReturnType<typeof fakeSolid>, component: () => unknown) {
  return solidIsland({
    component,
    runtime: () => Promise.resolve(solid.runtime),
    web: () => Promise.resolve(solid.web),
  });
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    expect(islandWith(fakeSolid(), () => ({})).name).toBe("solid");
  });

  it("can be given a different token", () => {
    const solid = fakeSolid();
    const island = solidIsland({
      name: "solid-chart",
      component: () => ({}),
      runtime: () => Promise.resolve(solid.runtime),
      web: () => Promise.resolve(solid.web),
    });

    expect(island.name).toBe("solid-chart");
  });
});

describe("mounting", () => {
  it("renders into the island's element", async () => {
    const solid = fakeSolid();
    const element = target();

    await islandWith(solid, () => ({})).mount(element, {});

    expect(solid.roots[0]?.target).toBe(element);
  });

  it("creates the component with the slide's props", async () => {
    const solid = fakeSolid();
    const component = () => null;

    await islandWith(solid, () => component).mount(target(), { title: "Q3", live: true });

    expect(solid.roots[0]?.rendered).toEqual([{ component, props: { title: "Q3", live: true } }]);
  });

  it("unwraps the default export of a dynamically imported component", async () => {
    const solid = fakeSolid();
    const component = () => null;

    await islandWith(solid, () => Promise.resolve({ default: component })).mount(target(), {});

    expect((solid.roots[0]!.rendered[0] as { component: unknown }).component).toBe(component);
  });

  it("accepts a function component written inline", async () => {
    const solid = fakeSolid();
    const component = () => null;

    await islandWith(solid, () => component).mount(target(), {});

    expect((solid.roots[0]!.rendered[0] as { component: unknown }).component).toBe(component);
  });
});

describe("unmounting", () => {
  it("disposes the Solid root", async () => {
    const solid = fakeSolid();
    const handle = await islandWith(solid, () => ({})).mount(target(), {});

    handle.unmount();

    expect(solid.roots[0]?.disposals).toBe(1);
  });
});

describe("failing", () => {
  it("rejects when solid-js cannot be loaded", async () => {
    const solid = fakeSolid();
    const island = solidIsland({
      component: () => ({}),
      runtime: () => Promise.reject(new Error("solid-js is not installed")),
      web: () => Promise.resolve(solid.web),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("solid-js is not installed");
    expect(solid.roots).toHaveLength(0);
  });

  it("rejects when solid-js/web cannot be loaded", async () => {
    const solid = fakeSolid();
    const island = solidIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(solid.runtime),
      web: () => Promise.reject(new Error("solid-js/web is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("solid-js/web is not installed");
    expect(solid.roots).toHaveLength(0);
  });

  it("creates no root when the component cannot be loaded", async () => {
    const solid = fakeSolid();
    const island = solidIsland({
      component: () => Promise.reject(new Error("chunk load failed")),
      runtime: () => Promise.resolve(solid.runtime),
      web: () => Promise.resolve(solid.web),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("chunk load failed");
    expect(solid.roots).toHaveLength(0);
  });
});
