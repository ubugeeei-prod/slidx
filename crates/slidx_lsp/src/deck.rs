//! Which files this server is for.
//!
//! A deck is Markdown, and most Markdown is not a deck. An editor that put
//! slidx diagnostics on every `README.md` — or offered `autoSteps:` while
//! somebody wrote a changelog — would be a plugin people uninstall, and the
//! author would be right.
//!
//! # The rule
//!
//! A Markdown file whose directory is [`DECK_DIRECTORY`]. That is the plugin's
//! default `srcDir` and the path `slidx lint`, `slidx fmt` and `slidx dev` all
//! fall back to, so it is the one layout every other part of slidx already
//! assumes. Its subdirectories are not decks: `slides/images/notes.md` is an
//! asset, which is the same answer [`slidx_cli::project`] gives.
//!
//! Nothing else qualifies, and two near-misses are worth naming.
//!
//! A single-file deck — `talk.md` at the top of a project — is **not** picked
//! up. Nothing in its path distinguishes it from a README, and the only way to
//! tell would be to open every Markdown file in a workspace and read it, which
//! is the behaviour this module exists to refuse. Moving it under `slides/` is
//! what makes it a deck to slidx, and to the plugin that builds it.
//!
//! A project that configured the plugin's `srcDir` to something else is not
//! picked up either. The server is told a URI and nothing about the Vite
//! config it belongs to, and guessing from a directory name would be the same
//! overreach one level up.
//!
//! # Why here and not in each client
//!
//! Because one of the three cannot express it. A VS Code client scopes to a
//! glob and a Neovim one to a file-name pattern, so both could filter on their
//! own — but a Zed extension binds a language server to a whole *language*, and
//! has nowhere to put a path rule at all. Answering it here is the only place
//! all three editors get the same answer, and it is the only place a test can
//! state it.

/// The directory whose Markdown is a deck.
///
/// The plugin's default `srcDir`, spelled the same way `slidx lint` spells the
/// path it falls back to — pinned to it by a test in [`slidx_cli`], which is
/// the crate that owns that default.
pub const DECK_DIRECTORY: &str = "slides";

/// The glob a client filters on, for the editors that can.
///
/// Exactly [`is_deck`] in the notation a `documentSelector` and an `autocmd`
/// both take. Restated for those clients because neither can call Rust, and
/// held to this one by a test that reads them.
pub const DECK_GLOB: &str = "**/slides/*.md";

/// True for a URI this server should analyse.
///
/// Takes the URI rather than a path because that is what the protocol carries,
/// and the conversion to a path is the client's business and platform's — a
/// Windows document arrives as `file:///c%3A/talks/slides/0001.md`, and only
/// the last two segments decide anything.
pub fn is_deck(uri: &str) -> bool {
    let path = path_of(uri);
    let mut segments = path.rsplit('/');

    let Some(file) = segments.next() else { return false };
    // Case-folded like `slidx lint` folds it, because `.MD` off a Windows or
    // macOS filesystem is the same file.
    if !file.to_lowercase().ends_with(".md") {
        return false;
    }

    segments.next() == Some(DECK_DIRECTORY)
}

/// The path part of a URI, percent-decoded.
///
/// Query and fragment go first: a client is entitled to append either, and
/// `slides/0001.md?version=3` is still slide one.
fn path_of(uri: &str) -> String {
    let without_scheme = uri.split_once("://").map_or(uri, |(_, rest)| rest);
    let path = without_scheme.split(['?', '#']).next().unwrap_or_default();

    decode(path)
}

