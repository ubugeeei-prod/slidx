/**
 * Bringing a slide's islands to life, and taking them down again.
 *
 * Everything here exists to keep one broken framework from costing a speaker
 * the room. The rules it enforces, in the order they matter:
 *
 * - **An island that fails does not take the slide with it.** Its placeholder
 *   content — the markup that was already there, the thing the deck renders
 *   with no JavaScript at all — is put back, and the rest of the slide
 *   presents. Nothing thrown by an integration escapes this module.
 * - **Mounting is deferred until the slide is on screen.** The framework
 *   import is the expensive part, and slide 1 must not pay for slide 40.
 * - **Leaving a slide unmounts it.** A GL context or an animation loop that
 *   survives the slide it belongs to accumulates, and by the end of a talk the
 *   machine is the problem.
 * - **Mounting is idempotent.** A re-render, a step change, or a scroll that
 *   re-fires visibility must not stack a second component on the first.
 *
 * Everything cheap is done eagerly, at hydration: the name is resolved and the
 * props are parsed while the deck is loading. A typo found when the slide is
 * reached is a typo found on stage.
 */

import {
  ISLAND_ATTRIBUTE,
  PROPS_ATTRIBUTE,
  STATE_ATTRIBUTE,
  type IslandDefinition,
  type IslandHandle,
  type IslandProps,
  type IslandState,
} from "./contract";
import { parseProps } from "./props";
import { consoleReporter, guardReporter, type IslandProblem, type IslandReporter } from "./report";
import { unknownIslandMessage, type IslandRegistry } from "./registry";
import { defaultVisibility, type IslandVisibility } from "./visibility";

export interface HydrateOptions {
  /** The frameworks this deck opted into. */
  registry: IslandRegistry;
  /** Injected so lazy mounting is testable without a layout engine. */
  visibility?: IslandVisibility;
  /** Where failures go. Defaults to a console warning. */
  report?: IslandReporter;
}

export interface IslandController {
  /** The islands being managed, in document order. Excludes ones that could not be resolved. */
  readonly islands: readonly HTMLElement[];
  /** Where an island is in its lifecycle, or undefined if it is not managed. */
  stateOf(element: HTMLElement): IslandState | undefined;
  /** Resolves once no mount is in flight. Print and tests both need a settling point. */
  settled(): Promise<void>;
  /** Unmounts everything and stops watching. Safe to call twice. */
  destroy(): void;
}

/** One island's place in the world: what it is, what it showed before, and where it is now. */
interface Slot {
  readonly element: HTMLElement;
  readonly definition: IslandDefinition;
  readonly props: IslandProps;
  /** The markup the slide renders without JavaScript. Restored on failure and on unmount. */
  readonly placeholder: string;
  state: IslandState;
  handle: IslandHandle | null;
  /** The most recent answer from the observer, which a mount in flight may contradict. */
  visible: boolean;
  stopObserving: (() => void) | null;
}

