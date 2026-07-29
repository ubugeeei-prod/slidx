//! The scanner.
//!
//! One pass, left to right, no backtracking beyond a fixed lookahead. It knows
//! about comments, strings, numbers, words, and punctuation, and it knows that
//! it does not know anything else.
//!
//! # The rule everything here follows
//!
//! When the scanner cannot decide, it emits [`Token::Plain`] and moves on.
//! Plain code looks like code; wrongly coloured code looks like a mistake the
//! author made, and an audience cannot tell which of the two they are looking
//! at. So every ambiguity in this file resolves towards *no colour* rather than
//! towards a guess.
//!
//! Two of them are unresolvable without a parser, and both are decided that
//! way here:
//!
//! - **Rust's `'`** opens a character literal and names a lifetime. A fixed
//!   lookahead settles it for everything a slide shows; where the lookahead
//!   finds no closing quote, the quote and the name after it go out plain, so
//!   `&'static str` never colours `static` as the keyword it spells.
//! - **A string that never closes** leaves the rest of the block plain rather
//!   than painting it as one long string. Slides show fragments of files, and a
//!   fragment routinely cuts a literal in half.

use crate::language::Language;
use crate::syntax::Syntax;
use crate::token::{Span, Token};

mod html;
mod literal;

use literal::{
    char_literal_end, colon_follows, comment_end, long_string_end, number_end, quote_at, quoted_end,
};

/// Classifies every byte of `source`.
///
/// The returned spans cover the source exactly once and in order, so a caller
/// can render them by concatenation without consulting the source's shape.
pub fn scan(source: &str, language: Language) -> Vec<Span> {
    match language {
        Language::Html => html::scan(source),
        other => scan_words(source, other),
    }
}

/// Collects spans, merging touching runs of the same token.
///
/// Merging is not a micro-optimisation: without it every character of
/// punctuation in a block becomes its own element, and a slide's code block
/// triples in size for no visible difference.
pub(crate) struct Emitter<'a> {
    source: &'a str,
    /// Everything before this is already accounted for.
    pending: usize,
    spans: Vec<Span>,
}

impl<'a> Emitter<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source, pending: 0, spans: Vec::new() }
    }

    /// Records `start..end` as `token`, marking anything skipped as plain.
    pub(crate) fn push(&mut self, token: Token, start: usize, end: usize) {
        if start > self.pending {
            self.append(Token::Plain, self.pending, start);
        }

        self.append(token, start, end);
        self.pending = end.max(self.pending);
    }

    pub(crate) fn finish(mut self) -> Vec<Span> {
        let end = self.source.len();
        if end > self.pending {
            self.append(Token::Plain, self.pending, end);
        }

        self.spans
    }

    fn append(&mut self, token: Token, start: usize, end: usize) {
        if start >= end {
            return;
        }

        match self.spans.last_mut() {
            Some(last) if last.token == token && last.end == start => last.end = end,
            _ => self.spans.push(Span::new(token, start, end)),
        }
    }
}

fn scan_words(source: &str, language: Language) -> Vec<Span> {
    let syntax = language.syntax();
    let mut emitter = Emitter::new(source);
    let mut at = 0usize;

    while at < source.len() {
        if let Some(end) = comment_end(source, at, &syntax) {
            emitter.push(Token::Comment, at, end);
            at = end;
            continue;
        }

        // Before words, because Rust's raw strings open with `r`; before
        // quotes, because Python's `"""` opens with `"`.
        if let Some(end) = long_string_end(source, at, &syntax) {
            emitter.push(Token::String, at, end);
            at = end;
            continue;
        }

        if syntax.char_literals && source[at..].starts_with('\'') {
            let end = match char_literal_end(source, at) {
                Some(end) => {
                    emitter.push(Token::String, at, end);
                    end
                }
                // A lifetime or a loop label. Taking the name with it is what
                // keeps `&'static str` from colouring `static` as a keyword.
                None => {
                    let end = at + 1 + word_len(&source[at + 1..]);
                    emitter.push(Token::Plain, at, end);
                    end
                }
            };

            at = end;
            continue;
        }

        if let Some(quote) = quote_at(source, at, &syntax) {
            // Unterminated: stop classifying. Everything left is plain, which
            // is what a fragment cut through a literal should look like.
            let Some(end) = quoted_end(source, at, quote) else { break };

            let token = if syntax.keys_before_colon && colon_follows(source, end) {
                Token::Type
            } else {
                Token::String
            };

            emitter.push(token, at, end);
            at = end;
            continue;
        }

        if let Some(end) = number_end(source, at) {
            emitter.push(Token::Number, at, end);
            at = end;
            continue;
        }

        let length = word_len(&source[at..]);
        if length > 0 {
            let word = &source[at..at + length];
            emitter.push(classify(word, language, &syntax), at, at + length);
            at += length;
            continue;
        }

        let character = next_char(source, at);
        if character.is_ascii_punctuation() {
            emitter.push(Token::Punctuation, at, at + 1);
        }

        at += character.len_utf8();
    }

    emitter.finish()
}

