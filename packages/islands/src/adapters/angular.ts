/**
 * Angular, as one slide's island.
 *
 * Angular is the one framework here that a deck cannot reach without changing
 * its own build, and the two halves of that are worth separating.
 *
 * **The runtime half is like the others.** An application for the environment,
 * a component created into the island's element, the view attached so it is
 * checked more than once, and both destroyed on the way out. Nothing is
 * imported statically, so a deck that never registers this island resolves no
 * Angular package and installs none.
 *
 * **The compile half is not**, and it is worse than it first looks. `@Component`
 * is a decorator that Angular's own compiler lowers into a component
 * definition; transpiling it — which is all esbuild, rolldown, or `tsc` do —
 * leaves a plain class with no definition on it, and creating a component from
 * that class throws rather than rendering. That much was expected. The part
 * that removes the last workaround is that Angular's *own* published packages
 * ship partially compiled: `@angular/platform-browser` does not evaluate as an
 * ES module at all until Angular's linker has processed it. So there is no
 * configuration-free path here, not even one that pre-compiles the component by
 * hand. A deck with an Angular island needs an Angular plugin in its own Vite
 * config, and that plugin is doing two jobs rather than one.
 *
 * None of the other four impose that. A Vue or React component can be written
 * with no plugin at all, Svelte's is a per-file transform, and all four ship
 * modules a bundler can consume as they are. Angular's compiler type-checks
 * templates, so it holds a whole TypeScript program instead. The cost is
 * confined to decks that opt in — no slidx package knows Angular exists, and
 * the plugin belongs to the deck author's config rather than to
 * `@slidx/vite-plugin` — but within such a deck it is not a small addition, and
 * an author who wants an island for free should reach for one of the others.
 *
 * ## Change detection
 *
 * A slide is static HTML with no application around it, so nothing ticks an
 * Angular island except Angular. The choice was between zoneless change
 * detection and an `NgZone` per island, and the second is not what it sounds
 * like: zone.js is not per island. Importing it patches the page's `setTimeout`,
 * `Promise`, `addEventListener`, and `requestAnimationFrame` once and globally,
 * for every other slide and for the deck's own runtime — the timer, the keymap,
 * the mirroring channel — none of which asked for it, and each of whose tasks
 * would then schedule a change-detection pass for a component that is not on
 * screen. An island is supposed to be paid for by the slide that opted into it,
 * and a global patch cannot be.
 *
 * So this adapter installs the zoneless provider itself rather than leaving it
 * to the deck, and Angular 20 is the floor because that is the release where it
 * stopped being experimental. What it costs the author is real and belongs in
 * the open: a component that mutates a plain field from a `setTimeout` will not
 * re-render. Signals, the async pipe, template event bindings, and
 * `markForCheck` all notify the scheduler; a bare assignment does not. Nothing
 * here can paper over that — a deck that genuinely needs zone.js should write
 * its own `IslandDefinition` and accept that it patches the whole page.
 *
 * ## Props
 *
 * Inputs are set through the component ref rather than assigned onto the
 * instance, because an assigned field is invisible to Angular: `ngOnChanges`
 * never runs and a signal input never updates. That also makes this the one
 * adapter that validates a slide's props, since Angular is the one framework
 * that knows which of them are inputs. It throws on an undeclared one in a
 * development build, and that is allowed to propagate: the hydrator restores
 * the slide's placeholder and reports Angular's own message, which names the
 * input. A production build ignores it instead, which is Angular's decision
 * rather than this adapter's.
 *
 * Angular is an optional peer dependency, so its API is declared structurally
 * and reached through a variable specifier. This package must not resolve it,
 * and a deck that has no Angular must never be asked to.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer, resolveDefault, type ComponentLoader } from "./peer";

/** The part of an Angular component reference this adapter uses. */
interface AngularComponentRef {
  /** Handed to `attachView`, which is what subjects the component to change detection. */
  readonly hostView: unknown;
  setInput(name: string, value: unknown): void;
  destroy(): void;
}

