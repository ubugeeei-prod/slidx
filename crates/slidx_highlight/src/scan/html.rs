//! Markup, which is not words.
//!
//! Every other language slidx highlights is a stream of words the scanner
//! classifies by name. HTML is the opposite: `href` means what it means because
//! of *where* it sits, and the same word in the text of the page means nothing
//! at all. So this scanner classifies by position, and the keyword tables the
//! others read are not consulted once.
//!
//! What that buys is the case a talk about HTML always contains: prose that
//! mentions a tag, next to markup that uses one. The prose stays plain.
//!
//! The one thing this deliberately does not do is look inside `<script>` and
//! `<style>`. Their contents are a different language, and switching scanners
//! mid-block would mean guessing which one — the guess this crate exists to
//! avoid.

use crate::scan::{next_char, word_len, Emitter};
use crate::token::Token;

pub(super) fn scan(source: &str) -> Vec<crate::token::Span> {
    let mut emitter = Emitter::new(source);
    let mut at = 0usize;

    while at < source.len() {
        if !source[at..].starts_with('<') {
            at += next_char(source, at).len_utf8();
            continue;
        }

        if source[at..].starts_with("<!--") {
            let end = source[at..].find("-->").map_or(source.len(), |offset| at + offset + 3);
            emitter.push(Token::Comment, at, end);
            at = end;
            continue;
        }

        // Doctypes and CDATA sections: one declaration, and nothing inside them
        // that a reader needs told apart.
        if source[at..].starts_with("<!") {
            let end = source[at..].find('>').map_or(source.len(), |offset| at + offset + 1);
            emitter.push(Token::Keyword, at, end);
            at = end;
            continue;
        }

        let closing = source[at + 1..].starts_with('/');
        let name_at = at + 1 + usize::from(closing);
        let name = word_len(&source[name_at..]);

        // `a < b` is arithmetic, not a tag. Requiring a name after the angle
        // bracket is the whole distinction, and it is the right one: a tag
        // without a name is not markup either.
        if name == 0 {
            at += 1;
            continue;
        }

        emitter.push(Token::Punctuation, at, name_at);
        emitter.push(Token::Type, name_at, name_at + name);
        at = scan_attributes(source, name_at + name, &mut emitter);
    }

    emitter.finish()
}

/// Classifies the inside of a tag, and returns where the tag ended.
fn scan_attributes(source: &str, from: usize, emitter: &mut Emitter<'_>) -> usize {
    let mut at = from;
    let mut expects_value = false;

    while at < source.len() {
        let character = next_char(source, at);

        if character == '>' {
            emitter.push(Token::Punctuation, at, at + 1);
            return at + 1;
        }

        if character.is_whitespace() {
            at += character.len_utf8();
            continue;
        }

        if character == '=' {
            emitter.push(Token::Punctuation, at, at + 1);
            expects_value = true;
            at += 1;
            continue;
        }

        if let Some(end) = quoted_end(source, at, character) {
            emitter.push(Token::String, at, end);
            expects_value = false;
            at = end;
            continue;
        }

        // An attribute's name is spelled by the HTML specification; its value
        // is whatever the author wrote. That is exactly the keyword/string
        // distinction every other language makes — and it is why an unquoted
        // value is taken whole: `href=/docs` is one value, not a slash and a
        // word, and the slash is only the self-closing marker outside one.
        if expects_value {
            let end = unquoted_value_end(source, at);
            emitter.push(Token::String, at, end);
            expects_value = false;
            at = end;
            continue;
        }

        let name = word_len(&source[at..]);
        if name == 0 {
            emitter.push(Token::Punctuation, at, at + character.len_utf8());
            at += character.len_utf8();
            continue;
        }

        emitter.push(Token::Keyword, at, at + name);
        at += name;
    }

    at
}

fn unquoted_value_end(source: &str, at: usize) -> usize {
    source[at..]
        .find(|c: char| c.is_whitespace() || c == '>')
        .map_or(source.len(), |offset| at + offset)
}

fn quoted_end(source: &str, at: usize, delimiter: char) -> Option<usize> {
    if delimiter != '"' && delimiter != '\'' {
        return None;
    }

    // An unterminated attribute value ends with the tag rather than running to
    // the end of the document, so one stray quote costs one attribute.
    let rest = &source[at + 1..];
    let end = rest.find([delimiter, '>'])?;

    rest[end..].starts_with(delimiter).then_some(at + 1 + end + 1)
}

#[cfg(test)]
mod tests {
    use crate::token::Token;
    use crate::Language;

    fn token_of(source: &str, needle: &str) -> Token {
        let at = source.find(needle).expect("the fixture contains the text it asks about");

        crate::scan(source, Language::Html)
            .into_iter()
            .find(|span| span.start <= at && at < span.end)
            .map(|span| span.token)
            .expect("every byte is covered")
    }

    #[test]
    fn a_tag_its_attribute_and_the_value_are_told_apart() {
        let source = "<a class=\"link\" href=/docs>Docs</a>";

        assert_eq!(token_of(source, "a class"), Token::Type);
        assert_eq!(token_of(source, "class"), Token::Keyword);
        assert_eq!(token_of(source, "\"link\""), Token::String);
        assert_eq!(token_of(source, "/docs"), Token::String);
        assert_eq!(token_of(source, "Docs<"), Token::Plain);
    }

    #[test]
    fn a_closing_tag_names_the_same_element_as_the_opening_one() {
        let source = "<p>text</p>";

        assert_eq!(token_of(source, "p>text"), Token::Type);
        assert_eq!(token_of(source, "p>"), Token::Type);
        assert_eq!(token_of(source, "text"), Token::Plain);
    }

    #[test]
    fn a_less_than_that_is_not_a_tag_stays_plain() {
        // A talk about HTML is full of prose that compares two numbers, and a
        // scanner that claims every `<` opens an element eats the rest of it.
        let source = "count < limit && limit > 0";

        assert_eq!(token_of(source, "< limit"), Token::Plain);
        assert_eq!(token_of(source, "> 0"), Token::Plain);
    }

    #[test]
    fn an_html_comment_covers_everything_inside_it() {
        let source = "<!-- <p>not a tag</p> --><p>real</p>";

        assert_eq!(token_of(source, "not a tag"), Token::Comment);
        assert_eq!(token_of(source, "p>real"), Token::Type);
    }

    #[test]
    fn a_doctype_is_one_declaration() {
        assert_eq!(token_of("<!doctype html>\n<p>a</p>", "<!doctype html>"), Token::Keyword);
    }

    #[test]
    fn the_contents_of_a_script_element_are_left_plain() {
        // Given up deliberately. Switching scanners inside a tag means deciding
        // which language a `type` attribute names, and that decision is the
        // guess this crate exists to avoid.
        let source = "<script>const x = 1;</script>";

        assert_eq!(token_of(source, "const"), Token::Plain);
        assert_eq!(token_of(source, "script>const"), Token::Type);
    }

    #[test]
    fn an_unterminated_attribute_value_costs_one_attribute_and_not_the_document() {
        let source = "<a href=\"/docs>text</a>";

        assert_eq!(token_of(source, "text"), Token::Plain);
        assert_eq!(token_of(source, "a href"), Token::Type);
    }
}
