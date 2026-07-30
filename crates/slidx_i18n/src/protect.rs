//! Which bytes of a line of prose a translator never sees.
//!
//! The one place that knows what is protected inside a run of Markdown, and the
//! reason the protection is structural rather than advisory. Each region is
//! replaced by `%1`, `%2`, … before the text reaches a catalogue, so a mark key,
//! a URL and a code span are simply not in the file a translator edits.
//!
//! Placeholders and not extraction, because word order moves. Japanese puts the
//! verb last and German the participle; a scheme that cut a sentence into the
//! runs between its markup would produce entries no translator could reassemble
//! into a sentence. A placeholder can be moved anywhere in the line.
//!
//! `%` is escaped as `%%` **only when a digit follows it**, so `50% faster`
//! reaches a translator as itself. Escaping every `%` would put `%%` into
//! ordinary prose, which reads as a bug to whoever reviews the diff.

use slidx_core::mark::find_marks;

/// A run of prose with its protected regions lifted out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Masked {
    /// The text as a translator sees it.
    pub text: String,
    /// What each placeholder stands for, in order.
    pub protected: Vec<String>,
}

impl Masked {
    /// True when nothing but markup was found, so there is nothing to translate.
    ///
    /// A line that is only an image, only a code span, or only a row of table
    /// rules has no words in it. Offering one to a translator wastes their time
    /// and invites them to translate a file name.
    pub fn has_words(&self) -> bool {
        self.text.chars().any(char::is_alphabetic)
    }
}

/// Replaces every region a translation must not touch with a placeholder.
pub fn mask(source: &str) -> Masked {
    let mut text = String::with_capacity(source.len());
    let mut protected: Vec<String> = Vec::new();
    let mut cursor = 0usize;

    while cursor < source.len() {
        let Some(region) = next_region(source, cursor) else {
            push_escaped(&mut text, &source[cursor..]);
            break;
        };

        push_escaped(&mut text, &source[cursor..region.start]);
        protected.push(source[region.start..region.end].to_string());
        text.push_str(&format!("%{}", protected.len()));
        cursor = region.end;
    }

    Masked { text, protected }
}

/// Puts every placeholder back, and reports the ones a translation dropped.
///
/// A dropped placeholder is not a formatting nit: `%1` standing for
/// `{#latency}` is the address a `steps:` entry points at, so silently writing
/// the translation without it produces a deck that renders and does not animate.
/// The caller refuses the entry rather than applying it.
pub fn restore(translation: &str, protected: &[String]) -> Result<String, usize> {
    let mut out = String::with_capacity(translation.len());
    let mut used = vec![false; protected.len()];
    let bytes = translation.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if bytes[cursor] != b'%' {
            let next = translation[cursor..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&translation[cursor..cursor + next]);
            cursor += next;
            continue;
        }

        match bytes.get(cursor + 1) {
            Some(b'%') => {
                out.push('%');
                cursor += 2;
            }
            Some(digit) if digit.is_ascii_digit() => {
                let digits = bytes[cursor + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
                let number: usize =
                    translation[cursor + 1..cursor + 1 + digits].parse().unwrap_or(0);

                match number.checked_sub(1).and_then(|index| protected.get(index).zip(Some(index)))
                {
                    Some((text, index)) => {
                        out.push_str(text);
                        used[index] = true;
                    }
                    // A placeholder the source never had. Left as written, so it
                    // shows up on the slide rather than vanishing into markup.
                    None => out.push_str(&translation[cursor..cursor + 1 + digits]),
                }
                cursor += 1 + digits;
            }
            _ => {
                out.push('%');
                cursor += 1;
            }
        }
    }

    match used.iter().position(|kept| !kept) {
        Some(missing) => Err(missing + 1),
        None => Ok(out),
    }
}

/// One protected region, as a byte range in the source.
struct Region {
    start: usize,
    end: usize,
}

/// The next region that must not be translated, at or after `from`.
///
/// Found by looking for each opener and taking whichever comes first, rather
/// than by scanning byte by byte through a state machine: the openers do not
/// nest in Markdown, and one pass per kind is easier to be sure is right than
/// one pass that is trying to be four things.
fn next_region(source: &str, from: usize) -> Option<Region> {
    [code_span(source, from), tag(source, from), mark(source, from), destination(source, from)]
        .into_iter()
        .flatten()
        .min_by_key(|region| region.start)
}

