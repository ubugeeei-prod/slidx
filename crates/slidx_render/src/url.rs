//! Where a deck's own pages live, relative to its root.
//!
//! A built deck is a set of addresses: slide one at the root, the rest one
//! directory down, a snippet page under `snippets/`, a social card beside them.
//! Four separate things have to agree about that layout — a QR code drawn on a
//! slide, a canonical link, a prev/next pair, and a sitemap — and a second
//! spelling of the rule is a link that resolves to nothing in whichever of them
//! nobody remembered to change.

/// Where slide `index` lives, relative to the deck root.
///
/// Slide one *is* the deck root: it is the page a bare deck URL opens, so
/// giving it a directory of its own would leave the root either empty or
/// holding a second copy of the same slide.
pub fn slide_path(index: u32) -> String {
    if index == 0 {
        String::new()
    } else {
        format!("{}/", index + 1)
    }
}

/// The prefix a page on slide `index` needs to reach the deck root.
///
/// Relative rather than absolute because it has to be right on a deck nobody
/// has told slidx the URL of — which includes every deck opened from a USB
/// stick, where an absolute path resolves against the filesystem root.
pub fn up_to_root(index: u32) -> &'static str {
    if index == 0 {
        ""
    } else {
        "../"
    }
}

/// Joins a path inside the deck onto the deck's own URL.
///
/// `url:` names the deck root, so a value with a file name on the end — someone
/// who wrote the address of slide one rather than of the deck — resolves against
/// the directory holding it rather than appending to a file name.
pub fn resolve(base: &str, path: &str) -> String {
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

    #[test]
    fn slide_one_is_the_deck_root_and_the_rest_are_one_directory_down() {
        assert_eq!(slide_path(0), "");
        assert_eq!(slide_path(1), "2/");
        assert_eq!(slide_path(9), "10/");
    }

    #[test]
    fn a_relative_link_from_a_deeper_slide_climbs_back_to_the_root_first() {
        // The pair that has to compose: from slide 3, slide 2 is `../2/`, and
        // getting that wrong points every prev link at a directory below the
        // page it is on.
        assert_eq!(format!("{}{}", up_to_root(2), slide_path(1)), "../2/");
        assert_eq!(format!("{}{}", up_to_root(0), slide_path(1)), "2/");
        assert_eq!(format!("{}{}", up_to_root(1), slide_path(0)), "../");
    }

    #[test]
    fn a_deck_url_with_or_without_a_trailing_slash_resolves_the_same_way() {
        assert_eq!(resolve("https://example.com/talk", "2/"), "https://example.com/talk/2/");
        assert_eq!(resolve("https://example.com/talk/", "2/"), "https://example.com/talk/2/");
    }

    #[test]
    fn a_deck_url_naming_a_file_resolves_against_the_directory_holding_it() {
        // An author who pasted the address of slide one rather than of the deck.
        assert_eq!(
            resolve("https://example.com/talk/index.html", "og.png"),
            "https://example.com/talk/og.png"
        );
    }

    #[test]
    fn a_bare_host_is_a_directory_rather_than_a_file_named_after_its_tld() {
        assert_eq!(resolve("https://example.com", "sitemap.xml"), "https://example.com/sitemap.xml");
    }
}
