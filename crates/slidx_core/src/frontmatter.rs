//! YAML frontmatter, and the typed views onto it.
//!
//! Frontmatter is kept as JSON alongside the typed fields. Themes and plugins
//! read keys this crate has never heard of, and the visual editor writes them
//! back unchanged — so a deck never loses data by passing through a version of
//! slidx that predates the option it uses.

use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::diagnostic::{Diagnostic, Diagnostics, Severity, SourceSpan};
use crate::model::{AspectRatio, DeckMeta, TalkMeta};
use crate::steps::AutoSteps;

/// An empty JSON object, the shape callers expect when there is no frontmatter.
pub fn empty() -> JsonValue {
    JsonValue::Object(JsonMap::new())
}

/// Parses a YAML frontmatter block.
///
/// Returns an empty object plus a diagnostic when the block is malformed. A
/// deck being edited minutes before a talk must still render.
pub fn parse(source: &str, line: u32, diagnostics: &mut Diagnostics) -> JsonValue {
    if source.trim().is_empty() {
        return empty();
    }

    match serde_yaml::from_str::<serde_yaml::Value>(source) {
        Ok(value) => match serde_json::to_value(value) {
            Ok(JsonValue::Object(map)) => JsonValue::Object(map),
            Ok(JsonValue::Null) => empty(),
            Ok(_) => {
                diagnostics.push(
                    Diagnostic::new(
                        "frontmatter/not-a-mapping",
                        Severity::Warning,
                        "frontmatter must be a mapping of keys to values",
                    )
                    .at(SourceSpan::line(line))
                    .with_help("write `title: My Talk` rather than a bare value or list"),
                );
                empty()
            }
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error("frontmatter/unrepresentable", error.to_string())
                        .at(SourceSpan::line(line)),
                );
                empty()
            }
        },
        Err(error) => {
            diagnostics.push(
                Diagnostic::error("frontmatter/invalid-yaml", error.to_string())
                    .at(SourceSpan::line(line))
                    .with_help("check for unbalanced quotes or brackets"),
            );
            empty()
        }
    }
}

/// Reads a string field, accepting both `camelCase` and `kebab-case` spellings.
///
/// Authors reasonably write either, and rejecting one of them is the kind of
/// papercut that makes a tool feel hostile.
pub fn string(value: &JsonValue, key: &str) -> Option<String> {
    field(value, key).and_then(JsonValue::as_str).map(str::to_string)
}

/// Reads a boolean field.
pub fn boolean(value: &JsonValue, key: &str) -> Option<bool> {
    field(value, key).and_then(JsonValue::as_bool)
}

/// Finds a field under either spelling an author may have written.
///
/// The one place that knows `autoSteps` and `auto-steps` are the same key, so
/// every reader agrees. Public because asking *whether a key was written at
/// all* is a different question from reading its value, and the dialect check
/// needs it: a `duration:` nobody can parse is silently dropped, and the only
/// way to tell that from a deck that named no duration is to look for the key.
pub fn field<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    value.get(key).or_else(|| value.get(kebab_case(key)))
}

fn kebab_case(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 2);
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            out.push('-');
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

/// Reads a duration, accepting seconds, `"25m"`, `"25:00"`, or `"1h30m"`.
///
/// Slot lengths are quoted in minutes in every CFP, and typing `1500` for a
/// 25-minute talk is an invitation to get it wrong.
pub fn duration_seconds(value: &JsonValue, key: &str) -> Option<u32> {
    let field = field(value, key)?;

    if let Some(number) = field.as_u64() {
        return Some(number as u32);
    }

    parse_duration(field.as_str()?)
}

