/**
 * Opt-in framework islands for slidx decks.
 *
 * A slidx deck is multi-page HTML and an audience slide ships zero JavaScript.
 * A framework therefore cannot be something the deck runs on; it has to be
 * something one slide asks for, mounted into one element, paid for by that
 * slide alone. This package is the contract for that and the loader that
 * honours it — not six frameworks in a trench coat.
 *
 * The adapters are separate entry points (`@slidx/islands/vue`,
 * `/react`, `/svelte`, `/solid`, `/three`, `/angular`) so importing one does
 * not pull the others, and every framework is an *optional* peer dependency:
 * React must not be in the install path of a deck that never mentions React.
 *
 * Angular is the one that costs a deck more than an import. Its components have
 * to be compiled by Angular's own toolchain, so a deck with an Angular island
 * adds Angular's compiler to its own Vite config, and it runs zoneless because
 * zone.js is a page-wide patch rather than an island-sized one. Neither is true
 * of the other five. The cost stops at the deck that opted in — nothing in
 * `@slidx/*` knows Angular exists — and `adapters/angular.ts` states it in
 * full.
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
