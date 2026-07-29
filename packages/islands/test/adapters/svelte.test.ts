/**
 * Svelte as one slide's island.
 *
 * Svelte is an optional peer dependency and is not installed here, so the
 * module is injected. The adapter is written against Svelte 5's functional
 * `mount`/`unmount`, which is the whole of the contract these tests state.
 *
 * The failure modes guarded here:
 *
 * - `unmount` takes the instance, not the component. Handing it the wrong one
 *   silently leaves the component running.
 * - `unmount` returns a promise when there are outro transitions. Awaiting it
 *   would delay releasing what the component holds for an animation playing on
 *   a slide the audience has already left, and `IslandHandle.unmount` is
 *   synchronous so teardown cannot be left pending.
 * - A module namespace passed as the component mounts nothing.
 */

import { describe, expect, it } from "vite-plus/test";

import { svelteIsland, type SvelteRuntime } from "../../src/adapters/svelte";

interface FakeMount {
  component: unknown;
  target: Element;
  props: unknown;
  instance: object;
}

function fakeSvelte(options: { unmount?: () => unknown } = {}): {
  runtime: SvelteRuntime;
  mounts: FakeMount[];
  unmounted: unknown[];
} {
  const mounts: FakeMount[] = [];
  const unmounted: unknown[] = [];

  return {
    mounts,
    unmounted,
    runtime: {
      mount(component, mountOptions) {
        const instance = {};
        mounts.push({
          component,
          target: mountOptions.target,
          props: mountOptions.props,
          instance,
        });
        return instance;
      },
      unmount(instance) {
        unmounted.push(instance);
        return options.unmount?.();
      },
    },
  };
}

function target(): HTMLElement {
  return document.createElement("div");
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    const svelte = fakeSvelte();
    const island = svelteIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    expect(island.name).toBe("svelte");
  });

  it("can be given a different token", () => {
    const svelte = fakeSvelte();
    const island = svelteIsland({
      name: "svelte-demo",
      component: () => ({}),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    expect(island.name).toBe("svelte-demo");
  });
});

describe("mounting", () => {
  it("mounts the component into the island's element with its props", async () => {
    const svelte = fakeSvelte();
    const component = { name: "Counter" };
    const element = target();
    const island = svelteIsland({
      component: () => component,
      runtime: () => Promise.resolve(svelte.runtime),
    });

    await island.mount(element, { start: 3 });

    expect(svelte.mounts[0]?.component).toBe(component);
    expect(svelte.mounts[0]?.target).toBe(element);
    expect(svelte.mounts[0]?.props).toEqual({ start: 3 });
  });

  it("unwraps the default export of a dynamically imported component", async () => {
    const svelte = fakeSvelte();
    const component = { name: "Counter" };
    const island = svelteIsland({
      component: () => Promise.resolve({ default: component }),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    await island.mount(target(), {});

    expect(svelte.mounts[0]?.component).toBe(component);
  });
});

describe("unmounting", () => {
  it("unmounts the instance rather than the component", async () => {
    const svelte = fakeSvelte();
    const island = svelteIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    const handle = await island.mount(target(), {});
    handle.unmount();

    expect(svelte.unmounted).toEqual([svelte.mounts[0]?.instance]);
  });

  it("does not wait for an outro transition", async () => {
    // The slide is already gone. Waiting for an animation nobody can see would
    // only delay releasing what the component holds.
    let resolved = false;
    const svelte = fakeSvelte({
      unmount: () =>
        new Promise<void>((resolve) =>
          setTimeout(() => {
            resolved = true;
            resolve();
          }, 0),
        ),
    });
    const island = svelteIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    const handle = await island.mount(target(), {});
    handle.unmount();

    expect(resolved).toBe(false);
    expect(svelte.unmounted).toHaveLength(1);
  });
});

describe("failing", () => {
  it("rejects when svelte cannot be loaded", async () => {
    const island = svelteIsland({
      component: () => ({}),
      runtime: () => Promise.reject(new Error("svelte is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("svelte is not installed");
  });

  it("mounts nothing when the component cannot be loaded", async () => {
    const svelte = fakeSvelte();
    const island = svelteIsland({
      component: () => Promise.reject(new Error("chunk load failed")),
      runtime: () => Promise.resolve(svelte.runtime),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("chunk load failed");
    expect(svelte.mounts).toHaveLength(0);
  });
});