/// Parses a human duration into seconds.
pub fn parse_duration(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // `mm:ss` and `hh:mm:ss`
    if text.contains(':') {
        let mut total = 0u32;
        for part in text.split(':') {
            total = total.checked_mul(60)?.checked_add(part.trim().parse::<u32>().ok()?)?;
        }
        return Some(total);
    }

    // `1h30m20s`, `25m`, `90s`, or a bare number of seconds.
    let mut total = 0u32;
    let mut current = String::new();
    let mut matched_unit = false;

    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
            continue;
        }

        let amount = current.parse::<u32>().ok()?;
        current.clear();

        let multiplier = match character.to_ascii_lowercase() {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        matched_unit = true;
        total = total.checked_add(amount.checked_mul(multiplier)?)?;
    }

    if !current.is_empty() {
        // A trailing bare number means seconds, but only on its own.
        if matched_unit {
            return None;
        }
        return current.parse::<u32>().ok();
    }

    matched_unit.then_some(total)
}

/// Reads the `transition:` token, normalised.
///
/// The vocabulary itself is not checked here. Which transitions exist is a
/// theme's business — `slidx_theme::Transition::parse` owns the list and
/// reports a typo — exactly as `theme:` is kept as a name and resolved by
/// `slidx_theme::resolve`. This crate records what the author asked for; what
/// can be honoured is decided where the CSS is written.
///
/// `transition: false` is accepted as a spelling of `none` because YAML reads
/// it as a boolean, and [`string`] drops anything that is not a string. A
/// slide switching a deck-wide transition off would otherwise read as a slide
/// that said nothing at all, and inherit the very thing it was turning off —
/// a silent failure that looks like a transition bug rather than a spelling
/// one.
pub fn transition(value: &JsonValue, diagnostics: &mut Diagnostics) -> Option<String> {
    let field = field(value, "transition")?;

    if field == &JsonValue::Bool(false) {
        return Some("none".to_string());
    }

    match field.as_str().map(|text| text.trim().to_ascii_lowercase()) {
        Some(token) if !token.is_empty() => Some(token),
        _ => {
            diagnostics.push(
                Diagnostic::warning(
                    "frontmatter/invalid-transition",
                    "`transition` must name a transition",
                )
                .with_help("write `transition: fade`, or `transition: none` for an instant cut"),
            );
            None
        }
    }
}

/// Builds deck metadata from the first frontmatter block.
pub fn deck_meta(value: &JsonValue, diagnostics: &mut Diagnostics) -> DeckMeta {
    let aspect = match string(value, "aspect").or_else(|| string(value, "aspectRatio")) {
        Some(text) => AspectRatio::parse(&text).unwrap_or_else(|| {
            diagnostics.push(
                Diagnostic::warning(
                    "deck/unknown-aspect",
                    format!("unknown aspect ratio `{text}`"),
                )
                .with_help("use one of `16:9`, `16:10`, or `4:3`"),
            );
            AspectRatio::default()
        }),
        None => AspectRatio::default(),
    };

    DeckMeta {
        title: string(value, "title"),
        description: string(value, "description"),
        author: string(value, "author"),
        theme: string(value, "theme"),
        transition: transition(value, diagnostics),
        aspect,
        duration_seconds: duration_seconds(value, "duration"),
        talk: TalkMeta {
            event: string(value, "event"),
            date: string(value, "date"),
            venue: string(value, "venue"),
            hashtag: string(value, "hashtag").map(|tag| tag.trim_start_matches('#').to_string()),
            url: string(value, "url"),
            repo: string(value, "repo"),
        },
        raw: value.clone(),
    }
}

