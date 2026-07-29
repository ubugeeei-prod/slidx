/**
 * Vue as one slide's island.
 *
 * Vue is an optional peer dependency and is deliberately *not* installed in
 * this repository — putting it here would defeat the point of the adapters
 * being separate entry points. So the framework is injected: these tests drive
 * a fake `vue` module and assert the adapter's own contract, which is all the
 * adapter has.
 *
 * The failure modes guarded here:
 *
 * - An app created and never unmounted keeps its reactive effects, its
 *   watchers, and any interval a component started, once per slide visited.
 * - `() => import("./Chart.vue")` resolves to a module namespace, not to a
 *   component. Forgetting to unwrap it mounts nothing and says nothing.
 * - A framework that fails to load has to reject, because rejecting is how the
 *   hydrator finds out it should keep the placeholder.
 */

import { describe, expect, it } from "vite-plus/test";

import { vueIsland, type VueRuntime } from "../../src/adapters/vue";

interface FakeApp {
  component: unknown;
  props: unknown;
  mountedInto: Element | null;
  unmounts: number;
}

function fakeVue(): { runtime: VueRuntime; apps: FakeApp[] } {
  const apps: FakeApp[] = [];

  return {
    apps,
    runtime: {
      createApp(component, props) {
        const app: FakeApp = { component, props: props, mountedInto: null, unmounts: 0 };
        apps.push(app);
        return {
          mount(target) {
            app.mountedInto = target;
            return app;
          },
          unmount() {
            app.unmounts += 1;
          },
        };
      },
    },
  };
}

function target(): HTMLElement {
  return document.createElement("div");
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    const island = vueIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(fakeVue().runtime),
    });

    expect(island.name).toBe("vue");
  });

  it("can be given a different token", () => {
    // A deck can register the same framework twice with different components,
    // which needs two names.
    const island = vueIsland({
      name: "vue-chart",
      component: () => ({}),
      runtime: () => Promise.resolve(fakeVue().runtime),
    });

    expect(island.name).toBe("vue-chart");
  });
});

describe("mounting", () => {
  it("creates the app with the component and the slide's props", async () => {
    const vue = fakeVue();
    const component = { name: "Chart" };
    const island = vueIsland({
      component: () => component,
      runtime: () => Promise.resolve(vue.runtime),
    });

    await island.mount(target(), { title: "Q3" });

    expect(vue.apps[0]?.component).toBe(component);
    expect(vue.apps[0]?.props).toEqual({ title: "Q3" });
  });

  it("mounts into the island's element", async () => {
    const vue = fakeVue();
    const element = target();
    const island = vueIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(vue.runtime),
    });

    await island.mount(element, {});

    expect(vue.apps[0]?.mountedInto).toBe(element);
  });

  it("unwraps the default export of a dynamically imported component", async () => {
    // `() => import("./Chart.vue")` is the form every framework's own
    // documentation uses, and it resolves to a namespace object.
    const vue = fakeVue();
    const component = { name: "Chart" };
    const island = vueIsland({
      component: () => Promise.resolve({ default: component }),
      runtime: () => Promise.resolve(vue.runtime),
    });

    await island.mount(target(), {});

    expect(vue.apps[0]?.component).toBe(component);
  });

  it("accepts a component object written inline", async () => {
    const vue = fakeVue();
    const component = { render: () => null };
    const island = vueIsland({
      component: () => component,
      runtime: () => Promise.resolve(vue.runtime),
    });

    await island.mount(target(), {});

    expect(vue.apps[0]?.component).toBe(component);
  });

  it("waits for the component and the framework together", async () => {
    // Two round trips on a slide reached mid-talk are the whole of the delay
    // the audience sees, and neither depends on the other.
    const order: string[] = [];
    const vue = fakeVue();
    const island = vueIsland({
      component: () => {
        order.push("component");
        return Promise.resolve({});
      },
      runtime: () => {
        order.push("runtime");
        return Promise.resolve(vue.runtime);
      },
    });

    await island.mount(target(), {});

    expect(order).toEqual(["runtime", "component"]);
  });
});

describe("unmounting", () => {
  it("unmounts the app", async () => {
    const vue = fakeVue();
    const island = vueIsland({
      component: () => ({}),
      runtime: () => Promise.resolve(vue.runtime),
    });

    const handle = await island.mount(target(), {});
    handle.unmount();

    expect(vue.apps[0]?.unmounts).toBe(1);
  });
});

describe("failing", () => {
  it("rejects when the framework cannot be loaded", async () => {
    // Rejecting is how the hydrator learns to keep the placeholder. Swallowing
    // it here would leave an empty box on the slide and nothing in the console.
    const island = vueIsland({
      component: () => ({}),
      runtime: () => Promise.reject(new Error("vue is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("vue is not installed");
  });

  it("rejects when the component cannot be loaded", async () => {
    const vue = fakeVue();
    const island = vueIsland({
      component: () => Promise.reject(new Error("chunk load failed")),
      runtime: () => Promise.resolve(vue.runtime),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("chunk load failed");
  });
});