/** The part of an Angular application reference this adapter uses. */
interface AngularApplicationRef {
  /** The application's environment injector, which the component is created from. */
  readonly injector: unknown;
  attachView(view: unknown): void;
  destroy(): void;
}

/** The part of the Angular core module this adapter uses. */
export interface AngularCoreRuntime {
  /**
   * Optional in the type because absence is a case this adapter handles: it is
   * how an Angular below the version floor presents itself.
   */
  provideZonelessChangeDetection?(): unknown;

  createComponent(
    component: unknown,
    options: { hostElement?: Element; environmentInjector: unknown },
  ): AngularComponentRef;
}

/** The part of the Angular browser platform module this adapter uses. */
export interface AngularPlatformRuntime {
  createApplication(options?: { providers: unknown[] }): Promise<AngularApplicationRef>;
}

export interface AngularIslandOptions {
  /** Loads the component. Deferred so it is fetched with Angular rather than before it. */
  component: ComponentLoader;

  /** The token a slide selects this with. */
  name?: string;

  /**
   * Environment providers for this island's application.
   *
   * `provideHttpClient` and its siblings return providers Angular refuses
   * inside a component's own `providers`, so an application is the only place
   * they can go and this is the only way in. They are added after the zoneless
   * provider, which is not negotiable — see above.
   */
  providers?: unknown[];

  /** Substitutes the core module. Exists so this is testable without bootstrapping Angular. */
  core?: () => Promise<AngularCoreRuntime>;

  /** Substitutes the browser platform module, for the same reason. */
  platform?: () => Promise<AngularPlatformRuntime>;
}

export function angularIsland(options: AngularIslandOptions): IslandDefinition {
  const loadCore = options.core ?? (() => importPeer<AngularCoreRuntime>("@angular/core"));
  const loadPlatform =
    options.platform ?? (() => importPeer<AngularPlatformRuntime>("@angular/platform-browser"));

  return {
    name: options.name ?? "angular",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      // In parallel: three round trips on a slide reached mid-talk are the
      // whole of the delay the audience sees, and none depends on the others.
      const [core, platform, loaded] = await Promise.all([
        loadCore(),
        loadPlatform(),
        options.component(),
      ]);

      const zoneless = core.provideZonelessChangeDetection;

      // Checked before anything is created, so a deck below the floor is told
      // what is wrong rather than left with a bootstrap that fails further in.
      if (typeof zoneless !== "function") {
        throw new TypeError(
          "the angular island runs zoneless, which requires Angular 20 or newer; " +
            "this one has no provideZonelessChangeDetection",
        );
      }

      const application = await platform.createApplication({
        providers: [zoneless(), ...(options.providers ?? [])],
      });

      const component = create(core, application, resolveDefault(loaded), target, props);

      return {
        unmount: () => {
          // Ordered: the component first, so its `ngOnDestroy` runs while the
          // injector it resolves against is still alive. The application after,
          // because it owns that injector, the effect scheduler, and every
          // teardown callback registered against it — one set per island
          // mounted, and none of it released by destroying the view alone.
          component.destroy();
          application.destroy();
        },
      };
    },
  };
}

/**
 * Everything between an application and a component that is actually live.
 *
 * Split out for the failure path rather than for tidiness: each of these three
 * steps can throw on an author's own component, and an application left running
 * behind one of them is a leak nothing later would collect.
 */
function create(
  core: AngularCoreRuntime,
  application: AngularApplicationRef,
  component: unknown,
  target: HTMLElement,
  props: IslandProps,
): AngularComponentRef {
  let created: AngularComponentRef | undefined;

  try {
    created = core.createComponent(component, {
      hostElement: target,
      environmentInjector: application.injector,
    });

    for (const [name, value] of Object.entries(props)) created.setInput(name, value);

    // The line this adapter exists for. A component created and not attached
    // renders once and is never checked again, and with no zone there is
    // nothing else that would ever notice.
    application.attachView(created.hostView);

    return created;
  } catch (error) {
    created?.destroy();
    application.destroy();
    throw error;
  }
}
