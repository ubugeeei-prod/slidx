/**
 * Angular as one slide's island.
 *
 * The framework is injected here, the way it is for the other four. Angular is
 * an optional peer dependency, so an adapter has to work against a package this
 * one does not depend on — and Angular in particular could not be bootstrapped
 * from a test even if it were depended on, because its published modules do not
 * evaluate until its own linker has processed them. Whether a deck that uses no
 * Angular carries any is a separate question, and it is asserted separately in
 * `packaging.test.ts`.
 *
 * The failure modes guarded here:
 *
 * - **A view that was never attached.** `createComponent` renders once. Without
 *   `attachView` the component is frozen at its first paint — a counter that
 *   never counts, on the slide where the speaker is demonstrating that it does.
 *   There is no zone here to hide the mistake, so nothing else catches it.
 * - **An application left alive.** Every island is its own `ApplicationRef`
 *   with its own injector, effect scheduler, and `DestroyRef` callbacks. One
 *   per slide visited, never released, is what this adapter is most exposed to.
 * - **A deck below the version floor.** Zoneless change detection is the whole
 *   design; on an Angular that does not provide it the mount has to say so
 *   rather than fail somewhere further in.
 */

import { describe, expect, it } from "vitest";

import {
  angularIsland,
  type AngularCoreRuntime,
  type AngularPlatformRuntime,
} from "../../src/adapters/angular";

interface FakeApplication {
  providers: readonly unknown[];
  attached: unknown[];
  destroys: number;
}

interface FakeComponent {
  type: unknown;
  host: Element | undefined;
  injector: unknown;
  inputs: Record<string, unknown>;
  destroys: number;
  hostView: object;
}

interface FakeAngular {
  core: AngularCoreRuntime;
  platform: AngularPlatformRuntime;
  applications: FakeApplication[];
  components: FakeComponent[];
  /** Every teardown call in the order it happened. */
  teardown: string[];
  /** Refuses this input the way a real component refuses an undeclared one. */
  refuseInput?: string;
}

function fakeAngular(overrides: { refuseInput?: string } = {}): FakeAngular {
  const applications: FakeApplication[] = [];
  const components: FakeComponent[] = [];
  const teardown: string[] = [];

  const fake: FakeAngular = {
    applications,
    components,
    teardown,
    refuseInput: overrides.refuseInput,

    core: {
      provideZonelessChangeDetection: () => ({ zoneless: true }),

      createComponent(component, options) {
        const created: FakeComponent = {
          type: component,
          host: options.hostElement,
          injector: options.environmentInjector,
          inputs: {},
          destroys: 0,
          hostView: { view: components.length },
        };
        components.push(created);

        return {
          hostView: created.hostView,
          setInput(name, value) {
            if (fake.refuseInput === name) {
              throw new Error(`Can't set value of the '${name}' input`);
            }
            created.inputs[name] = value;
          },
          destroy() {
            created.destroys += 1;
            teardown.push("component");
          },
        };
      },
    },

    platform: {
      createApplication(options) {
        const application: FakeApplication = {
          providers: options?.providers ?? [],
          attached: [],
          destroys: 0,
        };
        applications.push(application);

        return Promise.resolve({
          injector: { application: applications.length - 1 },
          attachView(view) {
            application.attached.push(view);
          },
          destroy() {
            application.destroys += 1;
            teardown.push("application");
          },
        });
      },
    },
  };

  return fake;
}

/** An Angular below the version floor: everything but the zoneless provider. */
function withoutZonelessProvider(angular: FakeAngular): AngularCoreRuntime {
  return { createComponent: angular.core.createComponent };
}

function target(): HTMLElement {
  return document.createElement("div");
}

function islandWith(angular: FakeAngular, component: () => unknown) {
  return angularIsland({
    component,
    core: () => Promise.resolve(angular.core),
    platform: () => Promise.resolve(angular.platform),
  });
}

