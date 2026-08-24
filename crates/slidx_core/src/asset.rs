//! What a slide's reference to a file is called.
//!
//! Two crates ask this and used to answer it differently, which is how the
//! image rules came to be silent in `vite build` and not from `slidx lint`.
//!
//! `slidx_lint` looks a reference up in a map of sizes somebody else measured.
//! `slidx_render` looks the same reference up in the same map, to write an
//! image's own dimensions onto the tag that draws it. Both need the key the
//! measurer used, and the measurer — `readAssetSizes` in the Vite plugin —
//! keys on **the path relative to the deck's directory**.
//!
//! So `slides/chart.png` is `chart.png`, and a slide almost always writes
//! `./chart.png`. One normalisation stripped a leading `/` and not a leading
//! `.`, the other stripped both, and the two disagreed about the commonest
//! reference there is. See #307.
//!
//! It lives here because this is the crate they already share, and because the
//! question is about the deck model rather than about linting or rendering:
//! *what did the author name*.

/// The key a reference is measured under, or nothing when it names no file.
///
/// Absent for a reference that cannot be a path under the deck — a remote URL,
/// a protocol-relative one, a `data:` URI. Those are somebody else's rule: the
/// offline guarantee reports the first two, and the third carries its own bytes.
///
/// A query and a fragment are dropped. Vite carries build instructions in a
/// query and a fragment selects part of an SVG; neither is part of a name.
///
/// `./` and a leading `/` are both dropped, because both resolve against the
/// deck's own directory — which is what the map is keyed on.
pub fn key(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.starts_with("//") || scheme(url).is_some() {
        return None;
    }

    let path = url.split(['?', '#']).next()?;

    Some(path.trim_start_matches("./").trim_start_matches('/').to_string())
}

/// The URL scheme, or `None` for a relative reference.
pub fn scheme(url: &str) -> Option<&str> {
    let (scheme, _) = url.split_once(':')?;

    // A scheme starts with a letter and continues with letters, digits, `+`,
    // `-`, or `.`. Anything else means the colon belongs to a path segment.
    let valid = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));

    valid.then_some(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordinary_way_to_write_a_relative_path_keys_the_way_it_is_measured() {
        // The bug this module exists for. A measurer keys `slides/chart.png` as
        // `chart.png`, a slide writes `./chart.png`, and one of the two
        // normalisations stripped a `/` and not a `.`.
        assert_eq!(key("./chart.png").as_deref(), Some("chart.png"));
        assert_eq!(key("chart.png").as_deref(), Some("chart.png"));
        assert_eq!(key("/chart.png").as_deref(), Some("chart.png"));
    }

    #[test]
    fn a_bundler_instruction_is_not_part_of_a_name() {
        assert_eq!(key("./chart.png?width=800").as_deref(), Some("chart.png"));
        assert_eq!(key("./icons.svg#chart").as_deref(), Some("icons.svg"));
    }

    #[test]
    fn a_reference_that_names_no_file_under_the_deck_has_no_key() {
        // Each of these belongs to a different rule, and answering here would
        // be this module having an opinion about somebody else's.
        for url in
            ["https://example.com/a.png", "//example.com/a.png", "data:image/png;base64,x", ""]
        {
            assert_eq!(key(url), None, "{url} was keyed");
        }
    }

    #[test]
    fn a_colon_inside_a_path_is_not_a_scheme() {
        assert_eq!(scheme("./a:b.png"), None);
        assert_eq!(scheme("https://example.com"), Some("https"));
    }
}
