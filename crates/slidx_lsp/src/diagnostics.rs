//! slidx findings, as the editor states them.
//!
//! This is the feature that pays for the crate. Contrast and font-size
//! failures are checked at `vite build` today, which is after the author has
//! stopped looking at the slide that caused them — and a projector-washout
//! warning is only useful while the colour is still under the cursor.
//!
//! Two decisions worth stating.
//!
//! **The remedy is part of the message.** A [`slidx_core::Diagnostic`] carries
//! `help` — a concrete next action — and an editor shows one string. Dropping
//! the remedy would leave the author with the half of the diagnostic they
//! cannot act on.
//!
//! **A span with no line still lands somewhere.** Many findings are addressed
//! to a slide rather than to a line, because the thing that is wrong is the
//! slide. Publishing those at line one would pile every finding in a deck on
//! top of the frontmatter, so they are resolved to the first line of content
//! on the slide they name.

use serde::{Deserialize, Serialize};
use slidx_core::{Diagnostic, Severity, SourceSpan};

use crate::analysis::Analysis;
use crate::position::{LineIndex, PositionEncoding, Range};
use crate::DIAGNOSTIC_SOURCE;

/// LSP severities, which are numbers on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", from = "u8")]
pub enum LspSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl From<LspSeverity> for u8 {
    fn from(severity: LspSeverity) -> Self {
        match severity {
            LspSeverity::Error => 1,
            LspSeverity::Warning => 2,
            LspSeverity::Information => 3,
            LspSeverity::Hint => 4,
        }
    }
}

impl From<u8> for LspSeverity {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            4 => Self::Hint,
            _ => Self::Information,
        }
    }
}

impl From<Severity> for LspSeverity {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Error => Self::Error,
            Severity::Warning => Self::Warning,
            Severity::Info => Self::Information,
        }
    }
}

/// One finding, in the shape `textDocument/publishDiagnostics` wants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub range: Range,
    pub severity: LspSeverity,
    /// The stable slidx code, so an author can suppress it by name.
    pub code: String,
    pub source: String,
    pub message: String,
}

/// Converts every finding in an analysis.
pub fn publish(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    encoding: PositionEncoding,
) -> Vec<LspDiagnostic> {
    analysis
        .findings
        .iter()
        .map(|finding| LspDiagnostic {
            range: resolve(analysis, text, index, finding.span, encoding),
            severity: finding.severity.into(),
            code: finding.code.clone(),
            source: DIAGNOSTIC_SOURCE.to_string(),
            message: message(finding),
        })
        .collect()
}

/// Where a span points, in the editor's coordinates.
///
/// Three cases, in the order a span narrows:
///
/// 1. A line. Underline it.
/// 2. A slide. Underline the first line on that slide with anything on it.
/// 3. Neither, which means the deck itself. Underline the deck's frontmatter,
///    because that is the only thing a deck-wide finding can be about.
fn resolve(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    span: SourceSpan,
    encoding: PositionEncoding,
) -> Range {
    if span.line > 0 {
        return index.line_range(text, span.line, encoding);
    }

    let Some(at) = span.slide_index else {
        return match analysis.frontmatter.first().filter(|block| block.is_deck()) {
            Some(block) => index.lines_range(text, block.lines.first, block.lines.last, encoding),
            None => index.line_range(text, 1, encoding),
        };
    };

    index.line_range(text, analysis.content_line(at as usize, text, index), encoding)
}

