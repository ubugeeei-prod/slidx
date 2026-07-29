//! What a link looks like in Markdown.
//!
//! The only place in the workspace that answers that question for publishing,
//! and it answers it by scanning rather than by matching a pattern: the four
//! syntaxes have to be tried in a fixed order at each position so that an
//! earlier one consumes the text a later one would otherwise re-match. The URL
//! inside `[docs](https://…)` must not also be found as a bare URL, and an
//! image's URL must be swallowed before the bare-URL branch can see it.
//!
//! Code is excluded before the scan begins. A URL inside a fenced block is
//! usually an example endpoint or an import path, and listing
//! `https://api.example.com/v1` as a resource sends people somewhere that does
//! not exist. Fence awareness is the same rule `scanner.rs` in `slidx_core`
//! applies to slide separators, for the same reason.

/// A link as it was written, before any policy is applied to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub url: String,
    /// Link text where the author wrote some, otherwise empty.
    pub label: String,
}

/// Every link in one block of Markdown, in source order.
pub fn scan(markdown: &str) -> Vec<Found> {
    let text: Vec<char> = without_code(markdown).chars().collect();
    let mut found = Vec::new();
    let mut at = 0;

    while at < text.len() {
        // An image is matched only so its URL is consumed here. An asset is not
        // a resource.
        if let Some(next) = image(&text, at) {
            at = next;
            continue;
        }

        let line_start = at == 0 || text[at - 1] == '\n';
        let matched = inline(&text, at)
            .or_else(|| line_start.then(|| reference(&text, at)).flatten())
            .or_else(|| autolink(&text, at))
            .or_else(|| bare(&text, at));

        let Some((next, link)) = matched else {
            at += 1;
            continue;
        };

        at = next;
        let url = trim_trailing_punctuation(&link.url);

        // Only web links are resources. `mailto:` is an address and a relative
        // path is part of the deck; neither is something an attendee can open
        // from a page of links.
        if is_http(&url) {
            let label = clean_label(&link.label);
            found.push(Found {
                label: if label.is_empty() { label_for_url(&url) } else { label },
                url,
            });
        }
    }

    found
}

/// `![alt](src)`, returning where it ends.
fn image(text: &[char], at: usize) -> Option<usize> {
    let at = expect(text, at, '!')?;
    let (at, _) = bracketed(text, at)?;
    let at = expect(text, at, '(')?;
    let (at, _) = take_until(text, at, ')')?;

    expect(text, at, ')')
}

/// `[text](url)`, with an optional title.
fn inline(text: &[char], at: usize) -> Option<(usize, Found)> {
    let (at, label) = bracketed(text, at)?;
    let at = expect(text, at, '(')?;
    let at = skip_spaces(text, at);
    let at = expect(text, at, '<').unwrap_or(at);

    let (at, url) = take_while(text, at, |c| !c.is_whitespace() && c != ')' && c != '>')?;
    let at = expect(text, at, '>').unwrap_or(at);
    let at = title(text, at).unwrap_or(at);
    let at = skip_spaces(text, at);
    let at = expect(text, at, ')')?;

    Some((at, Found { url, label }))
}

/// ` "The docs"` after an inline URL.
fn title(text: &[char], at: usize) -> Option<usize> {
    let after_space = skip_spaces(text, at);
    if after_space == at {
        return None;
    }

    let quote = *text.get(after_space)?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let (at, _) = take_until(text, after_space + 1, |c| c == '"' || c == '\'')?;
    expect(text, at, |c| c == '"' || c == '\'')
}