describe("the definition", () => {
  it("answers to the token a slide writes", () => {
    expect(islandWith(fakeAngular(), () => class {}).name).toBe("angular");
  });

  it("can be given a different token", () => {
    // A deck can register the same framework twice with different components,
    // which needs two names.
    const angular = fakeAngular();
    const island = angularIsland({
      name: "angular-chart",
      component: () => class {},
      core: () => Promise.resolve(angular.core),
      platform: () => Promise.resolve(angular.platform),
    });

    expect(island.name).toBe("angular-chart");
  });
});

describe("mounting", () => {
  it("creates the component on the island's element", async () => {
    const angular = fakeAngular();
    const element = target();

    await islandWith(angular, () => class {}).mount(element, {});

    expect(angular.components[0]?.host).toBe(element);
  });

  it("creates the component from the application's own injector", async () => {
    // Not a fresh injector: the component has to see the providers the island
    // was given, and the zoneless scheduler lives in that same environment.
    const angular = fakeAngular();

    await islandWith(angular, () => class {}).mount(target(), {});

    expect(angular.components[0]?.injector).toEqual({ application: 0 });
  });

  it("attaches the view so change detection reaches the component after its first paint", async () => {
    // The line this adapter exists for. `createComponent` renders once and an
    // unattached view is never checked again; with no zone to tick the
    // application there is nothing else that would ever notice.
    const angular = fakeAngular();

    await islandWith(angular, () => class {}).mount(target(), {});

    expect(angular.applications[0]?.attached).toEqual([angular.components[0]?.hostView]);
  });

  it("gives the slide's props to the component as declared inputs", async () => {
    // `setInput` rather than assignment: a field assigned from outside is
    // invisible to Angular's input machinery, so `ngOnChanges` never runs and
    // a signal input never updates.
    const angular = fakeAngular();

    await islandWith(angular, () => class {}).mount(target(), { title: "Q3", live: true });

    expect(angular.components[0]?.inputs).toEqual({ title: "Q3", live: true });
  });

  it("sets no inputs when the slide declared no props", async () => {
    const angular = fakeAngular();

    await islandWith(angular, () => class {}).mount(target(), {});

    expect(angular.components[0]?.inputs).toEqual({});
  });

  it("unwraps the default export of a dynamically imported component", async () => {
    const angular = fakeAngular();
    const component = class Chart {};

    await islandWith(angular, () => Promise.resolve({ default: component })).mount(target(), {});

    expect(angular.components[0]?.type).toBe(component);
  });

  it("accepts a component class written inline", async () => {
    // An Angular component is a class, which is a function, and must not be
    // mistaken for a module namespace and unwrapped.
    const angular = fakeAngular();
    const component = class Chart {};

    await islandWith(angular, () => component).mount(target(), {});

    expect(angular.components[0]?.type).toBe(component);
  });

  it("waits for the component and the framework together", async () => {
    // Angular is the largest of the five runtimes, so the slide reached
    // mid-talk pays the most here and the round trips must overlap.
    const order: string[] = [];
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => {
        order.push("component");
        return Promise.resolve(class {});
      },
      core: () => {
        order.push("core");
        return Promise.resolve(angular.core);
      },
      platform: () => {
        order.push("platform");
        return Promise.resolve(angular.platform);
      },
    });

    await island.mount(target(), {});

    expect(order).toEqual(["core", "platform", "component"]);
  });
});

