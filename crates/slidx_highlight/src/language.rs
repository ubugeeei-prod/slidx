//! The six languages, and the small table each one is scanned from.
//!
//! Six, chosen from what a technical talk puts on a slide rather than from what
//! a grammar registry happens to contain. Every one of them is scanned by code
//! in this crate that somebody read; a seventh added by pattern-matching a
//! grammar file would be a language nobody has checked.
//!
//! An info string this module does not recognise yields no language, and no
//! language means no highlighting. Guessing would be worse than plain text:
//! plain code reads as code, and code coloured by the wrong grammar reads as a
//! mistake the author made.

use crate::syntax::{Comment, Quote, Syntax};

/// A language slidx can highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Json,
    Html,
    Shell,
}

impl Language {
    pub const ALL: [Self; 7] = [
        Self::Rust,
        Self::TypeScript,
        Self::JavaScript,
        Self::Python,
        Self::Json,
        Self::Html,
        Self::Shell,
    ];

    /// Canonical name, as it appears in `language-*` on the rendered element.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Json => "json",
            Self::Html => "html",
            Self::Shell => "shell",
        }
    }

    /// Resolves a fence info string's language word.
    ///
    /// Aliases are the ones people actually type at the top of a fence. The
    /// list is deliberately short: an alias nobody writes is an alias that only
    /// widens the surface where a wrong grammar can be chosen.
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim().to_ascii_lowercase();

        Some(match word.as_str() {
            "rust" | "rs" => Self::Rust,
            "typescript" | "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "javascript" | "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "python" | "py" => Self::Python,
            "json" => Self::Json,
            "html" | "htm" => Self::Html,
            "shell" | "sh" | "bash" | "zsh" => Self::Shell,
            _ => return None,
        })
    }

    /// True when this word is reserved by the language itself.
    pub(crate) fn is_keyword(self, word: &str) -> bool {
        match self {
            Self::Rust => RUST_KEYWORDS.contains(&word),
            Self::TypeScript => JS_KEYWORDS.contains(&word) || TS_KEYWORDS.contains(&word),
            Self::JavaScript => JS_KEYWORDS.contains(&word),
            Self::Python => PYTHON_KEYWORDS.contains(&word),
            Self::Json => JSON_KEYWORDS.contains(&word),
            Self::Shell => SHELL_KEYWORDS.contains(&word),
            // Every word inside an HTML tag is spelled by the HTML spec, so
            // the tag scanner classifies them by position instead of by list.
            Self::Html => false,
        }
    }

    /// True when this word names a type the language ships with.
    pub(crate) fn is_type(self, word: &str) -> bool {
        match self {
            Self::Rust => RUST_TYPES.contains(&word),
            Self::TypeScript => TS_TYPES.contains(&word),
            Self::Python => PYTHON_TYPES.contains(&word),
            Self::JavaScript | Self::Json | Self::Html | Self::Shell => false,
        }
    }

    /// The table the generic scanner reads this language from.
    pub(crate) fn syntax(self) -> Syntax {
        match self {
            Self::Rust => Syntax {
                comments: &[Comment::Line("//"), Comment::Nested("/*", "*/")],
                quotes: &[Quote::Escaped('"')],
                raw_strings: true,
                char_literals: true,
                named_types: true,
                ..Syntax::EMPTY
            },
            Self::TypeScript | Self::JavaScript => Syntax {
                comments: &[Comment::Line("//"), Comment::Flat("/*", "*/")],
                quotes: &[Quote::Escaped('"'), Quote::Escaped('\''), Quote::Escaped('`')],
                named_types: true,
                ..Syntax::EMPTY
            },
            Self::Python => Syntax {
                comments: &[Comment::Line("#")],
                quotes: &[Quote::Escaped('"'), Quote::Escaped('\'')],
                triple_quotes: true,
                named_types: true,
                ..Syntax::EMPTY
            },
            Self::Json => {
                Syntax { quotes: &[Quote::Escaped('"')], keys_before_colon: true, ..Syntax::EMPTY }
            }
            // A `#` mid-word is a fragment, a colour, or part of a path, and
            // treating it as a comment would grey out the rest of the command.
            Self::Shell => Syntax {
                comments: &[Comment::Word("#")],
                quotes: &[Quote::Escaped('"'), Quote::Literal('\'')],
                ..Syntax::EMPTY
            },
            // Handled by its own scanner: HTML has no words outside markup.
            Self::Html => Syntax::EMPTY,
        }
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while", "yield",
];

const RUST_TYPES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8", "u16",
    "u32", "u64", "u128", "usize",
];

const JS_KEYWORDS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

const TS_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "declare",
    "enum",
    "implements",
    "infer",
    "interface",
    "is",
    "keyof",
    "namespace",
    "private",
    "protected",
    "public",
    "readonly",
    "satisfies",
    "static",
    "type",
];

