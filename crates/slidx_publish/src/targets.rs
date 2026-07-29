//! The destinations, and the one rule they share.
//!
//! A target turns deck metadata into one platform's shape. It composes, checks
//! that shape against that platform's documented caps, and returns either a
//! payload or the named reasons there is none. **No target performs anything.**
//! None of them opens a socket, reads a file, or takes a token, and the plan is
//! the boundary where that stops being an implementation detail and becomes a
//! property: a crate that can post as you is a crate that has to be trusted
//! with a credential, and this one never asks for one.
//!
//! The shared policy on limits, stated once and applied by every module here:
//! what the author wrote is passed through or reported, what slidx derived is
//! fitted. The social post is the documented exception — a character budget is
//! the entire premise of that target, so its description is cut to fit and the
//! payload says so.

pub mod archive;
pub mod blog;
pub mod docswell;
pub mod resources;
pub mod social;
pub mod speakerdeck;

pub use archive::{compose_archive, describe_archive, is_orderable_date, ArchiveRecord};
pub use blog::{compose_blog, describe_blog, BlogScaffold, BlogSection};
pub use docswell::{compose_docswell, describe_docswell, DocswellUpload};
pub use resources::{compose_resources, describe_resources, ResourcesPage};
pub use social::{compose_social, describe_social, SocialOptions, SocialPost, DEFAULT_POST_LIMIT};
pub use speakerdeck::{compose_speaker_deck, describe_speaker_deck, SpeakerDeckUpload};

/// A YAML scalar, always quoted.
///
/// A talk title with a colon in it is normal and would otherwise become two
/// keys, so quoting unconditionally costs nothing and removes the whole class.
/// The failure it prevents surfaces in whatever static site generator reads the
/// file, long after this crate ran.
pub(crate) fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_with_a_colon_in_it_stays_one_key() {
        assert_eq!(yaml_string("Slides: a talk"), "\"Slides: a talk\"");
    }

    #[test]
    fn a_quote_the_author_typed_is_escaped_rather_than_ending_the_value() {
        assert_eq!(yaml_string(r#"a "talk""#), r#""a \"talk\"""#);
        assert_eq!(yaml_string(r"a\path"), r#""a\\path""#);
    }
}