describe("change detection", () => {
  it("runs the island zoneless", async () => {
    // zone.js is not an island-sized cost. Importing it patches the page's
    // global async primitives for every other slide and for the deck's own
    // runtime, which is the opposite of one slide paying for its own choice.
    const angular = fakeAngular();

    await islandWith(angular, () => class {}).mount(target(), {});

    expect(angular.applications[0]?.providers).toContainEqual({ zoneless: true });
  });

  it("refuses to mount on an Angular with no zoneless provider, and names the floor", async () => {
    // Zoneless is the design, not a preference. On an older Angular the mount
    // would otherwise fail deep inside bootstrap with nothing pointing at the
    // version that caused it.
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => class {},
      core: () => Promise.resolve(withoutZonelessProvider(angular)),
      platform: () => Promise.resolve(angular.platform),
    });

    await expect(island.mount(target(), {})).rejects.toThrow(/Angular 20/);
  });

  it("creates no application when the zoneless provider is missing", async () => {
    // Bailing before `createApplication` matters: an application created and
    // then abandoned is the leak this adapter is most exposed to.
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => class {},
      core: () => Promise.resolve(withoutZonelessProvider(angular)),
      platform: () => Promise.resolve(angular.platform),
    });

    await expect(island.mount(target(), {})).rejects.toThrow();
    expect(angular.applications).toHaveLength(0);
  });

  it("keeps the deck's providers after its own so a deck can add DI", async () => {
    // `provideHttpClient` and friends return EnvironmentProviders, which
    // Angular rejects inside a component's own `providers`. `createApplication`
    // is the only place they can go, so the adapter has to forward them.
    const angular = fakeAngular();
    const http = { provider: "http" };
    const island = angularIsland({
      component: () => class {},
      providers: [http],
      core: () => Promise.resolve(angular.core),
      platform: () => Promise.resolve(angular.platform),
    });

    await island.mount(target(), {});

    expect(angular.applications[0]?.providers).toEqual([{ zoneless: true }, http]);
  });
});

describe("unmounting", () => {
  it("destroys the component before the application that owns its injector", async () => {
    // Ordered: a component destroyed after its environment injector has gone
    // runs `ngOnDestroy` against providers that are already released, so the
    // cleanup a component wrote is exactly the cleanup that throws.
    const angular = fakeAngular();

    const handle = await islandWith(angular, () => class {}).mount(target(), {});
    handle.unmount();

    expect(angular.teardown).toEqual(["component", "application"]);
  });

  it("destroys the application, not only the view", async () => {
    // The view is the visible half. The application holds the injector, the
    // effect scheduler, and every `DestroyRef` callback, and there is one per
    // island mounted.
    const angular = fakeAngular();

    const handle = await islandWith(angular, () => class {}).mount(target(), {});
    handle.unmount();

    expect(angular.components[0]?.destroys).toBe(1);
    expect(angular.applications[0]?.destroys).toBe(1);
  });
});

describe("failing", () => {
  it("rejects when @angular/core cannot be loaded", async () => {
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => class {},
      core: () => Promise.reject(new Error("@angular/core is not installed")),
      platform: () => Promise.resolve(angular.platform),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("@angular/core is not installed");
  });

  it("rejects when @angular/platform-browser cannot be loaded", async () => {
    // Separate loaders because they are separately resolved packages: a deck
    // that has one and not the other should be told which.
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => class {},
      core: () => Promise.resolve(angular.core),
      platform: () => Promise.reject(new Error("@angular/platform-browser is not installed")),
    });

    await expect(island.mount(target(), {})).rejects.toThrow(
      "@angular/platform-browser is not installed",
    );
  });

  it("creates no application when the component cannot be loaded", async () => {
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => Promise.reject(new Error("chunk load failed")),
      core: () => Promise.resolve(angular.core),
      platform: () => Promise.resolve(angular.platform),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("chunk load failed");
    expect(angular.applications).toHaveLength(0);
  });

  it("releases everything when a prop names something that is not an input", async () => {
    // Angular is the only one of the five that knows which props are real, and
    // it throws on an undeclared one in a development build. Left alone that
    // would abandon a live application on the way out.
    const angular = fakeAngular({ refuseInput: "titel" });
    const island = islandWith(angular, () => class {});

    await expect(island.mount(target(), { titel: "Q3" })).rejects.toThrow(/'titel' input/);

    expect(angular.teardown).toEqual(["component", "application"]);
  });

  it("destroys the application when the component itself cannot be created", async () => {
    const angular = fakeAngular();
    const island = angularIsland({
      component: () => class {},
      core: () =>
        Promise.resolve({
          ...angular.core,
          createComponent: () => {
            throw new Error("NG0906: is not a standalone component");
          },
        }),
      platform: () => Promise.resolve(angular.platform),
    });

    await expect(island.mount(target(), {})).rejects.toThrow("NG0906");
    expect(angular.applications[0]?.destroys).toBe(1);
  });
});
