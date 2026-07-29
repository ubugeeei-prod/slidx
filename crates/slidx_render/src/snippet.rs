//! Code an audience can take away.
//!
//! Two things are wrong with code on a slide at once, and they need different
//! fixes. It is too small to read from row fifteen — that is what the type
//! scale and the highlighter are for. And it cannot be *kept*: nobody
//! transcribes a screen, so the useful half of a code slide walks out of the
//! room with the speaker.
//!
//! A shared snippet fixes the second one. A fenced block marked `.share` gets a
//! page of its own in the deck's output, carrying the whole snippet rather than
//! the part that fitted on the slide, selectable and highlighted; the slide
//! shows a QR pointing at it.
//!
//! # Why the deck's own output rather than a paste host
//!
//! The offline guarantee. A deck has to work with the network cable pulled, and
//! that is not a slogan here — it is why the QR encoder was written without
//! dependencies, why the theme names no webfont, and why a remote asset is a
//! lint error. A snippet published to a paste service is a slide that depends
//! on somebody else's uptime, somebody else's retention policy, and somebody
//! else's opinion about whether the talk was worth keeping. It is also a slide
//! that tells a third party who is presenting what, to whom, and when.
//!
//! A page in the deck's own output has none of that. It is deployed by the same
//! command, versioned in the same repository, and archived by the same copy.
//!
//! # Authoring
//!
//! The attribute list is the one [`slidx_core::mark`] already defines, moved
//! from after a span of text to after a fence's language:
//!
//! ```text
//! ```rust {#retry-policy .share title="The retry policy"}
//! ```
//!
//! - `#key` names the snippet, exactly as it names a mark. It is the file name
//!   and therefore the URL, so naming one is how an author makes a link survive
//!   the slides being reordered.
//! - `.share` asks for the page and the code.
//! - `title=` heads the page. It defaults to the slide's own title.
//!
//! The grammar is reached through [`slidx_core::find_marks`] rather than parsed
//! again here: one grammar with two parsers is one grammar with two answers.

use slidx_core::{find_marks, scanner::FenceTracker, Deck, Mark, Slide, SlugAllocator};
use slidx_theme::Theme;

use crate::qr::{render_qr, SlideQrOptions};

mod page;

pub use page::{render_snippet, SnippetOptions, STYLESHEET};

/// Where the pages go, relative to the deck's own output root.
pub const SNIPPET_DIR: &str = "snippets";

/// The class a fence carries to ask for a page and a code.
const SHARE_CLASS: &str = "share";

/// One snippet the build publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// Unique within the deck. Becomes the file name and the URL.
    pub key: String,
    /// The fence's language word, as written. `None` for a bare fence.
    pub language: Option<String>,
    /// The whole block, not the part that fitted on the slide.
    pub code: String,
    pub title: Option<String>,
    /// Every slide this snippet is shown on, in reading order.
    ///
    /// Usually one. A block a speaker sets up early and returns to later is on
    /// two, and it is still one page with one link — which is the reason a
    /// snippet is addressed by key rather than by position.
    pub slides: Vec<u32>,
    /// Byte offset just past the block, one per entry in `slides`.
    after: Vec<usize>,
}

impl Snippet {
    /// Path of this snippet's page, relative to the deck's output root.
    pub fn path(&self) -> String {
        format!("{SNIPPET_DIR}/{}.html", self.key)
    }

    /// The slide this snippet was first shown on.
    ///
    /// What the page names as its source: the first appearance is where the
    /// audience met the code, and a page that cited the last one would send a
    /// reader to the recap.
    pub fn first_slide(&self) -> u32 {
        self.slides.first().copied().unwrap_or(0)
    }

    /// Absolute URL of this snippet's page, when the deck has one.
    ///
    /// A deck built for a laptop and a USB stick has no URL at all, and there
    /// is nothing to invent: a QR encoding a relative path scans to nothing, and
    /// a code that resolves to nothing is worse than no code. The slide shows
    /// the path in words instead — see [`stage`].
    pub fn url(&self, deck_url: Option<&str>) -> Option<String> {
        Some(resolve(deck_url?, &self.path()))
    }
}