/// `[id]: url` — a reference definition, always alone on its line.
fn reference(text: &[char], at: usize) -> Option<(usize, Found)> {
    let at = skip_spaces_up_to(text, at, 3);
    let (at, label) = bracketed(text, at)?;
    if label.is_empty() {
        return None;
    }

    let at = expect(text, at, ':')?;
    let at = skip_spaces(text, at);

    let end = text[at..].iter().position(|c| *c == '\n').map_or(text.len(), |offset| at + offset);
    let line: String = text[at..end].iter().collect();
    let url = line.trim_end();

    // The URL runs to the end of the line, so anything after a space is not
    // part of a reference definition at all — the scanner falls through and
    // whatever is on the line is read as prose.
    if url.is_empty() || url.contains(char::is_whitespace) {
        return None;
    }

    // One pair of angle brackets, not every one: `<https://x>>` addresses a
    // path that ends in a bracket and the author wrote both.
    let url = url.strip_prefix('<').unwrap_or(url);
    let url = url.strip_suffix('>').unwrap_or(url);

    Some((end, Found { url: url.to_string(), label }))
}

/// `<https://…>`.
fn autolink(text: &[char], at: usize) -> Option<(usize, Found)> {
    let at = expect(text, at, '<')?;
    let start = at;
    let at = scheme(text, at)?;
    let (at, _) = take_while(text, at, |c| !c.is_whitespace() && c != '>')?;
    let url: String = text[start..at].iter().collect();

    Some((expect(text, at, '>')?, Found { url, label: String::new() }))
}

/// A URL written into prose.
fn bare(text: &[char], at: usize) -> Option<(usize, Found)> {
    let start = at;
    let at = scheme(text, at)?;
    let (at, _) =
        take_while(text, at, |c| !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | '\'' | ']'))?;

    Some((at, Found { url: text[start..at].iter().collect(), label: String::new() }))
}

/// `https://`, `git+ssh://`, and anything else shaped like a scheme.
///
/// The `://` is required: a bare `mailto:` has no authority and is not
/// something the scanner should swallow half of.
fn scheme(text: &[char], at: usize) -> Option<usize> {
    if !text.get(at)?.is_ascii_alphabetic() {
        return None;
    }

    let mut end = at + 1;
    while text.get(end).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) {
        end += 1;
    }

    if text.get(end..end + 3)? == [':', '/', '/'] {
        Some(end + 3)
    } else {
        None
    }
}

/// `[…]`, returning what was inside it.
fn bracketed(text: &[char], at: usize) -> Option<(usize, String)> {
    let at = expect(text, at, '[')?;
    let (at, inside) = take_until(text, at, ']')?;

    Some((expect(text, at, ']')?, inside))
}

/// One character, named either as itself or as a class it belongs to.
///
/// Two spellings because both read better in their own place: `expect(text, at,
/// '(')` says what the syntax requires, and a closure says what a class is.
trait Matcher {
    fn matches(&self, character: char) -> bool;
}

impl Matcher for char {
    fn matches(&self, character: char) -> bool {
        *self == character
    }
}

impl<F: Fn(char) -> bool> Matcher for F {
    fn matches(&self, character: char) -> bool {
        self(character)
    }
}

fn expect(text: &[char], at: usize, what: impl Matcher) -> Option<usize> {
    what.matches(*text.get(at)?).then_some(at + 1)
}

/// Everything up to the next `stop`, which must exist.
fn take_until(text: &[char], at: usize, stop: impl Matcher) -> Option<(usize, String)> {
    let end = text[at..].iter().position(|c| stop.matches(*c))? + at;
    Some((end, text[at..end].iter().collect()))
}

/// One or more characters the predicate accepts.
fn take_while(text: &[char], at: usize, keep: impl Fn(char) -> bool) -> Option<(usize, String)> {
    let mut end = at;
    while text.get(end).is_some_and(|c| keep(*c)) {
        end += 1;
    }

    (end > at).then(|| (end, text[at..end].iter().collect()))
}

fn skip_spaces(text: &[char], at: usize) -> usize {
    skip_spaces_up_to(text, at, usize::MAX)
}