export function hydrateIslands(root: ParentNode, options: HydrateOptions): IslandController {
  const report = guardReporter(options.report ?? consoleReporter());
  const visibility = options.visibility ?? defaultVisibility();
  const inFlight = new Set<Promise<void>>();
  const slots: Slot[] = [];
  let destroyed = false;

  function setState(slot: Slot, state: IslandState): void {
    slot.state = state;
    if (state === "idle") slot.element.removeAttribute(STATE_ATTRIBUTE);
    else slot.element.setAttribute(STATE_ATTRIBUTE, state);
  }

  /** Puts back what the slide showed before any framework touched it. */
  function restorePlaceholder(slot: Slot): void {
    slot.element.innerHTML = slot.placeholder;
  }

  function fail(slot: Slot, problem: Omit<IslandProblem, "element">): void {
    restorePlaceholder(slot);
    slot.handle = null;
    setState(slot, "failed");
    report({ ...problem, element: slot.element });
  }

  function startMount(slot: Slot): void {
    // The single line that makes mounting idempotent. `mounting` declines
    // because one is already on the way, `mounted` because one is already
    // there, and `failed` because it will fail the same way again.
    if (slot.state !== "idle" || destroyed) return;

    setState(slot, "mounting");

    const task = runMount(slot);
    inFlight.add(task);
    void task.finally(() => inFlight.delete(task));
  }

  async function runMount(slot: Slot): Promise<void> {
    let handle: IslandHandle;

    try {
      handle = await slot.definition.mount(slot.element, slot.props);
    } catch (cause) {
      fail(slot, {
        kind: "mount-failed",
        name: slot.definition.name,
        message: `the "${slot.definition.name}" island failed to mount`,
        cause,
      });
      return;
    }

    if (!isHandle(handle)) {
      // The component is on screen and working; only teardown is lost. Tearing
      // it down would be the worse trade, so this is a leak that is reported
      // once rather than a failure.
      report({
        kind: "invalid-handle",
        name: slot.definition.name,
        element: slot.element,
        message:
          `the "${slot.definition.name}" island resolved without an unmount function, ` +
          "so it cannot be released when the slide is left",
      });
      handle = { unmount: () => {} };
    }

    slot.handle = handle;
    setState(slot, "mounted");

    // The slide can be left while the framework is still being fetched. Without
    // this, a scene mounted after its slide has gone holds a GL context that
    // nothing will ever ask for again.
    if (!slot.visible || destroyed) startUnmount(slot);
  }

  function startUnmount(slot: Slot): void {
    // `mounting` is not handled here: the mount itself checks visibility when
    // it resolves, which is the only moment there is a handle to release.
    if (slot.state !== "mounted") return;

    const handle = slot.handle;
    slot.handle = null;

    // Moved out of `mounted` before the call, so a visibility change triggered
    // from inside `unmount` cannot start a second teardown of the same handle.
    setState(slot, "idle");

    try {
      handle?.unmount();
    } catch (cause) {
      // Whatever the integration held, it still holds. Marking the island
      // failed stops us from stacking a second mount on top of a teardown that
      // did not finish.
      setState(slot, "failed");
      report({
        kind: "unmount-failed",
        name: slot.definition.name,
        element: slot.element,
        message: `the "${slot.definition.name}" island failed to unmount and may still be running`,
        cause,
      });
    }

    restorePlaceholder(slot);
  }

  for (const element of root.querySelectorAll<HTMLElement>(`[${ISLAND_ATTRIBUTE}]`)) {
    const slot = resolve(element, options.registry, report);
    if (!slot) continue;

    slots.push(slot);
    slot.stopObserving = visibility.observe(element, (visible) => {
      slot.visible = visible;
      if (destroyed) return;
      if (visible) startMount(slot);
      else startUnmount(slot);
    });
  }

  return {
    islands: slots.map((slot) => slot.element),

    stateOf(element) {
      return slots.find((slot) => slot.element === element)?.state;
    },

    async settled() {
      // Drained rather than awaited once: a mount that resolves after its
      // slide has gone queues its own teardown as it settles.
      while (inFlight.size > 0) await Promise.all(inFlight);
    },

    destroy() {
      destroyed = true;

      for (const slot of slots) {
        slot.stopObserving?.();
        slot.stopObserving = null;
        slot.visible = false;
        startUnmount(slot);
      }
    },
  };
}

/**
 * Turns one marked element into something mountable, or reports why not.
 *
 * Both failures here are authoring or compiler mistakes rather than runtime
 * ones, so they are found while the deck loads and the element is then left
 * alone entirely — its placeholder is the slide's content, and an island that
 * can never mount should not be watched for a visibility change that would
 * mean nothing.
 */
function resolve(
  element: HTMLElement,
  registry: IslandRegistry,
  report: IslandReporter,
): Slot | null {
  const name = (element.getAttribute(ISLAND_ATTRIBUTE) ?? "").trim();

  if (name === "") {
    report({
      kind: "missing-name",
      name: "",
      element,
      message: `an element has ${ISLAND_ATTRIBUTE} but names no island`,
    });
    return null;
  }

  const definition = registry.lookup(name);

  if (!definition) {
    report({
      kind: "unknown-island",
      name,
      element,
      message: unknownIslandMessage(name, registry.names()),
    });
    return null;
  }

  const { props, problem } = parseProps(element.getAttribute(PROPS_ATTRIBUTE));

  if (problem !== undefined) {
    // Reported, then mounted anyway with nothing. A chart with no data still
    // leaves a readable slide; a chart that refused to mount leaves a hole.
    report({ kind: "invalid-props", name, element, message: `${name}: ${problem}` });
  }

  return {
    element,
    definition,
    props,
    placeholder: element.innerHTML,
    state: "idle",
    handle: null,
    visible: false,
    stopObserving: null,
  };
}

/** An integration is third-party code; what it resolved with has to be checked. */
function isHandle(value: unknown): value is IslandHandle {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as IslandHandle).unmount === "function"
  );
}