const TS_TYPES: &[&str] =
    &["any", "bigint", "boolean", "never", "number", "object", "string", "symbol", "unknown"];

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

const PYTHON_TYPES: &[&str] = &[
    "bool",
    "bytes",
    "complex",
    "dict",
    "float",
    "frozenset",
    "int",
    "list",
    "object",
    "set",
    "str",
    "tuple",
];

const JSON_KEYWORDS: &[&str] = &["false", "null", "true"];

/// POSIX reserved words plus the declaration builtins.
///
/// `echo`, `cd`, and the rest are commands rather than syntax, and colouring
/// them would mean colouring some of a pipeline's words and not others.
const SHELL_KEYWORDS: &[&str] = &[
    "case", "declare", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function",
    "if", "in", "local", "readonly", "return", "select", "then", "time", "until", "unset", "while",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_languages_a_technical_talk_shows_are_recognised() {
        assert_eq!(Language::parse("rust"), Some(Language::Rust));
        assert_eq!(Language::parse("ts"), Some(Language::TypeScript));
        assert_eq!(Language::parse("js"), Some(Language::JavaScript));
        assert_eq!(Language::parse("py"), Some(Language::Python));
        assert_eq!(Language::parse("json"), Some(Language::Json));
        assert_eq!(Language::parse("html"), Some(Language::Html));
        assert_eq!(Language::parse("bash"), Some(Language::Shell));
    }

    #[test]
    fn an_unrecognised_language_yields_nothing_rather_than_a_guess() {
        // Code coloured by the wrong grammar reads as a mistake the author
        // made. Plain code just reads as code.
        assert_eq!(Language::parse("elixir"), None);
        assert_eq!(Language::parse(""), None);
        assert_eq!(Language::parse("rustlang"), None);
    }

    #[test]
    fn a_language_word_is_matched_whatever_its_case() {
        assert_eq!(Language::parse("Rust"), Some(Language::Rust));
        assert_eq!(Language::parse("JSON"), Some(Language::Json));
    }

    #[test]
    fn every_language_round_trips_through_its_canonical_name() {
        for language in Language::ALL {
            assert_eq!(Language::parse(language.as_token()), Some(language));
        }
    }

    #[test]
    fn typescript_reserves_words_javascript_does_not() {
        assert!(Language::TypeScript.is_keyword("interface"));
        assert!(!Language::JavaScript.is_keyword("interface"));

        // And everything JavaScript reserves is still reserved in TypeScript.
        for word in JS_KEYWORDS {
            assert!(Language::TypeScript.is_keyword(word), "{word} lost in TypeScript");
        }
    }

    #[test]
    fn no_keyword_list_repeats_itself() {
        // A duplicate is invisible at a glance and means one of the two was
        // meant to be a different word.
        for list in [RUST_KEYWORDS, JS_KEYWORDS, TS_KEYWORDS, PYTHON_KEYWORDS, SHELL_KEYWORDS] {
            let mut sorted = list.to_vec();
            let total = sorted.len();
            sorted.sort_unstable();
            sorted.dedup();

            assert_eq!(sorted.len(), total, "a keyword is listed twice in {list:?}");
        }
    }

    #[test]
    fn a_word_is_never_both_reserved_and_a_built_in_type() {
        // The scanner checks keywords first, so an overlap would be a colour
        // silently unreachable rather than an error.
        for language in Language::ALL {
            for word in [RUST_TYPES, TS_TYPES, PYTHON_TYPES].concat() {
                if language.is_type(word) {
                    assert!(
                        !language.is_keyword(word),
                        "{word} is both reserved and a type in {}",
                        language.as_token()
                    );
                }
            }
        }
    }

    #[test]
    fn only_the_language_that_has_a_construct_declares_it() {
        // Every table is a diff from `Syntax::EMPTY`, so a stray `true` here is
        // a language scanned by a rule that does not apply to it — Python raw
        // strings, JSON character literals — which is exactly the confidently
        // wrong colour this crate is built to avoid.
        for language in Language::ALL {
            let syntax = language.syntax();
            let name = language.as_token();

            assert_eq!(syntax.raw_strings, language == Language::Rust, "raw strings in {name}");
            assert_eq!(syntax.char_literals, language == Language::Rust, "characters in {name}");
            assert_eq!(syntax.triple_quotes, language == Language::Python, "triples in {name}");
            assert_eq!(syntax.keys_before_colon, language == Language::Json, "keys in {name}");
        }
    }

    #[test]
    fn shell_reserves_syntax_rather_than_commands() {
        // Colouring `echo` would mean colouring some of a pipeline's words and
        // not others, which reads as a scanner that gave up halfway.
        assert!(Language::Shell.is_keyword("done"));
        assert!(!Language::Shell.is_keyword("echo"));
        assert!(!Language::Shell.is_keyword("npm"));
    }
}
