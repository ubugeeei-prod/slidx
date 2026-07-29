/**
 * Opt-in framework islands for slidx decks.
 *
 * A slidx deck is multi-page HTML and an audience slide ships zero JavaScript.
 * A framework therefore cannot be something the deck runs on; it has to be
 * something one slide asks for, mounted into one element, paid for by that
 * slide alone. This package is the contract for that and the loader that
 * honours it — not five frameworks in a trench coat.
 *
 * The adapters are separate entry points (`@slidx/islands/vue`,
 * `/react`, `/svelte`, `/three`) so importing one does not pull the others,
 * and every framework is an *optional* peer dependency: React must not be in
 * the install path of a deck that never mentions React.
 *
 * There is deliberately no Angular adapter. Angular components have to be
 * compiled by Angular's own toolchain and bootstrapped through an application
 * ref with a change-detection provider, which is a build-time requirement none
 * of the others impose on a deck. Half an Angular adapter — one that works
 * only for a hand-written, pre-compiled component — would be worse than none.
 */

export type { IslandDefinition, IslandHandle, IslandProps, IslandState } from "./contract";
export { ISLAND_ATTRIBUTE, PROPS_ATTRIBUTE, STATE_ATTRIBUTE } from "./contract";

export { hydrateIslands } from "./hydrate";
export type { HydrateOptions, IslandController } from "./hydrate";

export { parseProps } from "./props";
export type { ParsedProps } from "./props";

export { createRegistry, unknownIslandMessage } from "./registry";
export type { IslandRegistry } from "./registry";

export { consoleReporter, guardReporter } from "./report";
export type { IslandProblem, IslandProblemKind, IslandReporter } from "./report";

export { defaultVisibility, eagerVisibility, observerVisibility } from "./visibility";
export type { IslandVisibility, ObserverOptions, ObserverScope } from "./visibility";