/// Percent-decoding, over bytes rather than characters.
///
/// A multi-byte character arrives as one escape per byte, so decoding has to
/// reassemble the bytes and read them as UTF-8 afterwards. Anything that is not
/// valid UTF-8 once decoded is left as it was: this decides which files are
/// decks, and a URI nobody can read is not one.
fn decode(path: &str) -> String {
    if !path.contains('%') {
        return path.to_string();
    }

    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;

    while at < bytes.len() {
        let escape = (bytes[at] == b'%' && at + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[at + 1..at + 3]).ok())
            .flatten()
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());

        match escape {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_in_a_slides_directory_is_a_deck() {
        assert!(is_deck("file:///Users/somebody/talks/vueconf/slides/0001.md"));
        assert!(is_deck("file:///talks/slides/opening.md"));
    }

    #[test]
    fn every_other_markdown_file_in_a_workspace_is_left_alone() {
        // The whole reason this module exists. Diagnostics on somebody's
        // README are how an editor plugin gets uninstalled.
        assert!(!is_deck("file:///Users/somebody/talks/vueconf/README.md"));
        assert!(!is_deck("file:///Users/somebody/notes/2026-07-30.md"));
        assert!(!is_deck("file:///Users/somebody/blog/slides-that-work.md"));
    }

    #[test]
    fn a_single_file_deck_at_the_top_of_a_project_is_not_claimed() {
        // Nothing in the path tells it from a README, and the only way to know
        // would be to read every Markdown file the workspace has.
        assert!(!is_deck("file:///talks/vueconf/talk.md"));
    }

    #[test]
    fn a_subdirectory_of_slides_holds_assets_rather_than_more_slides() {
        // The same answer `slidx_cli::project` gives when it walks a project.
        assert!(!is_deck("file:///talks/slides/images/credits.md"));
    }

    #[test]
    fn a_directory_merely_ending_in_slides_is_not_the_slides_directory() {
        assert!(!is_deck("file:///talks/old-slides/0001.md"));
        assert!(!is_deck("file:///talks/slidesx/0001.md"));
    }

    #[test]
    fn a_file_that_is_not_markdown_is_not_a_slide_even_in_the_right_place() {
        assert!(!is_deck("file:///talks/slides/notes.txt"));
        assert!(!is_deck("file:///talks/slides/theme.css"));
    }

    #[test]
    fn an_uppercase_extension_off_a_case_insensitive_filesystem_still_counts() {
        // `slidx lint` folds the extension the same way, and macOS and Windows
        // both hand back whichever spelling was typed.
        assert!(is_deck("file:///talks/slides/0001.MD"));
    }

    #[test]
    fn a_windows_uri_is_read_the_same_way() {
        assert!(is_deck("file:///c%3A/talks/vueconf/slides/0001.md"));
        assert!(!is_deck("file:///c%3A/talks/vueconf/README.md"));
    }

    #[test]
    fn a_percent_encoded_japanese_path_decodes_before_it_is_judged() {
        // `/デッキ/slides/導入.md`. Encoded byte by byte, so decoding has to
        // reassemble them before anything is read as a directory name.
        let uri = "file:///%E3%83%87%E3%83%83%E3%82%AD/slides/%E5%B0%8E%E5%85%A5.md";

        assert!(is_deck(uri));
    }

    #[test]
    fn a_directory_name_hidden_behind_an_escape_is_still_read_as_what_it_spells() {
        // `%73` is `s`. Nothing writes a URI this way, and a rule that could be
        // stepped around by writing one would not be a rule.
        assert!(is_deck("file:///talks/%73lides/0001.md"));
    }

    #[test]
    fn a_query_or_fragment_does_not_change_which_file_a_uri_names() {
        assert!(is_deck("file:///talks/slides/0001.md?version=3"));
        assert!(is_deck("file:///talks/slides/0001.md#slide-1"));
    }

    #[test]
    fn a_buffer_that_was_never_saved_has_no_directory_to_judge() {
        // `untitled:Untitled-1` in VS Code, and `file:///` with nothing after
        // it elsewhere. Neither is on disk, so neither is a deck yet.
        assert!(!is_deck("untitled:Untitled-1"));
        assert!(!is_deck("file:///"));
        assert!(!is_deck(""));
    }

    #[test]
    fn a_malformed_escape_is_left_alone_rather_than_panicking() {
        // A client is not obliged to send a URI anybody can read, and a server
        // that crashed on one would take the editor's session with it.
        assert!(!is_deck("file:///talks/%zz/0001.md"));
        assert!(!is_deck("file:///talks/slides/%"));
    }

    #[test]
    fn the_glob_the_clients_filter_on_says_the_same_thing() {
        // Three editors restate this rule because none of them can call Rust.
        // The glob is the wording they all take; a test on each side names it.
        assert_eq!(DECK_GLOB, format!("**/{DECK_DIRECTORY}/*.md"));
    }
}