/// One built page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetPage {
    pub path: String,
    pub html: String,
}

/// Every shared snippet in the deck, in reading order.
///
/// Keys are allocated across the whole deck rather than per slide, because a
/// key is a URL and two files cannot have one name.
///
/// Two blocks under one key are the interesting case. **Identical code is one
/// snippet**: a speaker who shows the same block on the setup slide and again
/// on the payoff slide wants one link, and gets one. **Different code under one
/// key** is two snippets fighting over a name, and the second gets a numeric
/// suffix rather than losing — a QR that points at the wrong code is the
/// failure this whole crate is arranged to avoid, and it is the one an author
/// cannot recover from on stage.
pub fn collect(deck: &Deck) -> Vec<Snippet> {
    let mut keys = SlugAllocator::new();
    let mut snippets: Vec<Snippet> = Vec::new();
    // The key as the author wrote it, which is not the key that was handed out
    // once a suffix has been added. Matching on the request is what lets a
    // third appearance of the *same* code find the second one's page.
    let mut requests: Vec<String> = Vec::new();

    for slide in &deck.slides {
        for (ordinal, block) in shared_blocks(slide).into_iter().enumerate() {
            let requested =
                block.mark.key.clone().unwrap_or_else(|| format!("{}-{}", slide.id, ordinal + 1));

            let published = snippets
                .iter_mut()
                .zip(&requests)
                .find(|(snippet, request)| **request == requested && snippet.code == block.code);

            // Same key, same code: one page, shown again. The later slide still
            // gets a code, pointing at the page that already exists.
            if let Some((snippet, _)) = published {
                snippet.slides.push(slide.index);
                snippet.after.push(block.after);
                continue;
            }

            snippets.push(Snippet {
                key: keys.allocate(&requested),
                language: block.language,
                code: block.code,
                title: block.mark.properties.get("title").cloned().or_else(|| slide.title.clone()),
                slides: vec![slide.index],
                after: vec![block.after],
            });
            requests.push(requested);
        }
    }

    snippets
}

/// Every page a build should write.
pub fn render_snippets(deck: &Deck, options: &SnippetOptions) -> Vec<SnippetPage> {
    collect(deck)
        .into_iter()
        .map(|snippet| SnippetPage {
            path: snippet.path(),
            html: render_snippet(deck, &snippet, options),
        })
        .collect()
}

/// The slide's Markdown with a code beside every shared block.
///
/// Planted as raw HTML into the source rather than spliced into the rendered
/// output, which is how [`slidx_core::markers`] plants step anchors. Position
/// in the source is unambiguous; position among the `<pre>` elements of a
/// document is a count that any raw HTML on the slide would throw off.
pub fn stage(deck: &Deck, slide: &Slide, theme: &Theme) -> String {
    let snippets = collect(deck);

    let mut here: Vec<(usize, &Snippet)> = snippets
        .iter()
        .flat_map(|snippet| {
            snippet
                .slides
                .iter()
                .zip(&snippet.after)
                .filter(|(index, _)| **index == slide.index)
                .map(move |(_, after)| (*after, snippet))
        })
        .collect();

    if here.is_empty() {
        return slide.content.clone();
    }

    // Back to front, so an insertion never moves an offset still to be used.
    here.sort_unstable_by_key(|(after, _)| *after);

    let mut staged = slide.content.clone();
    for (after, snippet) in here.iter().rev() {
        staged.insert_str(*after, &figure(snippet, deck, theme));
    }

    staged
}

