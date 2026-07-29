//! What one language differs from another by.
//!
//! Rust, TypeScript, Python, JSON, and shell are scanned by the same driver.
//! They differ in a handful of decisions — how a comment opens, which quotes
//! exist, whether a backslash escapes inside them — and those decisions are
//! data here rather than five near-identical loops.
//!
//! HTML is not in that shape and has its own scanner. Forcing it into this
//! table would mean a flag that only HTML sets, which is a copy of the HTML
//! scanner written in booleans.

/// How a comment opens and closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Comment {
    /// Runs to the end of the line.
    Line(&'static str),
    /// Runs to the end of the line, but only when the opener starts a word.
    ///
    /// Shell's `#`. Mid-word it is a URL fragment, a colour, or a Git revision,
    /// and greying out the rest of the command is the failure this prevents.
    Word(&'static str),
    /// Opens and closes, and does not nest.
    Flat(&'static str, &'static str),
    /// Opens and closes, and nests. Rust's `/* /* */ */` is one comment.
    Nested(&'static str, &'static str),
}

/// A string delimiter and what a backslash means inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quote {
    /// A backslash escapes the next character.
    Escaped(char),
    /// A backslash is an ordinary character. Shell's `'…'`.
    Literal(char),
}

impl Quote {
    pub(crate) fn delimiter(self) -> char {
        match self {
            Self::Escaped(delimiter) | Self::Literal(delimiter) => delimiter,
        }
    }

    pub(crate) fn escapes(self) -> bool {
        matches!(self, Self::Escaped(_))
    }
}

/// Everything the generic scanner needs to know about one language.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Syntax {
    pub(crate) comments: &'static [Comment],
    pub(crate) quotes: &'static [Quote],
    /// Rust's `r"…"` and `r#"…"#`, where a backslash means nothing.
    pub(crate) raw_strings: bool,
    /// Rust's `'a'`, which shares its opener with a lifetime.
    pub(crate) char_literals: bool,
    /// Python's `"""…"""`, which spans lines and swallows single quotes.
    pub(crate) triple_quotes: bool,
    /// An identifier that names something, rather than a word the language
    /// reserves: `HashMap`, `ReadonlyArray`. Capitalised *and* mixed case, so
    /// a `SCREAMING_CASE` constant is left alone.
    pub(crate) named_types: bool,
    /// JSON's object keys: a string that a colon follows is a name.
    pub(crate) keys_before_colon: bool,
}

impl Syntax {
    /// A language with nothing declared. Every table starts from this.
    pub(crate) const EMPTY: Self = Self {
        comments: &[],
        quotes: &[],
        raw_strings: false,
        char_literals: false,
        triple_quotes: false,
        named_types: false,
        keys_before_colon: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;

    #[test]
    fn a_literal_quote_does_not_honour_a_backslash() {
        // Shell's `'…'`: `'a\'` is a complete string whose last byte is a
        // backslash, and treating the escape as real runs the string on.
        assert!(!Quote::Literal('\'').escapes());
        assert!(Quote::Escaped('"').escapes());
    }

    #[test]
    fn a_quote_reports_the_character_that_opens_it() {
        assert_eq!(Quote::Literal('\'').delimiter(), '\'');
        assert_eq!(Quote::Escaped('`').delimiter(), '`');
    }

    #[test]
    fn a_language_that_declares_nothing_is_left_entirely_plain() {
        // What `EMPTY` has to mean for the tables to be readable as a diff from
        // it: a language that turns nothing on gets no colour at all, rather
        // than some default grammar's opinion.
        let spans = crate::scan("fn main() { /* hi */ }", crate::Language::Html);
        let tokens: Vec<Token> = spans.iter().map(|span| span.token).collect();

        assert_eq!(tokens, vec![Token::Plain], "{spans:?}");
    }
}
