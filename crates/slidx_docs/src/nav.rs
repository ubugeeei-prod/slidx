//! The sections, which are the readers.
//!
//! A documentation site is usually organised the way the code is — a page per
//! crate, a page per package — and that is a map of the repository rather than
//! of anybody's question. Nobody arrives wanting to know what `slidx_render`
//! does.
//!
//! People arrive in one of four states, and each of those is a section here:
//!
//! - They have never heard of slidx, and want to see it rather than read about
//!   it. → [`Section::Start`], one page that ends with a built deck.
//! - They have a talk in a few weeks and are deciding. → [`Section::Choosing`],
//!   which has to be honest about what is not there.
//! - They are speaking tomorrow and something is wrong. → [`Section::Tonight`],
//!   indexed by symptom, because at that hour nobody wants a concept explained.
//! - They know what they want and need the exact spelling. →
//!   [`Section::Reference`].
//!
//! A page declares its section in frontmatter, and a section with no pages does
//! not appear at all — so the navigation can never advertise a door that opens
//! onto nothing.

/// One of the four states a reader arrives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Start,
    Choosing,
    Tonight,
    Reference,
}

impl Section {
    /// Every section, in the order the navigation lists them.
    ///
    /// Ordered by how soon a reader needs it, not by how much of the site it
    /// holds: reference is the largest section and the last one anybody wants.
    pub const ALL: [Self; 4] = [Self::Start, Self::Choosing, Self::Tonight, Self::Reference];

    /// The spelling a page's frontmatter uses.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Choosing => "choosing",
            Self::Tonight => "tonight",
            Self::Reference => "reference",
        }
    }

    /// What the navigation calls it.
    ///
    /// A phrase rather than a noun, because the label is doing the routing: a
    /// reader picks the door that describes their situation, and "Guides" does
    /// not describe anybody's situation.
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Choosing => "Choosing slidx",
            Self::Tonight => "The night before",
            Self::Reference => "Reference",
        }
    }

    /// Resolves the frontmatter spelling.
    ///
    /// Unknown is an error rather than a default, and that is the whole reason
    /// this is an enum: a page whose `section:` was mistyped would otherwise
    /// build, publish, and appear in no navigation anywhere.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.as_token() == token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_round_trips_through_its_token() {
        for section in Section::ALL {
            assert_eq!(Section::parse(section.as_token()), Some(section));
        }
    }

    #[test]
    fn an_unknown_section_does_not_resolve() {
        // The failure this prevents is silent: a mistyped `section:` that fell
        // back to a default would put the page in the wrong door and nothing
        // would say so.
        assert_eq!(Section::parse("guides"), None);
        assert_eq!(Section::parse(""), None);
    }

    #[test]
    fn the_sections_are_ordered_by_how_soon_a_reader_needs_them() {
        assert_eq!(Section::ALL[0], Section::Start);
        assert_eq!(*Section::ALL.last().expect("four sections"), Section::Reference);
    }

    #[test]
    fn no_two_sections_share_a_token_or_a_label() {
        for (index, section) in Section::ALL.into_iter().enumerate() {
            for other in Section::ALL.into_iter().skip(index + 1) {
                assert_ne!(section.as_token(), other.as_token());
                assert_ne!(section.label(), other.label());
            }
        }
    }
}
