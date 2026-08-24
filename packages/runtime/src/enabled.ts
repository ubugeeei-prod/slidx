/**
 * The mark that says the runtime arrived.
 *
 * Its own module rather than a pair of lines in the barrel, because two entries
 * re-export it: the package's API and the narrower one the plugin emits. A
 * symbol defined *in* a barrel drags whatever that barrel imports along with
 * it — which is exactly the 47% `emitted.ts` exists to stop shipping.
 */

/**
 * Attribute set on `<html>` as soon as the runtime loads.
 *
 * The staging CSS is gated on it, so a deck whose script never arrives — a
 * venue with no network, a blocked bundle — shows every element rather than a
 * slide that is mostly invisible.
 */
export const JS_ATTRIBUTE = "data-slidx-js";

/** Marks the document as script-enabled so staging CSS takes effect. */
export function markScriptEnabled(document: Document): void {
  document.documentElement.setAttribute(JS_ATTRIBUTE, "");
}