/// The finding and its remedy, as one string.
///
/// Indented under the message the way the build report writes it, so an author
/// who has seen one recognises the other.
fn message(finding: &Diagnostic) -> String {
    match &finding.help {
        Some(help) => format!("{}\n  {help}", finding.message),
        None => finding.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;

    fn run(source: &str) -> Vec<LspDiagnostic> {
        let analysis = analyze(source);
        publish(&analysis, source, &LineIndex::new(source), PositionEncoding::Utf16)
    }

    fn find<'a>(found: &'a [LspDiagnostic], code: &str) -> &'a LspDiagnostic {
        found.iter().find(|d| d.code == code).unwrap_or_else(|| panic!("no {code} in {found:?}"))
    }

    #[test]
    fn a_parse_diagnostic_lands_on_the_line_it_names() {
        let source = "---\ntitle: [oops\n---\n\n# One\n";
        let found = run(source);
        let broken = find(&found, "frontmatter/invalid-yaml");

        assert_eq!(broken.range.start.line, 1, "line two, zero-based");
        assert_eq!(broken.severity, LspSeverity::Error);
        assert_eq!(broken.source, "slidx");
    }

    #[test]
    fn the_remedy_travels_with_the_message() {
        // Half a diagnostic is the half the author cannot act on.
        let found = run("---\ntitle: [oops\n---\n\n# One\n");

        assert!(find(&found, "frontmatter/invalid-yaml").message.contains("unbalanced quotes"));
    }

    #[test]
    fn a_finding_without_a_remedy_is_just_the_message() {
        let found = run("# One\n\n![](./a.png)\n");
        let alt = find(&found, "structure/missing-alt");

        assert!(!alt.message.is_empty());
        assert_eq!(alt.message.lines().count(), 2, "this rule does carry a remedy");
    }

    #[test]
    fn a_slide_addressed_finding_lands_on_that_slide_rather_than_line_one() {
        // Every rule that names a slide instead of a line would otherwise
        // pile up on the frontmatter, sixty slides away from the problem.
        let source = "# One\n\n---\nsteps:\n  - reveal: \".x\"\n---\n\n- two <!-- step -->\n";
        let found = run(source);

        assert_eq!(find(&found, "steps/markers-ignored").range.start.line, 7, "the bullet");
    }

    #[test]
    fn a_slide_addressed_finding_skips_the_blank_lines_above_its_content() {
        let source = "# One\n\n---\nsteps:\n  - reveal: \".x\"\n---\n\n\n\n- two <!-- step -->\n";

        assert_eq!(find(&run(source), "steps/markers-ignored").range.start.line, 9);
    }

    #[test]
    fn a_finding_about_the_deck_itself_underlines_its_frontmatter() {
        // `aspect:` is wrong about the whole deck, so there is no line to
        // blame — but the block it was written in is right there.
        let source = "---\ntitle: T\naspect: 21x9\n---\n\n# One\n";
        let found = run(source);
        let aspect = find(&found, "deck/unknown-aspect");

        assert_eq!(aspect.range.start.line, 1, "the first key");
        assert_eq!(aspect.range.end.line, 2, "through the last");
    }

    #[test]
    fn severities_survive_the_trip_to_the_editor() {
        assert_eq!(LspSeverity::from(Severity::Error), LspSeverity::Error);
        assert_eq!(LspSeverity::from(Severity::Warning), LspSeverity::Warning);
        assert_eq!(LspSeverity::from(Severity::Info), LspSeverity::Information);
    }

    #[test]
    fn severities_are_numbers_on_the_wire() {
        let json = serde_json::to_string(&LspSeverity::Warning).unwrap();
        assert_eq!(json, "2");
    }

    #[test]
    fn a_diagnostic_on_a_japanese_line_underlines_all_of_it() {
        // The failure this crate exists to avoid: a column measured in bytes
        // puts every marker on this line in the wrong place.
        let source = "---\ntitle: 高速なデッキ\ntransition: 3\n---\n\n# 導入\n";
        let found = run(source);
        let broken = find(&found, "frontmatter/invalid-transition");

        assert_eq!(broken.range.start.line, 1, "the block's first key");
        assert_eq!(
            broken.range.end.character,
            PositionEncoding::Utf16.measure("transition: 3"),
            "and its last, measured in UTF-16 units rather than bytes",
        );
    }

    #[test]
    fn a_clean_deck_publishes_nothing() {
        assert!(run("---\ntitle: T\n---\n\n# One\n\n- a\n- b\n").is_empty());
    }

    #[test]
    fn a_code_is_published_so_an_author_can_suppress_it_by_name() {
        let found = run("# One\n\n![](./a.png)\n");
        assert!(found.iter().all(|d| d.code.contains('/')));
    }
}
