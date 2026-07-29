//! What a scanner is allowed to say about a piece of code.
//!
//! Six kinds, and no more. Every kind costs a colour in every palette of every
//! theme, and every one of those colours is held to the contrast rules — so the
//! set is chosen for what an audience can actually tell apart on a projected
//! slide, not for what an editor can distinguish on a desk.
//!
//! Two of the six are worth defining, because a scanner has to place every
//! word into one of them:
//!
//! - [`Token::Keyword`] is a word the *language* reserves: `fn`, `await`,
//!   `elif`, `esac`. An HTML attribute name counts, because the HTML spec is
//!   what gives `href` its meaning.
//! - [`Token::Type`] is a name the *document* gives meaning to: a Rust struct,
//!   a TypeScript interface, an HTML tag, a JSON key.
//!
//! Anything a scanner cannot place is [`Token::Plain`], which is the whole
//! design: unclassified code looks like code, whereas wrongly classified code
//! looks like a lie.

/// One classification a scanner can assign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    /// Unclassified. Drawn in the theme's ordinary code colour.
    Plain,
    Comment,
    String,
    Number,
    /// A word the language reserves.
    Keyword,
    /// A name the document gives meaning to.
    Type,
    Punctuation,
}

impl Token {
    /// Every token that carries a colour of its own.
    ///
    /// [`Token::Plain`] is absent deliberately: it is the *absence* of a
    /// classification, and giving it a class would put a second colour where
    /// the theme's code colour already is.
    pub const COLOURED: [Self; 6] =
        [Self::Comment, Self::String, Self::Number, Self::Keyword, Self::Type, Self::Punctuation];

    /// Stable token name, used for CSS classes and theme properties.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Comment => "comment",
            Self::String => "string",
            Self::Number => "number",
            Self::Keyword => "keyword",
            Self::Type => "type",
            Self::Punctuation => "punctuation",
        }
    }

    /// The CSS class this token is emitted with, if it has one.
    pub fn class(self) -> Option<String> {
        match self {
            Self::Plain => None,
            other => Some(format!("slidx-code-{}", other.as_token())),
        }
    }
}

/// One classified run of the source, as a byte range.
///
/// Ranges rather than owned strings: a slide's code block is rendered several
/// times over a build — the slide, the print sheet, the shared snippet page —
/// and none of those need a copy of the source to do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub token: Token,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(token: Token, start: usize, end: usize) -> Self {
        Self { token, start, end }
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_carries_no_class_of_its_own() {
        // A class on unclassified code would put a second colour exactly where
        // the theme's own code colour already is.
        assert_eq!(Token::Plain.class(), None);
        assert_eq!(Token::Comment.class().as_deref(), Some("slidx-code-comment"));
    }

    #[test]
    fn every_coloured_token_has_a_class_and_plain_is_not_among_them() {
        assert!(!Token::COLOURED.contains(&Token::Plain));

        for token in Token::COLOURED {
            assert!(token.class().is_some(), "{} has no class", token.as_token());
        }
    }

    #[test]
    fn token_names_are_unique_so_two_roles_cannot_share_a_colour() {
        let mut names: Vec<&str> = Token::COLOURED.iter().map(|token| token.as_token()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total);
    }

    #[test]
    fn a_span_addresses_the_source_it_came_from() {
        let source = "let x = 1;";
        let span = Span::new(Token::Keyword, 0, 3);

        assert_eq!(span.text(source), "let");
        assert!(!span.is_empty());
    }
}
