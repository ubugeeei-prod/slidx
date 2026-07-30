//! Non-fatal problems found while reading a deck.
//!
//! Parsing a deck never fails. A talk is often edited minutes before it starts,
//! and a tool that refuses to render because of one bad line is a tool that
//! fails at the worst possible moment. Every problem is collected here instead,
//! surfaced in the editor and the terminal, and left to the author to fix.

use serde::{Deserialize, Serialize};

/// How much a diagnostic should interrupt the author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Worth knowing, safe to ignore.
    Info,
    /// The deck renders, but not the way the author probably meant.
    Warning,
    /// Content was dropped or could not be understood.
    Error,
}

impl Severity {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Where in the source a diagnostic points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpan {
    /// One-based, to match what editors display.
    pub line: u32,
    pub slide_index: Option<u32>,
}

impl SourceSpan {
    pub fn line(line: u32) -> Self {
        Self { line, slide_index: None }
    }

    pub fn on_slide(mut self, slide_index: u32) -> Self {
        self.slide_index = Some(slide_index);
        self
    }
}

/// One problem, addressed to the person who wrote the deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Stable identifier such as `frontmatter/invalid-yaml`, used for
    /// suppression rules and for linking to documentation.
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: SourceSpan,
    /// A concrete next action, when there is an obvious one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(code: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            span: SourceSpan::default(),
            help: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, message)
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, message)
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Info, message)
    }

    pub fn at(mut self, span: SourceSpan) -> Self {
        self.span = span;
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// True when this diagnostic should fail a strict build.
    pub fn is_blocking(&self) -> bool {
        self.severity == Severity::Error
    }

    /// True when one of `allow` suppresses this diagnostic.
    ///
    /// A code is a path: the part before the slash names the group. So `theme`
    /// suppresses `theme/unknown-transition` and `theme/unknown-layout` alike,
    /// while `them` suppresses nothing — a prefix that is not a group boundary
    /// is not a group.
    ///
    /// This lives with the code rather than with any one checker because there
    /// is more than one. The visual linter, the dialect check and the parser all
    /// produce diagnostics, and `--allow` has to mean the same thing for all
    /// three or an author learns a rule that holds for two of them.
    pub fn is_suppressed_by(&self, allow: &[String]) -> bool {
        allow.iter().any(|allowed| {
            self.code == *allowed
                || self
                    .code
                    .strip_prefix(allowed.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }
}

/// A growable diagnostic list with the ergonomics the parsers want.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.0.push(diagnostic);
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = Diagnostic>) {
        self.0.extend(other);
    }

    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.0.iter()
    }

    /// Highest severity present, if any.
    pub fn max_severity(&self) -> Option<Severity> {
        self.0.iter().map(|diagnostic| diagnostic.severity).max()
    }

    /// True when a strict build should stop.
    pub fn has_blocking(&self) -> bool {
        self.0.iter().any(Diagnostic::is_blocking)
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<Diagnostic> for Diagnostics {
    fn from_iter<T: IntoIterator<Item = Diagnostic>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_from_least_to_most_urgent() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn max_severity_reports_the_worst_problem() {
        let diagnostics: Diagnostics = vec![
            Diagnostic::info("a", "a"),
            Diagnostic::error("b", "b"),
            Diagnostic::warning("c", "c"),
        ]
        .into_iter()
        .collect();

        assert_eq!(diagnostics.max_severity(), Some(Severity::Error));
        assert!(diagnostics.has_blocking());
    }

    #[test]
    fn warnings_alone_do_not_block_a_build() {
        let diagnostics: Diagnostics = vec![Diagnostic::warning("a", "a")].into_iter().collect();

        assert!(!diagnostics.has_blocking());
    }

    #[test]
    fn an_empty_list_has_no_severity() {
        assert_eq!(Diagnostics::default().max_severity(), None);
        assert!(Diagnostics::default().is_empty());
    }

    #[test]
    fn spans_carry_an_optional_slide_index() {
        let span = SourceSpan::line(12).on_slide(3);
        assert_eq!(span.line, 12);
        assert_eq!(span.slide_index, Some(3));
    }

    #[test]
    fn an_exact_code_is_suppressed_and_so_is_its_group() {
        let diagnostic = Diagnostic::warning("theme/unknown-layout", "m");

        assert!(diagnostic.is_suppressed_by(&["theme/unknown-layout".to_string()]));
        assert!(diagnostic.is_suppressed_by(&["theme".to_string()]));
        assert!(diagnostic.is_suppressed_by(&["a".to_string(), "theme".to_string()]));
    }

    #[test]
    fn a_prefix_that_is_not_a_group_boundary_suppresses_nothing() {
        // `them` must not swallow `theme/*`, and `theme/unknown` must not
        // swallow `theme/unknown-layout`.
        let diagnostic = Diagnostic::warning("theme/unknown-layout", "m");

        assert!(!diagnostic.is_suppressed_by(&["them".to_string()]));
        assert!(!diagnostic.is_suppressed_by(&["theme/unknown".to_string()]));
        assert!(!diagnostic.is_suppressed_by(&[]));
    }

    #[test]
    fn help_text_is_optional_and_omitted_from_json() {
        let json = serde_json::to_string(&Diagnostic::info("a", "b")).unwrap();
        assert!(!json.contains("help"));
    }
}