/// Whitespace that is not a newline.
///
/// A link's parts may be spread across a line but not across a paragraph, and
/// letting a newline through here would find a "link" made of two unrelated
/// lines that happened to end and begin with the right brackets.
fn skip_spaces_up_to(text: &[char], at: usize, most: usize) -> usize {
    let mut end = at;
    while end - at < most && text.get(end).is_some_and(|c| c.is_whitespace() && *c != '\n') {
        end += 1;
    }
    end
}

/// Drops the punctuation that ended the sentence rather than the URL.
///
/// A closing bracket is kept when the URL opened one, because Wikipedia and MDN
/// both publish paths that contain balanced parentheses.
fn trim_trailing_punctuation(url: &str) -> String {
    let mut trimmed = url.trim_end_matches(['.', ',', ';', ':', '!', '?', '"', '\'']).to_string();

    while trimmed.ends_with(')') && count_of(&trimmed, ')') > count_of(&trimmed, '(') {
        trimmed.pop();
    }

    trimmed
}

fn count_of(text: &str, character: char) -> usize {
    text.chars().filter(|found| *found == character).count()
}

/// Link text, flattened to one line and stripped of emphasis markers.
fn clean_label(text: &str) -> String {
    let stripped: String = text.chars().filter(|c| !matches!(c, '*' | '_' | '`')).collect();

    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A readable stand-in for link text: the URL without the noise.
pub fn label_for_url(url: &str) -> String {
    let without_scheme = url.find("://").map_or(url, |at| &url[at + 3..]);

    without_scheme.trim_start_matches("www.").trim_end_matches('/').to_string()
}

/// Only web links are resources.
pub fn is_http(url: &str) -> bool {
    let lowered = url.to_ascii_lowercase();

    lowered.starts_with("http://") || lowered.starts_with("https://")
}

/// Markdown with code removed.
///
/// Line-based rather than a scan over the whole block: a fence is closed by a
/// marker of the same character and at least the same length, which is a rule
/// about lines and reads as one.
fn without_code(markdown: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut fence: Option<(char, usize)> = None;

    for line in markdown.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let marker = fence_marker(line);

        match (fence, marker) {
            (None, Some(opened)) => fence = Some(opened),
            (None, None) => kept.push(without_code_spans(line)),
            (Some((character, width)), Some((closing, closing_width)))
                if closing == character && closing_width >= width =>
            {
                fence = None;
            }
            (Some(_), _) => {}
        }
    }

    kept.join("\n")
}

/// The fence a line opens or closes with, as its character and its width.
///
/// Four spaces of indent is an indented code block rather than a fence, so a
/// marker further in than three columns is not one.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let indent = line.chars().take_while(|c| c.is_whitespace()).count();
    if indent > 3 {
        return None;
    }

    let rest = line.trim_start();
    let character = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let width = rest.chars().take_while(|c| *c == character).count();

    (width >= 3).then_some((character, width))
}

