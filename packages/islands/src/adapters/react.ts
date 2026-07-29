/**
 * React, as one slide's island.
 *
 * Two modules rather than one: `react` for `createElement` and `react-dom/client`
 * for the root. They are separate packages with independent versions, so they
 * are separate loaders — a deck pinning `react-dom` to a different major is a
 * real situation and should fail on that island, not on the import.
 *
 * The root is created on mount and unmounted on leave. This is the adapter
 * where forgetting that is most expensive: a React root keeps its whole fibre
 * tree, every effect's cleanup unrun, and every subscription live, and none of
 * it is reachable for collection while the root exists.
 *
 * React is an optional peer dependency and is not installed here, so its API is
 * declared structurally and imported through a variable specifier.
 */

import type { IslandDefinition, IslandHandle, IslandProps } from "../contract";
import { importPeer, resolveDefault, type ComponentLoader } from "./peer";

/** The part of a React root this adapter uses. */
interface ReactRoot {
  render(node: unknown): void;
  unmount(): void;
}

/** The part of the `react` module this adapter uses. */
export interface ReactRuntime {
  createElement(type: unknown, props?: IslandProps | null): unknown;
}

/** The part of the `react-dom/client` module this adapter uses. */
export interface ReactDomRuntime {
  createRoot(container: Element): ReactRoot;
}

export interface ReactIslandOptions {
  /** Loads the component. Deferred so it is fetched with React rather than before it. */
  component: ComponentLoader;
  /** The token a slide selects this with. */
  name?: string;
  /** Substitutes `react`. Exists so this is testable where React is not installed. */
  runtime?: () => Promise<ReactRuntime>;
  /** Substitutes `react-dom/client`, for the same reason. */
  dom?: () => Promise<ReactDomRuntime>;
}

export function reactIsland(options: ReactIslandOptions): IslandDefinition {
  const loadRuntime = options.runtime ?? (() => importPeer<ReactRuntime>("react"));
  const loadDom = options.dom ?? (() => importPeer<ReactDomRuntime>("react-dom/client"));

  return {
    name: options.name ?? "react",

    async mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle> {
      const [runtime, dom, loaded] = await Promise.all([
        loadRuntime(),
        loadDom(),
        options.component(),
      ]);

      const root = dom.createRoot(target);
      root.render(runtime.createElement(resolveDefault(loaded), props));

      return { unmount: () => root.unmount() };
    },
  };
}
