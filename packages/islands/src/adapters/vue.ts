/**
 * Vue, as one slide's island.
 *
 * Thin on purpose: `createApp`, `mount`, `unmount`. Everything else Vue can do
 * belongs to the component, not to the deck. The adapter's only real job is
 * that `unmount` is actually called — a Vue app left running holds its
 * reactive effects, its watchers, and any interval a component started, and a
 * deck that never unmounts accumulates one of each per slide visited.
 *
 * Vue is loaded through a variable specifier (see `peer.ts`) and typed
 * structurally, because it is an optional peer dependency and is not installed
 * here. The shapes below are the whole of the API this adapter touches; if
 * Vue's own types disagree with them, the deck's build says so at the call
 * site where the component is passed.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer, resolveDefault, type ComponentLoader } from "./peer";

/** The part of a Vue application instance this adapter uses. */
interface VueApp {
  mount(target: Element): unknown;
  unmount(): void;
}

/** The part of the `vue` module this adapter uses. */
export interface VueRuntime {
  createApp(component: unknown, props?: IslandProps): VueApp;
}

export interface VueIslandOptions {
  /** Loads the component. Deferred so it is fetched with Vue rather than before it. */
  component: ComponentLoader;
  /** The token a slide selects this with. */
  name?: string;
  /** Substitutes the framework module. Exists so this is testable where Vue is not installed. */
  runtime?: () => Promise<VueRuntime>;
}

export function vueIsland(options: VueIslandOptions): IslandDefinition {
  const loadRuntime = options.runtime ?? (() => importPeer<VueRuntime>("vue"));

  return {
    name: options.name ?? "vue",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      // In parallel: on a slide reached mid-talk these two round trips are the
      // whole of the delay the audience sees, and they do not depend on each
      // other. A rejection from either propagates, which is what the hydrator
      // needs in order to isolate it.
      const [runtime, loaded] = await Promise.all([loadRuntime(), options.component()]);

      const app = runtime.createApp(resolveDefault(loaded), props);
      app.mount(target);

      return { unmount: () => app.unmount() };
    },
  };
}