/// Inline code spans removed, so a URL shown as `code` is not a resource.
fn without_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(open) = rest.find('`') {
        match rest[open + 1..].find('`') {
            Some(close) => {
                out.push_str(&rest[..open]);
                rest = &rest[open + 1 + close + 1..];
            }
            None => break,
        }
    }

    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(markdown: &str) -> Vec<String> {
        scan(markdown).into_iter().map(|link| link.url).collect()
    }

    fn labels(markdown: &str) -> Vec<String> {
        scan(markdown).into_iter().map(|link| link.label).collect()
    }

    #[test]
    fn an_inline_link_keeps_the_authors_words_as_its_label() {
        assert_eq!(urls("See [the docs](https://slidx.dev/docs)."), ["https://slidx.dev/docs"]);
        assert_eq!(labels("See [the docs](https://slidx.dev/docs)."), ["the docs"]);
    }

    #[test]
    fn a_link_carrying_a_title_attribute_is_still_read() {
        assert_eq!(urls(r#"[docs](https://slidx.dev "The docs")"#), ["https://slidx.dev"]);
    }

    #[test]
    fn an_autolink_is_read_without_its_angle_brackets() {
        assert_eq!(urls("<https://slidx.dev>"), ["https://slidx.dev"]);
    }

    #[test]
    fn a_reference_definition_is_labelled_with_its_reference() {
        assert_eq!(
            scan("[docs]: https://slidx.dev/docs"),
            [Found { url: "https://slidx.dev/docs".into(), label: "docs".into() }]
        );
    }

    #[test]
    fn a_reference_definition_is_only_one_when_it_starts_its_own_line() {
        assert_eq!(urls("see [docs]: https://slidx.dev/docs"), ["https://slidx.dev/docs"]);
        assert_eq!(labels("see [docs]: https://slidx.dev/docs"), ["slidx.dev/docs"]);
    }

    #[test]
    fn a_bare_url_loses_the_full_stop_that_ended_the_sentence() {
        assert_eq!(urls("See https://slidx.dev/docs."), ["https://slidx.dev/docs"]);
    }

    #[test]
    fn parentheses_that_belong_to_the_url_survive() {
        // Wikipedia and MDN both publish paths with balanced brackets in them.
        let content = "https://en.wikipedia.org/wiki/Deck_(cards)";
        assert_eq!(urls(content), [content]);
    }

    #[test]
    fn the_bracket_that_closed_the_sentence_is_dropped_rather_than_the_url() {
        assert_eq!(urls("(see https://slidx.dev/docs)"), ["https://slidx.dev/docs"]);
    }

    #[test]
    fn a_url_with_no_link_text_is_labelled_by_itself_without_the_noise() {
        assert_eq!(labels("https://www.slidx.dev/docs/"), ["slidx.dev/docs"]);
    }

    #[test]
    fn a_url_inside_a_fenced_code_block_is_not_a_resource() {
        // `https://api.example.com/v1` on a resources page sends people
        // somewhere that does not exist.
        let content = "```js\nfetch(\"https://api.example.com/v1\");\n```";
        assert!(urls(content).is_empty());
    }

    #[test]
    fn a_fence_is_closed_only_by_a_marker_of_its_own_kind_and_at_least_its_width() {
        let content = "~~~\nhttps://api.example.com/v1\n~~~~\nhttps://slidx.dev";
        assert_eq!(urls(content), ["https://slidx.dev"]);
    }

    #[test]
    fn a_url_inside_an_inline_code_span_is_not_a_resource() {
        assert!(urls("Run against `https://localhost:3000` while developing.").is_empty());
    }

    #[test]
    fn an_image_is_an_asset_rather_than_a_resource() {
        assert!(urls("![diagram](https://cdn.example.com/diagram.png)").is_empty());
    }

    #[test]
    fn an_address_that_is_not_a_web_link_is_left_alone() {
        assert!(urls("[mail](mailto:me@example.com)").is_empty());
        assert!(urls("[slide two](../2/)").is_empty());
    }

    #[test]
    fn a_url_inside_a_link_is_not_also_found_as_a_bare_one() {
        // The reason the syntaxes are tried in a fixed order at each position.
        assert_eq!(urls("[docs](https://slidx.dev/docs)").len(), 1);
    }

    #[test]
    fn a_bracket_the_author_typed_into_link_text_stays_in_the_label() {
        assert_eq!(labels("[draft [notes](https://slidx.dev/x)"), ["draft [notes"]);
    }

    #[test]
    fn emphasis_markers_are_dropped_from_a_label_and_its_runs_of_space_collapse() {
        assert_eq!(labels("[**the**  _docs_](https://slidx.dev)"), ["the docs"]);
    }

    #[test]
    fn a_code_span_inside_link_text_is_removed_with_every_other_code_span() {
        // Inline code is stripped before the scan begins, so a label made of it
        // falls back to the URL. Reading the span back out here would need a
        // second answer to "what is code", which is the one thing this module
        // must not grow.
        assert_eq!(labels("[`docs`](https://slidx.dev/docs)"), ["slidx.dev/docs"]);
    }
}
