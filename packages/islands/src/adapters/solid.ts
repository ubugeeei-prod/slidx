/**
 * Solid, as one slide's island.
 *
 * `createComponent` belongs to `solid-js` and `render` belongs to
 * `solid-js/web`, so they are loaded separately just as Solid publishes them.
 * The disposer returned by `render` is the whole teardown contract: calling it
 * releases the island's owner, effects, subscriptions, and DOM.
 *
 * Both modules are optional peers and imported through variable specifiers.
 * A deck that never imports this entry point therefore installs, resolves, and
 * ships no Solid code.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer, resolveDefault, type ComponentLoader } from "./peer";

/** The part of the `solid-js` module this adapter uses. */
export interface SolidRuntime {
  createComponent(component: unknown, props: IslandProps): unknown;
}

/** The part of the `solid-js/web` module this adapter uses. */
export interface SolidWebRuntime {
  render(code: () => unknown, target: Element): () => void;
}

export interface SolidIslandOptions {
  /** Loads the component. Deferred so it is fetched with Solid rather than before it. */
  component: ComponentLoader;
  /** The token a slide selects this with. */
  name?: string;
  /** Substitutes `solid-js`. Exists so this is testable where Solid is not installed. */
  runtime?: () => Promise<SolidRuntime>;
  /** Substitutes `solid-js/web`, for the same reason. */
  web?: () => Promise<SolidWebRuntime>;
}

export function solidIsland(options: SolidIslandOptions): IslandDefinition {
  const loadRuntime = options.runtime ?? (() => importPeer<SolidRuntime>("solid-js"));
  const loadWeb = options.web ?? (() => importPeer<SolidWebRuntime>("solid-js/web"));

  return {
    name: options.name ?? "solid",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      const [runtime, web, loaded] = await Promise.all([
        loadRuntime(),
        loadWeb(),
        options.component(),
      ]);

      const dispose = web.render(
        () => runtime.createComponent(resolveDefault(loaded), props),
        target,
      );

      return { unmount: dispose };
    },
  };
}
