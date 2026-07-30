//! The TypeScript form of everything that crosses into JavaScript.
//!
//! # Why the declarations are generated
//!
//! The deck arrives in JavaScript as JSON, and JSON carries no types. Every
//! consumer — the Vite plugin, the runtime, the visual editor — therefore has
//! to be *told* the shape, and until this module existed each of them was told
//! by hand. A hand-written declaration is not wrong on the day it is written;
//! it is wrong on the day someone adds a field in Rust, which is a day nobody
//! can predict and nothing announces.
//!
//! So the declarations come from the Rust types themselves, via `ts-rs`, whose
//! derive reads the `#[serde(…)]` attributes it sits next to. That matters more
//! than it sounds: the wire format is what serde emits, not what the struct
//! declares, and a generator that ignored `rename_all` or `skip_serializing_if`
//! would produce a confident description of a payload that never existed.
//!
//! Where a `#[ts(…)]` attribute does appear it is saying something sharper than
//! `ts-rs` would infer — a field serde omits is `field?: T`, not
//! `field?: T | null`, because a `null` is a case every consumer would then have
//! to handle and none would ever see. Two vocabularies on one field is a place
//! drift could hide, so it does not get to be a matter of care:
//! [`a_field_the_declaration_requires_is_one_serde_always_writes`] serialises
//! each type and checks the declaration against what actually came out.
//!
//! # Why the output is committed
//!
//! `deck.d.ts` sits next to this file in git rather than being produced during
//! a build. A generated file that only exists in `target/` makes a change to
//! the boundary invisible in review: the diff shows a new Rust field and says
//! nothing about the TypeScript every consumer now compiles against. Committed,
//! the contract change *is* the diff.
//!
//! The cost of committing is that the file can go stale, so it is not allowed
//! to: [`the_committed_declarations_are_what_the_rust_types_generate`] fails
//! when it does, and that test runs in `test:rust`, which CI already runs.
//! `vp run generate:types` writes the file.
//!
//! # How it reaches npm
//!
//! `wasm-bindgen` appends the file verbatim to the generated `slidx.d.ts`, so
//! `@slidx/wasm` ships one self-contained declaration file and a consumer needs
//! no second package to describe what `buildDeck` returned.

use ts_rs::{Config, TS};

use slidx_core::{
    AutoSteps, Easing, Effect, EffectKind, EffectPreset, ElementState, Origin, StepFrame, StepGrid,
    StepKind, StepPlacement, StepRow, StepTimeline, Visibility,
};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::summary::DeckSummary;
use crate::{AssetSize, BuildOptions, BuildResult, BuiltSlide, Finding, SnippetFile};

// Appended verbatim to the `.d.ts` wasm-bindgen writes, which is how the types
// reach npm without a second artifact to keep in step.
#[wasm_bindgen(typescript_custom_section)]
const DECK_TYPES: &str = include_str!("../deck.d.ts");

const HEADER: &str = "\
// The slidx deck, as it crosses out of Rust.
//
// Generated from the Rust types by `vp run generate:types`. Editing this file
// is pointless: `cargo test -p slidx_wasm` compares it against the types it
// came from, and the types win.
";

/// The one declaration written by hand, because it is a *use* of a generated
/// type rather than a second description of one.
///
/// `Partial<BuildOptions>` would be the obvious spelling and is subtly wrong
/// under `exactOptionalPropertyTypes`, which the repository turns on: it lets a
/// key be absent but not present-and-`undefined`, and `undefined` is exactly
/// what an unset option looks like at a call site. serde reads both as absent.
const FOOTER: &str = "
/**
 * What `buildDeck` accepts.
 *
 * Every field may be left out — the Rust struct is `#[serde(default)]` — or
 * passed explicitly as `undefined`, which means the same thing.
 */
export type BuildDeckOptions = { [K in keyof BuildOptions]?: BuildOptions[K] | undefined };
";

/// Every declaration, in one file, in a stable order.
///
/// The list is written out rather than walked from the roots so that adding a
/// type produces a one-line diff where a reviewer expects it, instead of
/// reshuffling the file around whatever order a dependency walk arrived in.
pub fn generate() -> String {
    let cfg = Config::default();
    let mut file = String::from(HEADER);

    // The call boundary: what `buildDeck` takes and what it gives back.
    push::<BuildOptions>(&mut file, &cfg);
    push::<AssetSize>(&mut file, &cfg);
    push::<BuildResult>(&mut file, &cfg);
    push::<BuiltSlide>(&mut file, &cfg);
    push::<Finding>(&mut file, &cfg);
    push::<SnippetFile>(&mut file, &cfg);

    // What `deckSummary` gives back. It never travels with a build — the
    // editor's history panel asks for it on its own — so it would otherwise
    // drift on a schedule of its own.
    push::<DeckSummary>(&mut file, &cfg);

    // The step timeline, which the renderer embeds as JSON in the page and the
    // client runtime parses back out. It crosses without going through
    // `buildDeck` at all, so left out it would drift on its own schedule.
    push::<StepTimeline>(&mut file, &cfg);
    push::<StepFrame>(&mut file, &cfg);
    push::<ElementState>(&mut file, &cfg);
    push::<Effect>(&mut file, &cfg);
    push::<EffectKind>(&mut file, &cfg);
    push::<EffectPreset>(&mut file, &cfg);
    push::<Easing>(&mut file, &cfg);
    push::<Origin>(&mut file, &cfg);
    push::<Visibility>(&mut file, &cfg);

    // The same slide's steps seen from the authoring side rather than the
    // presenting one: what the editor's timeline draws rows and columns from.
    push::<StepGrid>(&mut file, &cfg);
    push::<StepRow>(&mut file, &cfg);
    push::<StepPlacement>(&mut file, &cfg);
    push::<StepKind>(&mut file, &cfg);
    push::<AutoSteps>(&mut file, &cfg);

    file.push_str(FOOTER);
    file
}