/// An inline code span, from its opening run of backticks to a matching one.
fn code_span(source: &str, from: usize) -> Option<Region> {
    let open = from + source[from..].find('`')?;
    let ticks = source[open..].chars().take_while(|&c| c == '`').count();
    let fence = "`".repeat(ticks);
    let after = open + ticks;

    // An unclosed span is the author's problem and half of one exists constantly
    // while somebody types. Protect to the end of the run rather than to the end
    // of the line, so the words after it stay translatable.
    let close = source[after..].find(&fence).map(|at| after + at + ticks).unwrap_or(after);

    Some(Region { start: open, end: close })
}

/// An HTML tag or comment, including a step marker.
fn tag(source: &str, from: usize) -> Option<Region> {
    let open = from + source[from..].find('<')?;
    let rest = &source[open..];

    let close = if rest.starts_with("<!--") {
        rest.find("-->").map(|at| at + 3)
    } else {
        rest.find('>').map(|at| at + 1)
    };

    Some(Region { start: open, end: open + close.unwrap_or(1) })
}

/// A mark's attribute list: the `{…}` and nothing else.
///
/// The marked text stays in the segment, because `[3.2x faster]{.accent}` is a
/// phrase somebody has to translate. The braces are the part that addresses.
fn mark(source: &str, from: usize) -> Option<Region> {
    let found = find_marks(&source[from..]).into_iter().next()?;
    let span = found.attributes_span();

    Some(Region { start: from + span.start, end: from + span.end })
}

/// A link or image destination: the `(…)` after a `](`.
fn destination(source: &str, from: usize) -> Option<Region> {
    let mut cursor = from;

    while let Some(at) = source[cursor..].find("](").map(|at| cursor + at) {
        let open = at + 1;
        match source[open..].find(')').map(|end| open + end + 1) {
            Some(close) => return Some(Region { start: open, end: close }),
            // An unclosed destination is not one yet. Keep looking, so a
            // half-typed link does not protect the rest of the slide.
            None => cursor = open,
        }
    }

    // A bare URL that is nobody's destination. Ends at whitespace, because a
    // sentence continues after it and a full stop is usually punctuation rather
    // than part of the address.
    let start = ["https://", "http://"]
        .into_iter()
        .filter_map(|scheme| source[from..].find(scheme).map(|at| from + at))
        .min()?;
    let end =
        source[start..].find(char::is_whitespace).map(|at| start + at).unwrap_or(source.len());

    Some(Region { start, end: start + source[start..end].trim_end_matches(['.', ',', ')']).len() })
}

