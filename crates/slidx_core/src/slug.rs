//! Stable, human-readable slide identifiers.
//!
//! Slide ids end up in URLs, in PDF bookmarks, and in the links a speaker pastes
//! into chat mid-talk. They must stay readable for non-Latin decks, so letters
//! outside ASCII are preserved rather than transliterated or dropped — a deck
//! written in Japanese deserves Japanese anchors, not `slide-7`.

/// Converts a heading into a URL-safe identifier.
///
/// Returns an empty string when the heading has no usable characters; callers
/// decide the fallback, because only they know the slide's position.
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());

    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if character.is_alphanumeric() {
            // Non-ASCII letters and digits: keep them, case-folded by the
            // Unicode rules rather than the ASCII ones. Percent-encoding makes
            // these valid in a URL, and every target browser displays them
            // decoded in the address bar.
            slug.extend(character.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }

    slug.trim_matches('-').to_string()
}

/// Hands out ids that are unique within one deck.
///
/// Collisions are common and benign — two slides titled "Demo" is normal — so
/// duplicates get a numeric suffix instead of an error.
#[derive(Debug, Default)]
pub struct SlugAllocator {
    taken: std::collections::HashMap<String, u32>,
}

impl SlugAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `base`, or `base-2`, `base-3`, … if it is already taken.
    pub fn allocate(&mut self, base: &str) -> String {
        let count = self.taken.entry(base.to_string()).or_insert(0);
        *count += 1;

        if *count == 1 {
            return base.to_string();
        }

        // The suffixed form could itself collide with a literal heading, so keep
        // stepping until the result is genuinely free.
        let mut candidate = format!("{base}-{count}");
        while self.taken.contains_key(&candidate) {
            *self.taken.get_mut(base).expect("inserted above") += 1;
            let count = self.taken[base];
            candidate = format!("{base}-{count}");
        }

        self.taken.insert(candidate.clone(), 1);
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_headings_become_lowercase_hyphenated() {
        assert_eq!(slugify("Getting Started"), "getting-started");
        assert_eq!(slugify("Why Rust?"), "why-rust");
        assert_eq!(slugify("  Padded  "), "padded");
    }

    #[test]
    fn runs_of_punctuation_collapse_to_one_hyphen() {
        assert_eq!(slugify("A -- B"), "a-b");
        assert_eq!(slugify("a/b/c"), "a-b-c");
    }

    #[test]
    fn non_ascii_letters_survive() {
        assert_eq!(slugify("はじめに"), "はじめに");
        assert_eq!(slugify("Vue と React"), "vue-と-react");
        assert_eq!(slugify("Ünïcödé"), "ünïcödé");
    }

    #[test]
    fn a_heading_with_no_letters_produces_an_empty_slug() {
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn emoji_are_dropped_but_neighbouring_words_survive() {
        assert_eq!(slugify("🚀 Launch"), "launch");
    }

    #[test]
    fn the_allocator_suffixes_duplicates() {
        let mut allocator = SlugAllocator::new();
        assert_eq!(allocator.allocate("demo"), "demo");
        assert_eq!(allocator.allocate("demo"), "demo-2");
        assert_eq!(allocator.allocate("demo"), "demo-3");
        assert_eq!(allocator.allocate("other"), "other");
    }

    #[test]
    fn a_suffix_that_collides_with_a_real_heading_steps_past_it() {
        let mut allocator = SlugAllocator::new();
        assert_eq!(allocator.allocate("demo"), "demo");
        assert_eq!(allocator.allocate("demo-2"), "demo-2");
        assert_eq!(allocator.allocate("demo"), "demo-3", "must not reuse the literal demo-2");
    }
}