/// One type's JSDoc and declaration, appended.
///
/// The Rust doc comments come across as JSDoc: the reasoning behind a field is
/// as load-bearing on the TypeScript side as it is in the crate, and a
/// declaration stripped of it would read as a shape with no argument for why.
pub(crate) fn push<T: TS + ?Sized>(file: &mut String, cfg: &Config) {
    file.push('\n');
    if let Some(docs) = T::docs() {
        file.push_str(docs.trim_start_matches('\n'));
    }
    file.push_str("export ");
    file.push_str(&T::decl(cfg));
    file.push('\n');
}

/// The whole point of the exercise: a field added in Rust and forgotten in
/// TypeScript fails a test rather than reaching a consumer as `any`.
///
/// Writing the file is folded into the check rather than living in its own
/// binary so there is exactly one description of what belongs in it.
/// `vp run generate:types` sets the variable, and a person reading the failure
/// is told that command rather than left to find it.
///
/// Shared by every declaration file this crate commits, so a second boundary —
/// publishing is the first — inherits the check rather than restating it.
#[cfg(test)]
pub(crate) fn check_committed(name: &str, committed: &str, generated: &str) {
    if std::env::var_os("SLIDX_WRITE_DECK_TYPES").is_some() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name);
        std::fs::write(path, generated).expect("write the declarations");
        return;
    }

    let committed = significant_tokens(committed);
    let regenerated = significant_tokens(generated);

    assert!(
        committed == regenerated,
        "crates/slidx_wasm/{name} no longer describes the Rust types. \
         Run `vp run generate:types`.\n{}",
        first_difference(&committed, &regenerated),
    );
}

/// A declaration reduced to the tokens that carry meaning.
///
/// The committed copy is run through the repository's formatter after it is
/// generated, and everything a formatter changes here is punctuation that
/// separates or groups rather than names: it breaks lines, writes `;` between
/// members where `ts-rs` writes `,`, leads a long union with a `|`, brackets a
/// union member that is an intersection, and drops the quotes `ts-rs` puts
/// round an object key that did not need them. Those are dropped.
///
/// Nothing else is. Names, types, literals, and their order all survive, so a
/// field added in Rust and forgotten in TypeScript still changes this stream —
/// which is the whole thing the check has to catch.
#[cfg(test)]
fn significant_tokens(declarations: &str) -> Vec<String> {
    declarations
        .split_whitespace()
        .map(|token| token.replace([',', ';', '|', '(', ')', '"'], ""))
        .filter(|token| !token.is_empty())
        .collect()
}