fn classify(word: &str, language: Language, syntax: &Syntax) -> Token {
    if language.is_keyword(word) {
        return Token::Keyword;
    }
    if language.is_type(word) {
        return Token::Type;
    }

    // Capitalised *and* mixed case. `HashMap` names something; `MAX_RETRIES`
    // is a constant, and colouring it as a type would be a claim about the
    // code that the scanner has no way to check.
    let named = syntax.named_types
        && word.starts_with(|c: char| c.is_uppercase())
        && word.contains(|c: char| c.is_lowercase());

    if named {
        Token::Type
    } else {
        Token::Plain
    }
}

pub(crate) fn is_word_char(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

/// Length of the identifier at the start of `rest`, or zero.
pub(crate) fn word_len(rest: &str) -> usize {
    if rest.starts_with(|c: char| c.is_ascii_digit()) {
        return 0;
    }

    rest.find(|c: char| !is_word_char(c)).unwrap_or(rest.len())
}

/// The character at `at`, which is always on a boundary by construction.
pub(crate) fn next_char(source: &str, at: usize) -> char {
    source[at..].chars().next().unwrap_or('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every span, as the pair a reader cares about.
    fn tokens(source: &str, language: Language) -> Vec<(Token, &str)> {
        scan(source, language).into_iter().map(|span| (span.token, span.text(source))).collect()
    }

    /// How the first occurrence of `needle` was classified.
    fn token_of(source: &str, language: Language, needle: &str) -> Token {
        let at = source.find(needle).expect("the fixture contains the text it asks about");

        scan(source, language)
            .into_iter()
            .find(|span| span.start <= at && at < span.end)
            .map(|span| span.token)
            .expect("every byte is covered")
    }

    #[test]
    fn rust_tells_a_keyword_a_type_a_number_and_a_comment_apart() {
        let source = "let total: usize = 42; // counted\nlet map = HashMap::new();";

        assert_eq!(token_of(source, Language::Rust, "let"), Token::Keyword);
        assert_eq!(token_of(source, Language::Rust, "usize"), Token::Type);
        assert_eq!(token_of(source, Language::Rust, "42"), Token::Number);
        assert_eq!(token_of(source, Language::Rust, "// counted"), Token::Comment);
        assert_eq!(token_of(source, Language::Rust, "HashMap"), Token::Type);
        assert_eq!(token_of(source, Language::Rust, "total"), Token::Plain);
    }

    #[test]
    fn a_nested_rust_block_comment_ends_where_it_actually_ends() {
        // The reason Rust needs its own comment rule: a flat scanner stops at
        // the inner `*/` and colours the rest of the block as code.
        let source = "/* outer /* inner */ still */ let x = 1;";

        assert_eq!(token_of(source, Language::Rust, "still"), Token::Comment);
        assert_eq!(token_of(source, Language::Rust, "let"), Token::Keyword);
    }

    #[test]
    fn a_rust_raw_string_keeps_its_backslashes() {
        let source = "let pattern = r\"a\\\"; let after = 1;";

        assert_eq!(token_of(source, Language::Rust, "after"), Token::Plain);
        assert_eq!(token_of(source, Language::Rust, "r\"a\\\""), Token::String);
    }

    #[test]
    fn a_hashed_raw_string_is_closed_only_by_a_matching_hash() {
        let source = "r#\"say \"hi\"\"#";
        assert_eq!(tokens(source, Language::Rust), vec![(Token::String, source)]);
    }

    #[test]
    fn a_rust_lifetime_is_plain_rather_than_the_keyword_it_spells() {
        // Given up deliberately. `'` opens a character literal and names a
        // lifetime, and only a parser knows which. The lookahead takes the name
        // with the quote, so `static` here is never coloured as the keyword.
        let source = "fn hold<'a>(value: &'static str) -> &'a str { value }";

        assert_eq!(token_of(source, Language::Rust, "'static"), Token::Plain);
        assert_eq!(token_of(source, Language::Rust, "'a>"), Token::Plain);
        assert_eq!(token_of(source, Language::Rust, "fn"), Token::Keyword);
    }

    #[test]
    fn a_rust_character_literal_is_a_string_even_when_it_holds_a_quote() {
        for source in ["'x'", "'\\n'", "'\\''", "'\\u{1F600}'", "'あ'"] {
            assert_eq!(tokens(source, Language::Rust), vec![(Token::String, source)], "{source}");
        }
    }

    #[test]
    fn a_screaming_case_constant_is_not_claimed_to_be_a_type() {
        // Capitalisation alone would make every constant a type, which is a
        // claim about the code the scanner has no way to check.
        let source = "const MAX_RETRIES: u8 = 3; struct RetryPolicy;";

        assert_eq!(token_of(source, Language::Rust, "MAX_RETRIES"), Token::Plain);
        assert_eq!(token_of(source, Language::Rust, "RetryPolicy"), Token::Type);
    }

    #[test]
    fn a_range_between_two_integers_is_two_numbers() {
        assert_eq!(
            tokens("0..10", Language::Rust),
            vec![(Token::Number, "0"), (Token::Punctuation, ".."), (Token::Number, "10")]
        );
    }

    #[test]
    fn a_digit_inside_an_identifier_belongs_to_the_identifier() {
        assert_eq!(token_of("let utf8 = 1;", Language::Rust, "utf8"), Token::Plain);
        assert_eq!(token_of("let x = 1_000u64;", Language::Rust, "1_000u64"), Token::Number);
    }

    #[test]
    fn a_javascript_regular_expression_is_left_plain_rather_than_guessed_at() {
        // Given up deliberately. Deciding that `/` opens a literal needs the
        // previous *token*, and every heuristic that gets `/ab+c/` right also
        // paints `total / count / 2` as a string. Plain is the honest answer.
        let source = "const found = /ab+c/.test(input);";

        assert_eq!(token_of(source, Language::JavaScript, "ab+c"), Token::Plain);
        assert_eq!(token_of(source, Language::JavaScript, "const"), Token::Keyword);
    }

    #[test]
    fn a_double_slash_inside_a_regular_expression_reads_as_a_comment() {
        // The residual cost of refusing to detect regular expressions, stated
        // rather than hidden: a character class holding two slashes opens a
        // comment. It is the rarest shape in the rarest construct, and the
        // alternative miscolours ordinary arithmetic instead.
        let source = "const slash = /[//]/;";
        assert_eq!(token_of(source, Language::JavaScript, "//]/;"), Token::Comment);
    }

    #[test]
    fn a_template_literal_carries_its_interpolation_with_it() {
        // Given up deliberately. Highlighting inside `${…}` means re-entering
        // the scanner mid-string and tracking brace depth through nested
        // templates; the payoff is one colour on a slide.
        let source = "const line = `slide ${index + 1} of ${total}`;";
        assert_eq!(token_of(source, Language::JavaScript, "index + 1"), Token::String);
    }

    #[test]
    fn a_typescript_only_keyword_stays_plain_in_javascript() {
        assert_eq!(token_of("interface A {}", Language::TypeScript, "interface"), Token::Keyword);
        assert_eq!(token_of("interface A {}", Language::JavaScript, "interface"), Token::Plain);
    }

    #[test]
    fn typescript_primitive_types_are_types_rather_than_keywords() {
        let source = "function f(count: number): string { return String(count); }";

        assert_eq!(token_of(source, Language::TypeScript, "number"), Token::Type);
        assert_eq!(token_of(source, Language::TypeScript, "string"), Token::Type);
        assert_eq!(token_of(source, Language::TypeScript, "function"), Token::Keyword);
    }

    #[test]
    fn a_python_docstring_spans_lines_without_swallowing_the_function() {
        let source = "def f():\n    \"\"\"Say\n    hello.\"\"\"\n    return 1\n";

        assert_eq!(token_of(source, Language::Python, "hello."), Token::String);
        assert_eq!(token_of(source, Language::Python, "return"), Token::Keyword);
    }

    #[test]
    fn a_python_f_string_is_one_string_including_what_it_interpolates() {
        // Given up for the same reason as a template literal.
        let source = "print(f'{name} is {age}')";
        assert_eq!(token_of(source, Language::Python, "name"), Token::String);
    }

    #[test]
    fn a_hash_inside_a_python_string_does_not_open_a_comment() {
        let source = "colour = '#ff0000'\nsize = 2\n";

        assert_eq!(token_of(source, Language::Python, "#ff0000"), Token::String);
        assert_eq!(token_of(source, Language::Python, "2"), Token::Number);
    }

    #[test]
    fn a_json_key_is_told_apart_from_a_json_string_value() {
        // A key names a field and a value is data. Drawing them the same makes
        // a JSON slide one solid block of colour, which is no highlighting.
        let source = "{\"theme\": \"terminal\", \"duration\": 1200}";

        assert_eq!(token_of(source, Language::Json, "\"theme\""), Token::Type);
        assert_eq!(token_of(source, Language::Json, "\"terminal\""), Token::String);
        assert_eq!(token_of(source, Language::Json, "1200"), Token::Number);
    }

    #[test]
    fn json_has_no_comments_so_a_slash_stays_punctuation() {
        let source = "{\"url\": \"a\"}\n// not json\n";
        assert_eq!(token_of(source, Language::Json, "// not json"), Token::Punctuation);
    }

    #[test]
    fn a_hash_opens_a_shell_comment_only_at_the_start_of_a_word() {
        // Mid-word it is a URL fragment, a colour, or a Git revision, and
        // greying out the rest of the command is a line nobody can read.
        let source = "curl https://slidx.dev/#install # fetch it";

        assert_eq!(token_of(source, Language::Shell, "#install"), Token::Punctuation);
        assert_eq!(token_of(source, Language::Shell, "install "), Token::Plain);
        assert_eq!(token_of(source, Language::Shell, "# fetch it"), Token::Comment);
    }

    #[test]
    fn a_backslash_is_an_ordinary_character_inside_shell_single_quotes() {
        let source = "echo 'a\\' b";
        assert_eq!(token_of(source, Language::Shell, "'a\\'"), Token::String);
        assert_eq!(token_of(source, Language::Shell, " b"), Token::Plain);
    }

    #[test]
    fn shell_colours_its_syntax_and_leaves_commands_alone() {
        let source = "for f in *.md; do npm run build; done";

        assert_eq!(token_of(source, Language::Shell, "for"), Token::Keyword);
        assert_eq!(token_of(source, Language::Shell, "done"), Token::Keyword);
        assert_eq!(token_of(source, Language::Shell, "npm"), Token::Plain);
    }

    #[test]
    fn an_unterminated_string_leaves_the_rest_of_the_block_plain() {
        // Given up deliberately. Slides show fragments of files, and a fragment
        // routinely cuts a literal in half; painting the remainder as one long
        // string is the loudest possible way to be wrong.
        let source = "let a = \"unfinished\nlet b = 2;";
        let keywords: Vec<&str> = scan(source, Language::Rust)
            .into_iter()
            .filter(|span| span.token == Token::Keyword)
            .map(|span| span.text(source))
            .collect();

        assert_eq!(keywords, vec!["let"], "only the code before the break is classified");
        assert_eq!(token_of(source, Language::Rust, "let b"), Token::Plain);
    }
}
