/**
 * Which fields nothing reads on purpose.
 *
 * A field that only its own tests touch is indistinguishable, from the outside,
 * between two very different things: a protocol payload whose only reader is an
 * editor in another process, and a feature someone wired half of. No search can
 * tell those apart, so the difference is recorded here by a person, with the
 * reason, and everything else is reported.
 *
 * Separate from the check that uses it so the classification can be tested
 * without running a scan of the workspace.
 */

/**
 * Write-only by design, and why.
 *
 * Every entry is a language-server payload: the server fills it in, serialises
 * the message, and never looks at it again. A reason is required because a bare
 * list of names is how an exemption outlives the reason for it.
 */
export const WRITE_ONLY = new Map([
  [
    "crates/slidx_lsp/src/protocol.rs:Message.jsonrpc",
    "JSON-RPC requires the version on every message; the client checks it, we only send it",
  ],
  [
    "crates/slidx_lsp/src/protocol.rs:Message.result",
    "the response half of the envelope, written on the way out and never parsed back",
  ],
  [
    "crates/slidx_lsp/src/completion.rs:CompletionItem.insert_text",
    "what the editor inserts when a completion is accepted, which only the editor can act on",
  ],
  [
    "crates/slidx_lsp/src/hover.rs:Hover.contents",
    "the hover body the editor renders; nothing here consumes its own hover",
  ],
  [
    "crates/slidx_lsp/src/symbols.rs:DocumentSymbol.selection_range",
    "where an editor puts the cursor when jumping to a symbol",
  ],
]);

/** How a field is named in the exemption list. */
export const keyOf = ({ file, struct, field }) => `${file}:${struct}.${field}`;

/**
 * Splits what nothing reads into what to report and what has been justified,
 * and finds exemptions that are no longer needed.
 *
 * The stale case is easy to get backwards: an exemption is stale when the field
 * it names *is* read, which is the absence of a key rather than the presence of
 * one. Written inline that inversion looked right and reported every exemption
 * as stale while the check beside it correctly said nothing read them.
 */
export function classify(unreadKeys, exemptions = new Set(WRITE_ONLY.keys())) {
  const unread = new Set(unreadKeys);

  return {
    unexplained: unreadKeys.filter((entry) => !exemptions.has(entry)),
    stale: [...exemptions].filter((entry) => !unread.has(entry)),
  };
}
