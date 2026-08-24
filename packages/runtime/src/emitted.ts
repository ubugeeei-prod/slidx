/**
 * The entry every page of a deck downloads.
 *
 * Not the package's API — `index.ts` is that, and a deck author importing
 * `@slidxjs/runtime` still gets all of it. This is the narrower question of
 * **what a room downloads**, and the two had been the same file.
 *
 * Narrower again since `presenter.ts`: what is left here is what a *slide*
 * imports, plus `createStopCursor`, which only the presenter asks for and which
 * lives in a module every staged slide already ships.
 *
 * That cost 47% of the bundle. `readRuntime()` reads the packed entry as a
 * file and the plugin emits it whole, so it is never an input to the deck's own
 * Rollup graph and nothing ever sees that a page imports eleven names out of
 * forty. Emitting the barrel therefore shipped the presenter's camera, the
 * media level meter, the key table and the demo switch to every audience that
 * ever loaded a staged slide — 8.7KB gzipped of code no page could call.
 *
 * Reading the whole file rather than bundling it is deliberate and stays: it is
 * what makes the emitted module byte-identical across every page of a deck, so
 * a room fetches it once and every later slide is cache-warm. The fix is to
 * make the file smaller, not to make it per-page.
 *
 * # This list is checked, not trusted
 *
 * A name a page imports and this file does not export is a deck that breaks on
 * load, and a name here that no page imports is the 8.7KB again. Neither is
 * something to notice in review, so `scripts/check-reachable.mjs` asserts the
 * two sets are equal — the names `slidx_render` writes into its pages, against
 * the names below.
 *
 * So this file is not edited by hand except when a page's imports change, and
 * the check names exactly what to add or remove when they do.
 */

export { loadEffects } from "./effects";
export { markScriptEnabled } from "./enabled";
export { createMirror } from "./mirror";
export { createNavigator, LAST_STEP } from "./navigate";
export { createStage, createStopCursor } from "./stage";
