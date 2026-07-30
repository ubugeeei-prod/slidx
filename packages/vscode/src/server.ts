/**
 * What the extension starts, and which documents it starts it for.
 *
 * Kept apart from `extension.ts` because nothing here imports `vscode`, and so
 * every decision this extension makes is a value a test can assert on rather
 * than something only an editor can observe.
 *
 * ## No language of its own
 *
 * A deck is Markdown. This extension contributes no language, no grammar and no
 * file association, and that is the design rather than an omission.
 *
 * Registering a `slidx` language would take those files away from whatever
 * Markdown tooling the author already has — their preview, their table
 * formatter, their spell checker — in exchange for a highlighter slidx would
 * then have to write. And it would have to write one, because a TextMate
 * grammar cannot read `EffectPreset::ALL`: every preset, transition, theme and
 * frontmatter key would be a second list, in a third language, going stale the
 * first time somebody adds a variant. The completion list is derived from the
 * Rust that defines those. A grammar could only be copied from it.
 *
 * The dialect's own constructs are already ordinary Markdown to a highlighter.
 * `<!-- step: fade -->` is a comment, `[3.2x]{#result}` is a link-like span,
 * `---` is a rule. They render as what they are.
 */

/** The subcommand that runs the language server. */
export const SERVER_COMMAND = "lsp";

/**
 * Which files this extension is for, as a glob.
 *
 * The same rule `slidx_lsp::deck` enforces, restated because a
 * `DocumentSelector` cannot call Rust — and pinned to it by a test that reads
 * this file. Filtering here saves the traffic; the server is what decides.
 *
 * Markdown under a `slides/` directory, which is the plugin's default `srcDir`
 * and the path `slidx lint`, `slidx fmt` and `slidx dev` all fall back to. A
 * `README.md` is not a deck, a note is not a deck, and an extension that put
 * slide diagnostics on either would deserve everything it got.
 */
export const DECK_GLOB = "**/slides/*.md";

/** The language identifier those files already have, and keep. */
export const LANGUAGE = "markdown";

/** How this client is identified in the output pane and in settings. */
export const CLIENT_ID = "slidx";
export const CLIENT_NAME = "slidx language server";

/**
 * The document filter the client registers.
 *
 * `scheme: file` and nothing else. An untitled buffer has no path, so no rule
 * about paths can say whether it is a deck; a `git:` or `vscode-remote:`
 * document from a diff view is a revision of a file rather than one somebody is
 * editing, and publishing diagnostics onto a diff is noise nobody asked for.
 */
export function documentSelector(): readonly DocumentFilter[] {
  return [{ scheme: "file", language: LANGUAGE, pattern: DECK_GLOB }];
}

export interface DocumentFilter {
  readonly scheme: string;
  readonly language: string;
  readonly pattern: string;
}

/** Everything needed to spawn the server, as the client library wants it. */
export interface ServerCommand {
  readonly command: string;
  readonly args: readonly string[];
}

export function serverCommand(binary: string): ServerCommand {
  return { command: binary, args: [SERVER_COMMAND] };
}