/// Where two token streams first diverge, with enough either side to recognise.
/// Printing both in full would bury the one changed field in a page of things
/// that did not change.
#[cfg(test)]
fn first_difference(committed: &[String], regenerated: &[String]) -> String {
    let at = committed
        .iter()
        .zip(regenerated)
        .position(|(one, other)| one != other)
        .unwrap_or_else(|| committed.len().min(regenerated.len()));

    let window = |tokens: &[String]| {
        let start = at.saturating_sub(6).min(tokens.len());
        tokens[start..(at + 6).min(tokens.len())].join(" ")
    };

    format!("  committed: …{}…\n  generated: …{}…", window(committed), window(regenerated))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;

    use serde::Serialize;

    /// The declarations as they were last generated and committed.
    const COMMITTED: &str = include_str!("../deck.d.ts");

    #[test]
    fn the_committed_declarations_are_what_the_rust_types_generate() {
        check_committed("deck.d.ts", COMMITTED, &generate());
    }

    /// Field names that appear in the declaration but never in the payload —
    /// or the other way round — are the failure this whole module exists to
    /// make impossible, and `ts-rs` reading the `#[serde(…)]` attributes is
    /// the reason it holds. This checks that reading against the real thing.
    #[test]
    fn a_field_the_declaration_requires_is_one_serde_always_writes() {
        for (name, value) in minimal_payloads() {
            assert_eq!(
                required_fields(&generate(), name),
                serialized_keys(&value),
                "{name}: the declaration and serde disagree about which keys are always present",
            );
        }
    }

    /// The check above is only as good as the parser under it. An inline mapped
    /// type ends in a `};` of its own, so a scan that stopped at the first one
    /// would silently miss every field declared after it, and the eventual
    /// failure would blame serde rather than the parser that never looked.
    #[test]
    fn a_field_after_an_inline_object_is_still_scanned() {
        let declarations = concat!(
            "export type Thing = {\n",
            "  properties?: { [key in string]: string };\n",
            "  after: string;\n",
            "};\n",
            "\n",
            "export type Other = { outside: string };\n",
        );

        assert_eq!(required_fields(declarations, "Thing"), BTreeSet::from(["after".to_owned()]));
    }

    /// The least serde ever writes for each type: every option empty, every
    /// collection empty. What survives that is what a consumer can rely on.
    fn minimal_payloads() -> Vec<(&'static str, serde_json::Value)> {
        let finding = Finding {
            severity: "warning".into(),
            code: "a/rule".into(),
            message: "something".into(),
            help: None,
            slide_index: None,
        };
        let slide = BuiltSlide {
            id: "one".into(),
            index: 0,
            title: None,
            notes: Vec::new(),
            stop_count: 1,
            steps: slidx_core::StepGrid::default(),
            budget_seconds: None,
            estimated_seconds: 0,
            optional: false,
            frontmatter: serde_json::Value::Null,
            html: None,
            og_svg: None,
            presenter_html: None,
        };
        let result = BuildResult {
            title: None,
            description: None,
            duration_seconds: None,
            slides: Vec::new(),
            diagnostics: Vec::new(),
            has_blocking: false,
            print_html: None,
            og_svg: None,
            snippets: Vec::new(),
            sitemap: None,
            robots: None,
        };
        let state = ElementState {
            target: "[data-slidx-anchor]".into(),
            visibility: Visibility::Hidden,
            content: None,
            properties: Default::default(),
            effect: None,
        };

        let summary =
            DeckSummary { first: false, slides: 0, subject: String::new(), changes: Vec::new() };

        vec![
            ("BuildOptions", json(&BuildOptions::default())),
            ("BuildResult", json(&result)),
            ("DeckSummary", json(&summary)),
            ("BuiltSlide", json(&slide)),
            ("Finding", json(&finding)),
            ("StepTimeline", json(&StepTimeline::default())),
            ("StepFrame", json(&StepFrame::default())),
            ("ElementState", json(&state)),
            ("Effect", json(&Effect::default())),
            ("StepGrid", json(&StepGrid::default())),
        ]
    }

    fn json<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).expect("serialize")
    }

    fn serialized_keys(value: &serde_json::Value) -> BTreeSet<String> {
        value.as_object().expect("an object").keys().cloned().collect()
    }

    /// Fields of one declaration that are not marked `?`.
    ///
    /// Reads the generated text rather than the Rust types on purpose: the
    /// declaration is what a consumer compiles against, so it is the thing
    /// worth checking.
    fn required_fields(declarations: &str, name: &str) -> BTreeSet<String> {
        let start = declarations.find(&format!("export type {name} = {{")).expect("declared");
        // Prose first: a doc comment reading "off by default: …" is otherwise
        // indistinguishable from a field called `default`, and a brace in prose
        // is otherwise indistinguishable from the type's own.
        let body = without_doc_comments(&declarations[start..]);
        // The type ends at the brace matching its own opening one, not at the
        // first `};`: an inline mapped type like `{ [key in string]: string };`
        // contains that, and stopping there hides every field after it.
        let end = closing_brace(&body).expect("terminated");
        let body = &body[..end];

        let mut required = BTreeSet::new();
        let mut depth = 0usize;
        let mut field: Option<String> = None;

        for token in body.split_whitespace() {
            // Only the outermost braces are this type's own fields; a nested
            // `{ [key in string]: string }` is one field, not two.
            depth += token.matches('{').count();
            depth -= token.matches('}').count().min(depth);

            if depth == 1 {
                if let Some(colon) = token.strip_suffix(':') {
                    field = Some(colon.to_owned());
                    continue;
                }
            }
            if let Some(previous) = field.take() {
                if !previous.ends_with('?') {
                    required.insert(previous);
                }
            }
        }

        required
    }

    /// One past the brace that closes the first `{` in `body`.
    fn closing_brace(body: &str) -> Option<usize> {
        let mut depth = 0usize;

        for (index, character) in body.char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn without_doc_comments(body: &str) -> String {
        let mut out = String::with_capacity(body.len());
        let mut rest = body;

        while let Some(open) = rest.find("/**") {
            out.push_str(&rest[..open]);
            rest = match rest[open..].find("*/") {
                Some(close) => &rest[open + close + 2..],
                None => "",
            };
        }
        out.push_str(rest);
        out
    }

    #[test]
    fn field_names_are_the_camel_case_ones_serde_emits() {
        let generated = generate();

        assert!(generated.contains("stopCount"));
        assert!(!generated.contains("stop_count"));
    }

    #[test]
    fn enum_members_are_the_tokens_the_runtimes_css_is_keyed_on() {
        // The preset names are class-name fragments in the runtime stylesheet.
        // A rename that arrived in TypeScript as `FlyIn` would stop matching a
        // keyframe set, and the failure is an element that never appears — on
        // stage, in front of an audience.
        assert!(generate().contains("\"fly-in\""));
    }
}