/// The tile that goes under a shared block.
fn figure(snippet: &Snippet, deck: &Deck, theme: &Theme) -> String {
    let path = snippet.path();
    let options = SlideQrOptions { caption: Some(path.clone()), ..SlideQrOptions::default() };

    let tile = snippet
        .url(deck.meta.talk.url.as_deref())
        .and_then(|url| render_qr(&url, theme, &options))
        // No deck URL, or a payload too long to encode: the path in words is
        // still something an audience can type and a reader can follow.
        .unwrap_or_else(|| {
            format!(
                "<figure class=\"slidx-qr\">\n  <figcaption class=\"slidx-qr-caption\">{path}</figcaption>\n</figure>"
            )
        });

    format!("\n\n{tile}\n")
}

/// A fenced block that asked to be shared.
struct SharedBlock {
    mark: Mark,
    language: Option<String>,
    code: String,
    after: usize,
}

fn shared_blocks(slide: &Slide) -> Vec<SharedBlock> {
    let mut tracker = FenceTracker::new();
    let mut blocks = Vec::new();
    let mut open: Option<(&str, usize)> = None;
    let mut at = 0usize;

    for line in slide.content.split_inclusive('\n') {
        let was_inside = tracker.is_inside();
        tracker.feed(line.trim_end_matches('\n'));
        let next = at + line.len();

        match (was_inside, tracker.is_inside()) {
            (false, true) => open = Some((line, next)),
            (true, false) => {
                if let Some((fence, body)) = open.take() {
                    if let Some(block) = shared(fence, &slide.content[body..at], next) {
                        blocks.push(block);
                    }
                }
            }
            _ => {}
        }

        at = next;
    }

    // An unclosed fence is not a block. Its "contents" are the rest of the
    // slide, and publishing those as a snippet would publish the slide.
    blocks
}

fn shared(fence: &str, code: &str, after: usize) -> Option<SharedBlock> {
    let info = fence.trim_start().trim_start_matches(['`', '~']).trim();
    let mark = attributes(info)?;

    if !mark.classes.iter().any(|class| class == SHARE_CLASS) {
        return None;
    }

    let language = info.split([' ', '\t', '{']).next().filter(|word| !word.is_empty());

    Some(SharedBlock {
        mark,
        language: language.map(str::to_string),
        code: code.to_string(),
        after,
    })
}

/// Reads a fence's `{…}` with the grammar an inline mark already uses.
///
/// Reached through [`find_marks`] on a synthesised empty mark rather than
/// reimplemented: quoting, escaping, and the bare-word shorthand for a class
/// all have exactly one implementation, and it is the one the editor writes
/// against.
fn attributes(info: &str) -> Option<Mark> {
    let braced = info.find('{')?;
    let found = find_marks(&format!("[]{}", &info[braced..]));

    found.into_iter().next().map(|found| found.mark)
}