/// Reads the `autoSteps` mode.
///
/// The two levels of `Option` are load-bearing: the outer one says whether the
/// slide mentioned `autoSteps` at all, and the inner one says which mode it
/// chose. Without that distinction `autoSteps: none` could not switch off a
/// deck-wide default — the slide would look identical to one that never
/// mentioned the option.
pub fn auto_steps(value: &JsonValue, diagnostics: &mut Diagnostics) -> Option<Option<AutoSteps>> {
    let field = field(value, "autoSteps")?;

    if field == &JsonValue::Bool(false) {
        return Some(None);
    }

    let text = field.as_str()?;
    match text.trim() {
        "list" => Some(Some(AutoSteps::List)),
        "block" => Some(Some(AutoSteps::Block)),
        "row" => Some(Some(AutoSteps::Row)),
        "none" | "false" | "off" => Some(None),
        other => {
            diagnostics.push(
                Diagnostic::warning(
                    "slide/unknown-auto-steps",
                    format!("unknown autoSteps `{other}`"),
                )
                .with_help("use one of `list`, `block`, `row`, or `none`"),
            );
            Some(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_ok(source: &str) -> (JsonValue, Diagnostics) {
        let mut diagnostics = Diagnostics::default();
        let value = parse(source, 1, &mut diagnostics);
        (value, diagnostics)
    }

    #[test]
    fn a_valid_mapping_parses() {
        let (value, diagnostics) = parse_ok("title: Hello\ncount: 3");
        assert_eq!(value["title"], json!("Hello"));
        assert_eq!(value["count"], json!(3));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_empty_block_is_an_empty_object() {
        let (value, diagnostics) = parse_ok("   \n");
        assert_eq!(value, empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn broken_yaml_reports_an_error_and_yields_an_empty_object() {
        let (value, diagnostics) = parse_ok("title: [unclosed");
        assert_eq!(value, empty());
        assert_eq!(diagnostics.as_slice()[0].code, "frontmatter/invalid-yaml");
        assert_eq!(diagnostics.as_slice()[0].span.line, 1);
    }

    #[test]
    fn a_non_mapping_block_reports_a_warning() {
        let (_, diagnostics) = parse_ok("- a\n- b");
        assert_eq!(diagnostics.as_slice()[0].code, "frontmatter/not-a-mapping");
        assert!(!diagnostics.has_blocking(), "the deck still renders");
    }

    #[test]
    fn keys_can_be_written_in_camel_or_kebab_case() {
        let value = json!({ "auto-steps": "list" });
        assert_eq!(string(&value, "autoSteps").as_deref(), Some("list"));

        let value = json!({ "autoSteps": "block" });
        assert_eq!(string(&value, "autoSteps").as_deref(), Some("block"));
    }

    #[test]
    fn durations_accept_the_notations_cfps_actually_use() {
        assert_eq!(parse_duration("1500"), Some(1500));
        assert_eq!(parse_duration("25m"), Some(1500));
        assert_eq!(parse_duration("25:00"), Some(1500));
        assert_eq!(parse_duration("1h30m"), Some(5400));
        assert_eq!(parse_duration("1:00:00"), Some(3600));
        assert_eq!(parse_duration("90s"), Some(90));
    }

    #[test]
    fn nonsense_durations_are_rejected_rather_than_guessed() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("soon"), None);
        assert_eq!(parse_duration("25m30"), None);
    }

    #[test]
    fn duration_fields_accept_numbers_and_strings() {
        assert_eq!(duration_seconds(&json!({ "duration": 300 }), "duration"), Some(300));
        assert_eq!(duration_seconds(&json!({ "duration": "5m" }), "duration"), Some(300));
        assert_eq!(duration_seconds(&json!({}), "duration"), None);
    }

    #[test]
    fn deck_meta_reads_the_fields_a_talk_needs() {
        let mut diagnostics = Diagnostics::default();
        let meta = deck_meta(
            &json!({
                "title": "Fast Decks",
                "author": "ubugeeei",
                "duration": "20m",
                "aspect": "4:3",
                "event": "SlidxConf",
                "hashtag": "#slidxconf",
            }),
            &mut diagnostics,
        );

        assert_eq!(meta.title.as_deref(), Some("Fast Decks"));
        assert_eq!(meta.duration_seconds, Some(1200));
        assert_eq!(meta.aspect, AspectRatio::Classic);
        assert_eq!(meta.talk.hashtag.as_deref(), Some("slidxconf"), "the # is stripped once");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_unknown_aspect_warns_and_falls_back_to_wide() {
        let mut diagnostics = Diagnostics::default();
        let meta = deck_meta(&json!({ "aspect": "21:9" }), &mut diagnostics);

        assert_eq!(meta.aspect, AspectRatio::Wide);
        assert_eq!(diagnostics.as_slice()[0].code, "deck/unknown-aspect");
    }

    #[test]
    fn raw_frontmatter_is_preserved_for_themes_and_plugins() {
        let mut diagnostics = Diagnostics::default();
        let meta =
            deck_meta(&json!({ "title": "T", "themeOption": { "grid": true } }), &mut diagnostics);
        assert_eq!(meta.raw["themeOption"]["grid"], json!(true));
    }

    #[test]
    fn auto_steps_accepts_the_documented_modes() {
        let mut diagnostics = Diagnostics::default();
        assert_eq!(
            auto_steps(&json!({ "autoSteps": "list" }), &mut diagnostics),
            Some(Some(AutoSteps::List))
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_absent_option_is_distinguishable_from_an_explicit_none() {
        let mut diagnostics = Diagnostics::default();
        assert_eq!(auto_steps(&json!({}), &mut diagnostics), None, "not declared");
        assert_eq!(
            auto_steps(&json!({ "autoSteps": "none" }), &mut diagnostics),
            Some(None),
            "declared, and switched off"
        );
        assert_eq!(auto_steps(&json!({ "autoSteps": false }), &mut diagnostics), Some(None));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_transition_is_read_as_a_normalised_token() {
        let mut diagnostics = Diagnostics::default();
        assert_eq!(
            transition(&json!({ "transition": "  Fade " }), &mut diagnostics).as_deref(),
            Some("fade")
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_unrecognised_transition_reaches_the_theme_rather_than_being_dropped() {
        // Which transitions exist is the theme's list, not this crate's. A
        // token filtered out here would be a typo nothing could report.
        let mut diagnostics = Diagnostics::default();
        assert_eq!(
            transition(&json!({ "transition": "cube" }), &mut diagnostics).as_deref(),
            Some("cube")
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_absent_transition_is_distinguishable_from_one_switched_off() {
        let mut diagnostics = Diagnostics::default();
        assert_eq!(transition(&json!({}), &mut diagnostics), None, "not declared, so it inherits");
        assert_eq!(
            transition(&json!({ "transition": "none" }), &mut diagnostics).as_deref(),
            Some("none"),
            "declared, and switched off"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_transition_written_as_a_yaml_boolean_still_switches_it_off() {
        // `transition: false` reads as a bool, and a slide that lost it would
        // silently inherit the deck transition it was trying to turn off.
        let mut diagnostics = Diagnostics::default();
        assert_eq!(
            transition(&json!({ "transition": false }), &mut diagnostics).as_deref(),
            Some("none")
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_transition_that_is_not_a_name_is_reported() {
        for value in [json!({ "transition": 3 }), json!({ "transition": "" })] {
            let mut diagnostics = Diagnostics::default();
            assert_eq!(transition(&value, &mut diagnostics), None);
            assert_eq!(diagnostics.as_slice()[0].code, "frontmatter/invalid-transition");
            assert!(!diagnostics.has_blocking(), "the deck still renders");
        }
    }

    #[test]
    fn deck_meta_carries_the_default_transition() {
        let mut diagnostics = Diagnostics::default();
        let meta = deck_meta(&json!({ "transition": "Push" }), &mut diagnostics);

        assert_eq!(meta.transition.as_deref(), Some("push"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn an_unknown_mode_warns_and_switches_off() {
        let mut diagnostics = Diagnostics::default();
        assert_eq!(auto_steps(&json!({ "autoSteps": "wiggle" }), &mut diagnostics), Some(None));
        assert_eq!(diagnostics.as_slice()[0].code, "slide/unknown-auto-steps");
    }
}