/// Appends prose, escaping only what would read back as a placeholder.
fn push_escaped(out: &mut String, text: &str) {
    let bytes = text.as_bytes();

    for (index, character) in text.char_indices() {
        if character == '%' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            out.push_str("%%");
        } else {
            out.push(character);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(source: &str) -> String {
        let masked = mask(source);
        restore(&masked.text, &masked.protected).expect("every placeholder kept")
    }

    #[test]
    fn a_mark_key_never_reaches_the_translator_and_its_text_does() {
        // The failure this whole crate exists for: a translated `#latency` is a
        // `steps:` entry pointing at nothing, and the deck still renders.
        let masked = mask("Latency dropped to [120ms]{#latency}[38ms]{#latency}.");

        assert_eq!(masked.text, "Latency dropped to [120ms]%1[38ms]%2.");
        assert_eq!(masked.protected, ["{#latency}", "{#latency}"]);
    }

    #[test]
    fn mark_classes_and_properties_are_protected_with_the_key() {
        let masked = mask("The result was [3.2x faster]{#result .accent color=success}.");

        assert_eq!(masked.text, "The result was [3.2x faster]%1.");
        assert_eq!(masked.protected, ["{#result .accent color=success}"]);
    }

    #[test]
    fn an_inline_code_span_is_an_identifier_rather_than_a_phrase() {
        let masked = mask("The `contrast/projector` rule catches it.");

        assert_eq!(masked.text, "The %1 rule catches it.");
    }

    #[test]
    fn a_link_keeps_its_words_and_loses_its_destination() {
        let masked = mask("See [the handbook](https://example.test/en/guide) for more.");

        assert_eq!(masked.text, "See [the handbook]%1 for more.");
        assert_eq!(masked.protected, ["(https://example.test/en/guide)"]);
    }

    #[test]
    fn an_images_alt_text_is_prose_and_its_path_is_not() {
        // A translated `./diagram.png` is a broken image and a build that still
        // succeeds.
        let masked = mask("![A flame graph](./diagram.png)");

        assert_eq!(masked.text, "![A flame graph]%1");
    }

    #[test]
    fn a_bare_url_is_protected_without_swallowing_the_sentence_around_it() {
        let masked = mask("Slides at https://example.test/talk after the session.");

        assert_eq!(masked.text, "Slides at %1 after the session.");
        assert_eq!(masked.protected, ["https://example.test/talk"]);
    }

    #[test]
    fn a_step_marker_is_a_position_in_a_pipeline_rather_than_words() {
        let masked = mask("- The venue Wi-Fi is down <!-- step -->");

        assert_eq!(masked.text, "- The venue Wi-Fi is down %1");
        assert_eq!(masked.protected, ["<!-- step -->"]);
    }

    #[test]
    fn an_html_tag_is_protected_and_the_text_inside_it_is_not() {
        let masked = mask("<strong>Never</strong> on stage");

        assert_eq!(masked.text, "%1Never%2 on stage");
    }

    #[test]
    fn a_percent_before_a_digit_is_escaped_so_it_cannot_read_as_a_placeholder() {
        assert_eq!(mask("%1 of the time").text, "%%1 of the time");
        assert_eq!(round_trip("%1 of the time"), "%1 of the time");
    }

    #[test]
    fn an_ordinary_percent_is_left_alone_because_prose_is_full_of_them() {
        assert_eq!(mask("30% faster").text, "30% faster");
    }

    #[test]
    fn masking_and_restoring_is_the_identity_on_every_shape_at_once() {
        for source in [
            "Latency dropped to [120ms]{#latency}[38ms]{#latency}.",
            "The `retry` policy, see [the docs](./retry.md) <!-- step -->",
            "![alt](./a.png) and ![alt](./b.png)",
            "plain words with no markup at all",
            "",
        ] {
            assert_eq!(round_trip(source), source, "{source}");
        }
    }

    #[test]
    fn a_translation_may_move_a_placeholder_because_word_order_does() {
        // Japanese puts the verb last. A scheme that could not reorder would be
        // a scheme that produced ungrammatical Japanese.
        let masked = mask("Latency dropped to [120ms]{#latency}.");
        let translated = restore("[120ms]%1になりました。", &masked.protected).unwrap();

        assert_eq!(translated, "[120ms]{#latency}になりました。");
    }

    #[test]
    fn a_translation_that_drops_a_placeholder_is_refused_rather_than_applied() {
        // Applying it would produce a slide with the animation silently gone,
        // which is exactly the failure that is impossible to notice.
        let masked = mask("Latency dropped to [120ms]{#latency}.");

        assert_eq!(restore("レイテンシが下がりました。", &masked.protected), Err(1));
    }

    #[test]
    fn a_placeholder_the_source_never_had_is_left_as_written() {
        // It shows up on the slide, where somebody sees it, rather than
        // disappearing into markup where nobody does.
        assert_eq!(restore("a %7 b", &[]), Ok("a %7 b".to_string()));
    }

    #[test]
    fn a_line_that_is_only_markup_has_no_words_to_translate() {
        assert!(!mask("![](./diagram.png)").has_words());
        assert!(!mask("| --- | --- |").has_words());
        assert!(mask("| `rule` | what it catches |").has_words());
    }

    #[test]
    fn an_unterminated_code_span_does_not_protect_the_rest_of_the_line() {
        // Half a code span exists constantly while somebody is typing, and it
        // must not hide the words after it from a translator.
        assert!(mask("The `retry policy matters").has_words());
    }
}
