//! Cloudflare Pages, as a file on disk and a command the author runs.
//!
//! A built deck is a directory of HTML. Cloudflare Pages will host that
//! directory; slidx will not. What this target writes is the `wrangler.toml`
//! Pages reads, and what it prints is `wrangler pages deploy` — the author is
//! logged into *their* Cloudflare account, and slidx still has no HTTP client
//! and no token store.
//!
//! The file is the whole payload. A plan that only named the command would
//! leave the author to invent a project name, a compatibility date, and a
//! build-output path, which is the chore this destination exists to remove.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::fields::{resolve_slug, SlugField};
use crate::types::{Composed, DeckSource};

/// Pages project names. Alphanumeric and dashes; 58 is the documented cap.
const NAME_LIMIT: usize = 58;

/// Where the file is written, relative to wherever the caller is writing.
pub const PATH: &str = "wrangler.toml";

/// What the author runs after slidx writes the file. Never executed here.
pub const COMMAND: &str = "wrangler pages deploy";

/// The directory a slidx build already writes. Named rather than discovered:
/// this crate has no filesystem, and guessing would make a plan depend on
/// whether somebody had built yet.
const DIST: &str = "./dist";

/// The day this target landed.
///
/// Not "today". A plan has no clock, and two runs of the same deck have to
/// produce the same file — a date that moved between them would be a diff
/// that says nothing about the deck.
const COMPATIBILITY_DATE: &str = "2026-08-25";

/// What slidx writes, and the command the author still has to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudflarePages {
    /// Pages project name. Alphanumeric and dashes only.
    pub name: String,
    /// Always [`PATH`].
    pub path: String,
    /// The file, comments included.
    pub toml: String,
    /// What the author runs after slidx writes the file. Never executed here.
    pub command: String,
}

pub fn compose_cloudflare(source: &DeckSource) -> Composed<CloudflarePages> {
    let mut reasons = Vec::new();
    let name = resolve_slug(
        &source.meta,
        &SlugField { limit: NAME_LIMIT, minimum: 1, platform: "Cloudflare Pages" },
        &mut reasons,
    );

    if !reasons.is_empty() {
        return Composed::Blocked(reasons);
    }

    Composed::Ready(CloudflarePages {
        toml: render(&name),
        name,
        path: PATH.to_string(),
        command: COMMAND.to_string(),
    })
}

/// One line for a printed plan.
pub fn describe_cloudflare(pages: &CloudflarePages) -> String {
    format!("write {}; then {}", pages.path, pages.command)
}

fn render(name: &str) -> String {
    format!(
        "\
# Written by `slidx publish`. Deploy with:
#
#     {COMMAND}
#
# slidx does not log in and does not hold a token. `wrangler login` is yours.

name = \"{name}\"
pages_build_output_dir = \"{DIST}\"
compatibility_date = \"{COMPATIBILITY_DATE}\"
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DeckMetadata;

    fn deck(title: Option<&str>, slug: Option<&str>) -> DeckSource {
        DeckSource {
            meta: DeckMetadata {
                title: title.map(str::to_string),
                slug: slug.map(str::to_string),
                ..DeckMetadata::default()
            },
            ..DeckSource::default()
        }
    }

    fn ready(source: &DeckSource) -> CloudflarePages {
        compose_cloudflare(source).value().cloned().expect("a wrangler.toml")
    }

    #[test]
    fn a_latin_title_becomes_the_pages_project_name() {
        let pages = ready(&deck(Some("Zero-JavaScript Slides"), None));

        assert_eq!(pages.name, "zero-javascript-slides");
        assert_eq!(pages.path, "wrangler.toml");
        assert_eq!(pages.command, "wrangler pages deploy");
        assert!(pages.toml.contains("name = \"zero-javascript-slides\""));
        assert!(pages.toml.contains("pages_build_output_dir = \"./dist\""));
        assert!(pages.toml.contains("compatibility_date = \"2026-08-25\""));
        assert!(pages.toml.contains("wrangler pages deploy"));
    }

    #[test]
    fn an_authored_slug_is_the_project_name_rather_than_the_title() {
        // A URL people have already written down. Deriving from the title
        // after the author pinned a slug would move the Pages project under
        // them the next time they retitled the talk.
        let pages = ready(&deck(Some("Zero-JavaScript Slides"), Some("plain-html")));

        assert_eq!(pages.name, "plain-html");
    }

    #[test]
    fn a_title_with_no_latin_characters_is_blocked_rather_than_invented() {
        let composed = compose_cloudflare(&deck(Some("日本語のスライド"), None));

        assert!(composed.value().is_none());
        let reasons = match composed {
            Composed::Blocked(reasons) => reasons,
            Composed::Ready(_) => panic!("expected blocked"),
        };
        assert_eq!(reasons[0].field, "slug");
        assert!(reasons[0].message.contains("`slug:`"), "{}", reasons[0].message);
    }

    #[test]
    fn the_file_names_no_token_and_asks_for_no_login_from_slidx() {
        // The whole point of the destination: slidx writes a file, wrangler
        // is the author's, and a secret in this toml would be a secret in git.
        // The comment may say the word; the keys must not.
        let toml = ready(&deck(Some("A talk"), None)).toml;
        let keys: String =
            toml.lines().filter(|line| !line.trim_start().starts_with('#')).collect();

        assert!(!keys.to_ascii_uppercase().contains("TOKEN"));
        assert!(!keys.contains("CLOUDFLARE_API"));
        assert!(toml.contains("`wrangler login` is yours"));
    }

    #[test]
    fn the_same_deck_writes_the_same_file_twice() {
        // A date that moved between runs would be a diff that says nothing.
        let source = deck(Some("A talk"), None);

        assert_eq!(ready(&source).toml, ready(&source).toml);
    }

    #[test]
    fn the_summary_names_the_file_and_the_command() {
        assert_eq!(
            describe_cloudflare(&ready(&deck(Some("A talk"), None))),
            "write wrangler.toml; then wrangler pages deploy"
        );
    }

    #[test]
    fn a_title_longer_than_the_project_name_cap_is_fitted_rather_than_blocked() {
        let title = format!("{} Talk", "Very Long ".repeat(20));
        let pages = ready(&deck(Some(&title), None));

        assert!(pages.name.len() <= NAME_LIMIT, "{}", pages.name);
        assert!(pages
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn an_authored_slug_over_the_cap_is_blocked_rather_than_cut() {
        // A URL people have already written down. Cutting it would publish a
        // different project from the one the author named.
        let composed = compose_cloudflare(&deck(None, Some(&"a".repeat(NAME_LIMIT + 1))));

        assert!(composed.value().is_none());
        let reasons = match composed {
            Composed::Blocked(reasons) => reasons,
            Composed::Ready(_) => panic!("expected blocked"),
        };
        assert_eq!(reasons[0].field, "slug");
    }
}
