/**
 * Svelte, as one slide's island.
 *
 * Written against Svelte 5's functional API — `mount(component, { target,
 * props })` and `unmount(instance)`. Svelte 4's class form is a different
 * contract, not a variation on this one, and detecting between them would put
 * a version fork in the adapter that no deck asked for. A deck on Svelte 4 can
 * write an `IslandDefinition` by hand; it is four lines.
 *
 * `unmount` returns a promise when there are outro transitions to play. It is
 * deliberately not awaited: by the time this runs the slide has already gone,
 * so waiting for an animation nobody can see would only delay releasing what
 * the component holds — and `IslandHandle.unmount` is synchronous precisely so
 * teardown cannot be left half-done in a pending promise.
 *
 * Svelte is an optional peer dependency and is not installed here, so its API
 * is declared structurally and imported through a variable specifier.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer, resolveDefault, type ComponentLoader } from "./peer";

/** The part of the `svelte` module this adapter uses. */
export interface SvelteRuntime {
  mount(component: unknown, options: { target: Element; props?: IslandProps }): unknown;
  unmount(instance: unknown, options?: { outro?: boolean }): unknown;
}

export interface SvelteIslandOptions {
  /** Loads the component. Deferred so it is fetched with Svelte rather than before it. */
  component: ComponentLoader;
  /** The token a slide selects this with. */
  name?: string;
  /** Substitutes the framework module. Exists so this is testable where Svelte is not installed. */
  runtime?: () => Promise<SvelteRuntime>;
}

export function svelteIsland(options: SvelteIslandOptions): IslandDefinition {
  const loadRuntime = options.runtime ?? (() => importPeer<SvelteRuntime>("svelte"));

  return {
    name: options.name ?? "svelte",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      const [runtime, loaded] = await Promise.all([loadRuntime(), options.component()]);

      const instance = runtime.mount(resolveDefault(loaded), { target, props });

      return {
        unmount: () => {
          runtime.unmount(instance);
        },
      };
    },
  };
}
