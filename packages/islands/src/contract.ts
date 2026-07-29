/**
 * The boundary between a deck and a framework.
 *
 * An audience slide ships no JavaScript. That is the property slidx is sold on
 * and a test asserts it directly, so a framework cannot be a runtime the deck
 * carries — it has to be something one slide opts into and pays for alone.
 * This module is the whole of that opt-in: a name a slide can select, and a
 * function that puts something into an element and hands back a way to take it
 * down again.
 *
 * It is deliberately smaller than any framework's own mounting API. Everything
 * an integration needs beyond this — a compiler plugin, a component loader, a
 * renderer — belongs to the integration, because the deck must not link
 * against it. If this interface grows, a framework has leaked into the deck.
 *
 * The attributes live here too. They are the other end of the same contract:
 * the compiler writes them and the browser reads them, and neither half means
 * anything without the other. Renaming one is a change to a wire format
 * between two languages, not a refactor.
 */

/**
 * Data handed to a component, parsed from the slide's markup.
 *
 * `unknown` rather than a generic: props cross a JSON boundary written by an
 * author, so nothing here can be trusted to have a shape. The integration is
 * the only place that knows what its component expects.
 */
export type IslandProps = Record<string, unknown>;

/** What an integration returns so its island can be taken down again. */
export interface IslandHandle {
  /**
   * Releases everything the mount acquired.
   *
   * Called when the slide leaves the screen, so it has to be complete: a
   * WebGL context, a `requestAnimationFrame` loop, or an interval that
   * survives forty slides exhausts the machine mid-talk, and the failure looks
   * like the deck rather than like the island.
   */
  unmount(): void;
}

/** One framework, as a deck opts into it. */
export interface IslandDefinition {
  /** Frontmatter/mark token that selects it, e.g. "vue". */
  readonly name: string;

  /**
   * Called in the browser to mount the component into `target`.
   *
   * Async because the point of an island is that the framework is fetched when
   * the slide is reached, not when the deck loads. Rejecting is a supported
   * outcome — the hydrator isolates it — so an integration should not swallow
   * its own errors to be polite.
   */
  mount(target: HTMLElement, props: IslandProps): Promise<IslandHandle>;
}

/** Marks an element as an island. The value is the definition's name. */
export const ISLAND_ATTRIBUTE = "data-slidx-island";

/** Holds the island's props as a JSON object. Absent means no props. */
export const PROPS_ATTRIBUTE = "data-slidx-island-props";

/**
 * Reflects where an island is in its lifecycle, for CSS and for diagnostics.
 *
 * Written rather than kept private because a theme needs it: an island that is
 * still fetching its framework should be able to say so, and one that failed
 * should be able to look deliberate instead of looking like a rendering bug.
 */
export const STATE_ATTRIBUTE = "data-slidx-island-state";

/**
 * An island's lifecycle.
 *
 * `failed` is terminal on purpose. A mount that failed once fails again, and
 * retrying it every time the slide comes back floods the console a speaker
 * might be reading and re-runs whatever side effect broke it.
 */
export type IslandState = "idle" | "mounting" | "mounted" | "failed";