/// Resolves a path against the deck's own URL.
///
/// The ordinary relative-URL rule: a base naming a file is replaced from its
/// last segment, a base naming a directory is appended to. Authors write both —
/// `https://example.com/talk/` and `https://example.com/talk/index.html` are
/// the same deck — and only one of them works if this guesses.
fn resolve(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');

    // The host is not a path segment, so `https://example.com` is a directory
    // even though `example.com` looks like a file name.
    let host_at = trimmed.find("://").map_or(0, |at| at + 3);

    let directory = match trimmed[host_at..].rsplit_once('/') {
        Some((head, last)) if last.contains('.') && !base.ends_with('/') => {
            &trimmed[..host_at + head.len()]
        }
        _ => trimmed,
    };

    format!("{directory}/{path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    const URL: &str = "---\nurl: https://example.com/talk/\n---\n\n";

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn shared(code: &str) -> String {
        format!("# One\n\n```rust {{#retry .share}}\n{code}\n```\n")
    }

    #[test]
    fn a_fence_marked_share_becomes_a_snippet_with_its_whole_block() {
        let deck = deck(&shared("async fn retry() {\n    loop {}\n}"));
        let snippets = collect(&deck);

        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].key, "retry");
        assert_eq!(snippets[0].language.as_deref(), Some("rust"));
        assert!(snippets[0].code.contains("loop {}"), "{:?}", snippets[0].code);
    }

    #[test]
    fn an_ordinary_fence_is_not_published() {
        // Most code on most slides is an illustration, not something anyone
        // wants a link to. Sharing is asked for, never assumed.
        assert!(collect(&deck("```rust\nlet x = 1;\n```\n")).is_empty());
        assert!(collect(&deck("```rust {#named}\nlet x = 1;\n```\n")).is_empty());
    }

    #[test]
    fn the_attribute_list_is_the_one_a_mark_already_uses() {
        // Including the bare-word shorthand for a class, because that is what
        // `slidx_core` does with `[x]{accent}` and one grammar cannot have two
        // sets of rules depending on where it is written.
        let deck = deck("```ts {#api share title=\"The client\"}\nexport const a = 1;\n```\n");
        let snippets = collect(&deck);

        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].title.as_deref(), Some("The client"));
    }

    #[test]
    fn a_snippet_with_no_title_takes_the_slides() {
        let deck = deck("# Retrying\n\n```rust {#r .share}\nfn f() {}\n```\n");
        assert_eq!(collect(&deck)[0].title.as_deref(), Some("Retrying"));
    }

    #[test]
    fn a_snippet_with_no_key_is_named_after_its_slide_and_its_position() {
        // Naming it is how an author makes the URL survive a reorder; not
        // naming it should still produce a working link rather than an error.
        let deck = deck("# Setup\n\n```sh {.share}\nnpm i\n```\n\n```sh {.share}\nnpm test\n```\n");
        let snippets = collect(&deck);
        let keys: Vec<&str> = snippets.iter().map(|snippet| snippet.key.as_str()).collect();

        assert_eq!(keys, vec!["setup-1", "setup-2"]);
    }

    #[test]
    fn the_same_code_under_one_key_on_two_slides_is_one_page() {
        // The case a key exists for: a snippet the speaker sets up early and
        // returns to later is one thing, and deserves one link.
        let block = "```rust {#retry .share}\nfn retry() {}\n```\n";
        let deck = deck(&format!("# One\n\n{block}\n---\n\n# Two\n\n{block}"));

        let snippets = collect(&deck);
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].slides, vec![0, 1]);
        assert_eq!(snippets[0].first_slide(), 0, "the page cites where the audience met it");
    }

    #[test]
    fn a_snippet_shown_twice_carries_a_code_on_both_slides() {
        // One page, two placements. A second slide with no code would leave the
        // audience looking at a block they were told they could take away.
        let block = "```rust {#retry .share}\nfn retry() {}\n```\n";
        let deck = deck(&format!("{URL}# One\n\n{block}\n---\n\n# Two\n\n{block}"));
        let theme = slidx_theme::default_theme();

        for slide in &deck.slides {
            assert!(
                stage(&deck, slide, &theme).contains("snippets/retry.html"),
                "slide {} has no code",
                slide.index + 1
            );
        }
    }

    #[test]
    fn different_code_under_one_key_gets_two_pages_rather_than_losing_one() {
        // A QR pointing at the wrong code is the failure this crate is arranged
        // to avoid, and it is the one an author cannot recover from on stage.
        let deck = deck(concat!(
            "# One\n\n```rust {#retry .share}\nfn before() {}\n```\n\n",
            "---\n\n# Two\n\n```rust {#retry .share}\nfn after() {}\n```\n",
        ));

        let snippets = collect(&deck);
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0].key, "retry");
        assert_eq!(snippets[1].key, "retry-2");
        assert_ne!(snippets[0].path(), snippets[1].path());
    }

    #[test]
    fn a_page_lands_in_the_decks_own_output() {
        // Not a paste host. A deck has to work with the network cable pulled,
        // and a snippet on somebody else's service is a slide that fails in the
        // room where it matters.
        let deck = deck(&shared("fn f() {}"));
        let pages = render_snippets(&deck, &SnippetOptions::default());

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].path, "snippets/retry.html");
        assert!(pages[0].html.starts_with("<!doctype html>"));
    }

    #[test]
    fn the_url_is_resolved_against_the_deck_however_the_author_wrote_it() {
        // `https://example.com/talk/` and `https://example.com/talk/index.html`
        // are the same deck, and both are written.
        let snippet = &collect(&deck(&shared("fn f() {}")))[0];

        for base in ["https://example.com/talk", "https://example.com/talk/"] {
            assert_eq!(
                snippet.url(Some(base)).as_deref(),
                Some("https://example.com/talk/snippets/retry.html"),
                "from {base}"
            );
        }

        assert_eq!(
            snippet.url(Some("https://example.com/talk/index.html")).as_deref(),
            Some("https://example.com/talk/snippets/retry.html")
        );
        assert_eq!(
            snippet.url(Some("https://example.com")).as_deref(),
            Some("https://example.com/snippets/retry.html")
        );
    }

    #[test]
    fn a_deck_with_a_url_shows_a_code_on_the_slide() {
        let deck = deck(&format!("{URL}{}", shared("fn retry() {}")));
        let html = crate::render_slide(&deck, &deck.slides[0], &crate::ShellOptions::default());

        assert!(html.contains("<svg"), "no code was drawn");
        assert!(html.contains("snippets/retry.html"), "the path is not readable");
    }

    #[test]
    fn a_deck_with_no_url_shows_the_path_rather_than_a_code_that_scans_to_nothing() {
        // There is nothing to invent. A QR carrying a relative path resolves to
        // nothing on a phone, and a code that resolves to nothing is worse than
        // no code — the same reason `render_qr` refuses rather than draws.
        let deck = deck(&shared("fn retry() {}"));
        let html = crate::render_slide(&deck, &deck.slides[0], &crate::ShellOptions::default());

        assert!(!html.contains("<svg"));
        assert!(html.contains("snippets/retry.html"));
    }

    #[test]
    fn the_code_lands_after_its_own_block_and_not_after_another_one() {
        let deck = deck(&format!(
            "{URL}# Two blocks\n\n```rust\nfn plain() {{}}\n```\n\n\
             ```rust {{#shared .share}}\nfn shared() {{}}\n```\n"
        ));

        let staged = stage(&deck, &deck.slides[0], &slidx_theme::default_theme());
        let at_figure = staged.find("slidx-qr").expect("a tile was planted");

        assert!(staged[..at_figure].contains("fn shared()"), "planted before its block");
        assert!(!staged[at_figure..].contains("fn shared()"), "planted after the wrong block");
    }

    #[test]
    fn a_slide_with_nothing_shared_is_left_exactly_as_it_was() {
        let deck = deck("# One\n\n```rust\nlet x = 1;\n```\n");
        let slide = &deck.slides[0];

        assert_eq!(stage(&deck, slide, &slidx_theme::default_theme()), slide.content);
    }

    #[test]
    fn an_unclosed_fence_publishes_nothing() {
        // Its "contents" are the rest of the slide, and publishing those would
        // publish the slide.
        assert!(collect(&deck("```rust {#x .share}\nfn f() {}\n")).is_empty());
    }

    #[test]
    fn a_shared_block_in_a_language_nobody_scans_still_gets_a_page() {
        // Sharing and highlighting are separate promises. The audience can take
        // the code away whether or not slidx knows how to colour it.
        let deck = deck("```elixir {#mod .share}\ndefmodule A do\nend\n```\n");
        let pages = render_snippets(&deck, &SnippetOptions::default());

        assert_eq!(pages.len(), 1);
        assert!(pages[0].html.contains("defmodule A do"));
        assert!(!pages[0].html.contains("<span class=\"slidx-code-"), "a grammar was guessed at");
    }

    #[test]
    fn the_printed_deck_carries_the_codes_too() {
        // A handout is the other artefact an audience takes home, and a page of
        // code with no link is the same problem in a different medium.
        let deck = deck(&format!("{URL}{}", shared("fn retry() {}")));
        let html = crate::render_print(&deck, &crate::PrintOptions::default());

        assert!(html.contains("snippets/retry.html"));
    }
}
