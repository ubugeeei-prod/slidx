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
 *
 * # What this can and cannot claim
 *
 * The search behind it matches a field *name*, and a name is not a type. The two
 * directions are therefore not equally trustworthy, and that decides what is
 * worth reporting at all:
 *
 * - **"Nothing reads this name anywhere"** is reliable. If `.contents` appears in
 *   no file outside a test, `Hover.contents` is certainly unread.
 * - **"Something reads this"** is not. Any unrelated struct with a field of the
 *   same name satisfies it. `slidx_qr` having a `layout()` method was enough to
 *   make `Slide::layout` look read for two runs, and merging shell completions
 *   was enough to make all five exemptions below look read at once, because some
 *   new file happened to contain `.contents` and `.result`.
 *
 * So there is no check for "this exemption is no longer needed": the tool cannot
 * know, and one that guessed reported every entry here as stale while the
 * summary beside it said nothing read them. What it can check is that an
 * exemption still names a field that exists, which catches the rot that actually
 * happens — a rename, or a struct deleted with its reason left behind.
 */

/**
 * Write-only by design, and why.
 *
 * Every entry is something slidx serialises for a reader that is not in this
 * workspace — a language server's client, or whoever consumes `--json`. It gets
 * filled in, written out, and never looked at again from here. A reason is
 * required because a bare list of names is how an exemption outlives the reason
 * for it.
 */
export const WRITE_ONLY = new Map([
  [
    "crates/slidx_cli/src/grep.rs:Hit.project",
    "in the --json payload so a caller can jump to the deck; the report prints the deck's name",
  ],
  [
    "crates/slidx_cli/src/grep.rs:Hit.slide_id",
    "in the --json payload so a hit is a link; a person reads the slide number instead",
  ],
  [
    "crates/slidx_lsp/src/formatting.rs:TextEdit.new_text",
    "the replacement text in a formatting response, applied by the editor and never read back",
  ],
  [
    "crates/slidx_brand/src/mark.rs:Geometry.page_width",
    "published in assets/brand/tokens.json, where the documentation site reads the mark's grid",
  ],
  [
    "crates/slidx_brand/src/mark.rs:Geometry.min_px",
    "published in assets/brand/tokens.json as mark.minPx — the size below which the mark stops",
  ],
  [
    "crates/slidx_brand/src/wordmark.rs:Lockup.min_px",
    "published in assets/brand/tokens.json as lockup.minPx, for whoever places the lockup",
  ],
  [
    "crates/slidx_brand/src/tokens.rs:Tokens.lockup",
    "the lockup half of assets/brand/tokens.json; nothing in this workspace draws a lockup",
  ],
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
 * and finds exemptions that no longer name anything.
 *
 * `declared` is every public field the scan found rather than only the unread
 * ones, because that is what makes an orphaned exemption detectable: a renamed
 * field leaves an entry pointing at a name the workspace no longer has.
 */
export function classify(unreadKeys, declaredKeys, exemptions = new Set(WRITE_ONLY.keys())) {
  const declared = new Set(declaredKeys);

  return {
    unexplained: unreadKeys.filter((entry) => !exemptions.has(entry)),
    orphaned: [...exemptions].filter((entry) => !declared.has(entry)),
  };
}
