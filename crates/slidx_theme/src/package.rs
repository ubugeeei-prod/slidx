//! Themes somebody else published.
//!
//! # A theme package is data, not a stylesheet
//!
//! The obvious shape for a distributable theme is CSS, and it is the wrong one.
//! Every guarantee slidx makes about a deck is checkable because a theme
//! describes itself as tokens: the linter reads colour roles paired with the
//! background each is drawn on and the size each is drawn at, and the whole
//! contrast-through-projector-washout model depends on being handed *values*.
//! A stylesheet answers none of that. It cannot be told whether its text
//! clears 4.5:1 in a bright room, it cannot be asked what its safe area is,
//! and the rule that would check it would have to be a CSS engine.
//!
//! So a theme package ships the document [`Theme`] already serialises to — the
//! same palette, type scale, spacing and motion the four built-ins are, in
//! JSON. Nothing new to learn, nothing new for a rule to understand, and
//! [`crate::audit`] runs over a package exactly as it runs over a built-in
//! without knowing which it has.
//!
//! ```json
//! {
//!   "id": "workshop",
//!   "name": "Workshop",
//!   "description": "For a hands-on session.",
//!   "light": { "canvas": { "r": 232, "g": 236, "b": 237, "a": 1.0 }, "…": "…" },
//!   "dark": { "…": "…" },
//!   "scale": { "basePx": 32.0, "ratio": 1.2, "codeFactor": 1.0 },
//!   "spacing": { "paddingPx": 96.0, "blockPx": 28.0, "radiusPx": 0.0, "hairlinePx": 1.0 },
//!   "fontSans": "system-ui, sans-serif",
//!   "fontMono": "ui-monospace, monospace"
//! }
//! ```
//!
//! An npm package points at that file from its own manifest, under a key slidx
//! owns:
//!
//! ```json
//! { "name": "@slidx/theme-workshop", "slidx": { "theme": "./theme.json" } }
//! ```
//!
//! A manifest key rather than an `exports` subpath, because the document is not
//! a module: nothing imports it, a bundler never sees it, and the side that
//! reads it — the Vite plugin — has a filesystem and no reason to go through a
//! resolver to read one file it already knows the path of.
//!
//! # Finding one is the caller's job
//!
//! There is no filesystem on the side of the boundary the pipeline runs on, the
//! same constraint that makes the plugin read image headers and hand the sizes
//! back. So the plugin finds the documents and passes them here as text, and
//! this module decides whether each one is a theme. That split is what keeps
//! one answer: the editor's live preview and the production build harden and
//! audit the same bytes with the same code.
//!
//! # Precedence, and why it runs this way
//!
//! [`Catalogue::resolve`] tries the built-ins first, always, and a package
//! claiming a built-in id is refused rather than merged.
//!
//! The alternative — a package overriding a built-in — is the one that looks
//! reasonable. It is how plugin systems usually work, and it means
//! `theme: minimal` in a deck written last year renders as whatever a
//! dependency decided this morning. That is a supply-chain lever over every
//! deck in a repository, and it is silent: the deck asked for a name it still
//! gets, and nothing about the page says the answer changed.
//!
//! The other half of the same property is that a package *disappearing* must
//! not quietly change a deck either. `theme:` naming nothing resolvable was
//! already a reported warning rather than an absorbed one — `dialect/unknown-theme`
//! — and that stays true for a package name whose package is not installed.
//! Both directions come out of the same rule: a deck gets the theme it asked
//! for, or it is told it did not.
//!
//! # A published theme is untrusted input
//!
//! It arrives from a registry and is written into a page, so it goes through
//! [`guard`] before anything renders it, and through [`crate::audit`] before
//! anyone stands in front of it. Neither is advisory. See [`guard`] for what
//! the shell can and cannot defend on its own.

pub mod guard;

use slidx_core::{Diagnostic, Diagnostics, SourceSpan};
use slidx_lint::LintOptions;

use crate::theme::Theme;
use crate::{builtin, default_theme};

pub use guard::{Reason, Repair};

/// A theme document, and where the caller found it.
///
/// `source` is what a person has to go and look at — a package name, or a path
/// — and it is the only part of a finding they can act on. A diagnostic about a
/// theme with no origin in it sends an author looking through their own slides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub source: String,
    pub document: String,
}

impl Published {
    pub fn new(source: impl Into<String>, document: impl Into<String>) -> Self {
        Self { source: source.into(), document: document.into() }
    }
}

/// Every theme a deck may name.
///
/// The built-ins are not stored: they are a function, and holding a copy here
/// would be a second list to keep in step with [`builtin::all`].
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    installed: Vec<Installed>,
    diagnostics: Diagnostics,
}

/// One theme package, as it will actually be used.
#[derive(Debug, Clone)]
struct Installed {
    source: String,
    theme: Theme,
}

/// A theme id has to be a name a deck can write and a rule can print.
///
/// It reaches `theme:` in YAML, a `Surface` name in a diagnostic, and from
/// there a terminal — where a control character is not a cosmetic problem but
/// an escape sequence. Kebab-case ASCII is what every built-in already is.
fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Catalogue {
    /// Reads every document the caller found, hardening and auditing each.
    ///
    /// Never fails. A theme package that cannot be read is a diagnostic and a
    /// deck that still renders, for the same reason a bad line in a deck is:
    /// this runs while somebody is editing minutes before a talk.
    pub fn read(published: &[Published]) -> Self {
        let mut catalogue = Self::default();

        for candidate in published {
            catalogue.add(candidate);
        }

        catalogue
    }

    fn add(&mut self, published: &Published) {
        let Published { source, document } = published;

        let parsed: Theme = match serde_json::from_str(document) {
            Ok(theme) => theme,
            Err(error) => {
                self.reject(source, format!("is not a theme document: {error}"));
                return;
            }
        };

        if !is_id(&parsed.id) {
            self.reject(
                source,
                format!("declares `{}`, which is not a theme id", parsed.id.escape_debug()),
            );
            return;
        }

        if builtin::find(&parsed.id).is_some() {
            self.reject(source, format!("claims `{}`, which is a built-in theme", parsed.id));
            return;
        }

        if let Some(other) = self.installed.iter().find(|held| held.theme.id == parsed.id) {
            self.reject(
                source,
                format!("also declares `{}`, which {} already did", parsed.id, other.source),
            );
            return;
        }

        let held = guard::hold(parsed, &default_theme());
        for repair in &held.repairs {
            self.diagnostics.push(repaired(source, &held.theme.id, repair));
        }

        self.installed.push(Installed { source: source.clone(), theme: held.theme });
    }

    fn reject(&mut self, source: &str, what: String) {
        self.diagnostics.push(
            Diagnostic::error("theme/unreadable-package", format!("{source} {what}"))
                .at(SourceSpan::default().on_slide(0))
                .with_help(
                    "a theme package ships the token document `Theme` serialises to, \
                     named by `slidx.theme` in its own package.json",
                ),
        );
    }

    /// The theme a name resolves to, and where it came from.
    ///
    /// A built-in first, always. See this module's header for why a package may
    /// not take a built-in name.
    pub fn resolve(&self, id: &str) -> Option<Resolved> {
        if let Some(theme) = builtin::find(id) {
            return Some(Resolved { theme, source: None });
        }

        self.installed
            .iter()
            .find(|held| held.theme.id == id)
            .map(|held| Resolved { theme: held.theme.clone(), source: Some(held.source.clone()) })
    }

    /// Everything found wrong with the packages, whether or not one is used.
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Every name a deck may write, built-ins first.
    ///
    /// What the editor's picker and the language server's completion offer, so
    /// installing a theme makes it suggestable without either of them knowing
    /// what a package is.
    pub fn names(&self) -> Vec<String> {
        builtin::all()
            .into_iter()
            .map(|theme| theme.id)
            .chain(self.installed.iter().map(|held| held.theme.id.clone()))
            .collect()
    }

    /// Every installed theme, in the order the caller handed them over.
    pub fn installed(&self) -> impl Iterator<Item = (&str, &Theme)> {
        self.installed.iter().map(|held| (held.source.as_str(), &held.theme))
    }
}

/// A theme, and the package it came from if it was not built in.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub theme: Theme,
    /// `None` for a built-in.
    pub source: Option<String>,
}

impl Resolved {
    /// What the linter says about this theme, addressed to the deck using it.
    ///
    /// Empty for a built-in. Those are held to these rules by this crate's own
    /// test suite in every room slidx models, so running them again on every
    /// build would be arithmetic nobody reads. A package has no such gate: its
    /// author's CI is not this one, and the deck is where the failure lands.
    ///
    /// Only the theme actually resolved, not everything installed. A dependency
    /// a deck never names should not be able to fail its build.
    ///
    /// The rule's own code and message are kept. A comment colour that
    /// disappears in a bright room fails for the reason the contrast rule
    /// already explains, and re-coding it under a theme namespace would put one
    /// failure behind two names depending on where the colour came from. What
    /// is added is the origin — the one thing an author cannot work out from a
    /// finding about a colour they never wrote.
    pub fn audit(&self, options: &LintOptions) -> Diagnostics {
        let Some(source) = &self.source else {
            return Diagnostics::default();
        };

        crate::audit::audit(&self.theme, options)
            .iter()
            .map(|diagnostic| {
                Diagnostic::new(
                    diagnostic.code.clone(),
                    diagnostic.severity,
                    format!("theme `{}` from {source}: {}", self.theme.id, diagnostic.message),
                )
                .at(SourceSpan::default().on_slide(0))
                .with_help(diagnostic.help.clone().unwrap_or_else(|| {
                    format!("this is {source}'s to fix, not the deck's — report it there")
                }))
            })
            .collect()
    }
}

fn repaired(source: &str, id: &str, repair: &Repair) -> Diagnostic {
    let (code, severity) = match repair.reason {
        // An escape attempt is not a difference of opinion about density. It is
        // reported as blocking so a build that would have shipped it stops,
        // which is the same answer the offline rule gives a remote asset.
        Reason::Unsafe => ("theme/unsafe-token", slidx_core::Severity::Error),
        Reason::OutOfRange => ("theme/out-of-range-token", slidx_core::Severity::Warning),
    };

    let what = match repair.reason {
        Reason::Unsafe => "cannot be written into a page",
        Reason::OutOfRange => "is outside what the slide shell can hold",
    };

    Diagnostic::new(
        code,
        severity,
        format!(
            "theme `{id}` from {source}: `{}` = `{}` {what}, so `{}` was used",
            repair.field,
            repair.asked.escape_debug(),
            repair.given.escape_debug()
        ),
    )
    .at(SourceSpan::default().on_slide(0))
    .with_help(format!("this is {source}'s to fix, not the deck's — report it there"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(theme: &Theme) -> String {
        serde_json::to_string(theme).unwrap()
    }

    /// A theme nothing built in answers to, so resolution has something to find.
    fn published() -> Published {
        let mut theme = builtin::editorial();
        theme.id = "aurora".into();
        theme.name = "Aurora".into();

        Published::new("@example/theme-aurora", document(&theme))
    }

    fn codes(catalogue: &Catalogue) -> Vec<String> {
        catalogue.diagnostics().iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn a_deck_can_name_a_theme_that_arrived_in_a_package() {
        let catalogue = Catalogue::read(&[published()]);

        assert_eq!(codes(&catalogue), Vec::<String>::new());
        assert_eq!(catalogue.resolve("aurora").unwrap().theme.name, "Aurora");
    }

    #[test]
    fn a_resolved_theme_says_which_package_it_came_from() {
        // Every finding about it has to name something an author can go and
        // look at, and a theme they never wrote is not it.
        let catalogue = Catalogue::read(&[published()]);

        assert_eq!(
            catalogue.resolve("aurora").unwrap().source.as_deref(),
            Some("@example/theme-aurora")
        );
        assert_eq!(catalogue.resolve("minimal").unwrap().source, None);
    }

    #[test]
    fn a_built_in_name_still_means_the_built_in_when_a_package_claims_it() {
        // The precedence this module exists to fix in writing. A package that
        // could take `minimal` is a package that can repaint every deck in a
        // repository without changing a line of any of them.
        let mut impostor = builtin::terminal();
        impostor.id = "minimal".into();
        impostor.name = "Not minimal".into();

        let catalogue = Catalogue::read(&[Published::new("@evil/theme", document(&impostor))]);

        assert_eq!(catalogue.resolve("minimal").unwrap().theme, builtin::minimal());
        assert_eq!(codes(&catalogue), vec!["theme/unreadable-package"]);
        assert!(catalogue.diagnostics().has_blocking());
    }

    #[test]
    fn a_package_that_shadows_a_built_in_says_so_rather_than_being_dropped() {
        let mut impostor = builtin::terminal();
        impostor.id = "contrast".into();

        let catalogue = Catalogue::read(&[Published::new("@evil/theme", document(&impostor))]);
        let message = &catalogue.diagnostics().as_slice()[0].message;

        assert!(message.contains("@evil/theme"), "{message}");
        assert!(message.contains("built-in"), "{message}");
    }

    #[test]
    fn a_name_no_package_installed_resolves_to_nothing_rather_than_to_something_else() {
        // The other half of the precedence rule: a package disappearing must
        // not silently hand the deck a different theme. `dialect/unknown-theme`
        // is what tells the author, and it can only fire because this is None.
        let catalogue = Catalogue::read(&[published()]);

        assert!(catalogue.resolve("aurorra").is_none());
    }

    #[test]
    fn two_packages_claiming_one_name_is_reported_and_settled_deterministically() {
        // Whichever the caller handed over first keeps the name, so two builds
        // of the same tree cannot disagree about which theme a deck got.
        let first = published();
        let mut other = builtin::contrast();
        other.id = "aurora".into();
        other.name = "Also Aurora".into();
        let second = Published::new("@other/theme-aurora", document(&other));

        let catalogue = Catalogue::read(&[first, second]);

        assert_eq!(catalogue.resolve("aurora").unwrap().theme.name, "Aurora");
        assert_eq!(codes(&catalogue), vec!["theme/unreadable-package"]);
    }

    #[test]
    fn a_document_that_is_not_json_is_a_diagnostic_and_not_a_panic() {
        let catalogue = Catalogue::read(&[Published::new("@example/theme", "not json at all")]);

        assert_eq!(codes(&catalogue), vec!["theme/unreadable-package"]);
        assert!(catalogue.resolve("anything").is_none());
    }

    #[test]
    fn a_stylesheet_offered_as_a_theme_is_refused_with_the_reason() {
        // The mistake somebody will make first, because CSS is what a theme is
        // everywhere else.
        let catalogue = Catalogue::read(&[Published::new(
            "@example/theme",
            ":root { --slidx-color-text: red }",
        )]);
        let help = catalogue.diagnostics().as_slice()[0].help.clone().unwrap();

        assert!(help.contains("token document"), "{help}");
        assert!(help.contains("slidx.theme"), "{help}");
    }

    #[test]
    fn an_id_a_deck_could_not_write_is_refused() {
        // It reaches `theme:` in YAML and a diagnostic in a terminal, where a
        // control character is an escape sequence rather than a typo.
        for id in ["", "Aurora", "aurora theme", "../../etc", "a\u{1b}[31m", "-aurora"] {
            let mut theme = builtin::minimal();
            theme.id = id.into();

            let catalogue = Catalogue::read(&[Published::new("@example/theme", document(&theme))]);

            assert_eq!(codes(&catalogue), vec!["theme/unreadable-package"], "`{id}` was accepted");
        }
    }

    #[test]
    fn a_theme_that_tries_to_break_out_of_its_declaration_blocks_the_build() {
        // The guard already replaced the value; this is the part that makes a
        // build stop rather than quietly ship a theme that attempted it.
        let mut theme = builtin::minimal();
        theme.id = "aurora".into();
        theme.font_sans = "sans-serif</style><script>fetch('//x')</script>".into();

        let catalogue = Catalogue::read(&[Published::new("@evil/theme", document(&theme))]);

        assert_eq!(codes(&catalogue), vec!["theme/unsafe-token"]);
        assert!(catalogue.diagnostics().has_blocking());
        assert_eq!(catalogue.resolve("aurora").unwrap().theme.font_sans, default_theme().font_sans);
    }

    #[test]
    fn a_theme_that_only_asks_for_too_much_padding_still_builds() {
        // Out of range is a warning: the deck renders, inside a safe area the
        // shell chose, and the theme's author is the one told.
        let mut theme = builtin::minimal();
        theme.id = "aurora".into();
        theme.spacing.padding_px = 0.0;

        let catalogue = Catalogue::read(&[Published::new("@example/theme", document(&theme))]);

        assert_eq!(codes(&catalogue), vec!["theme/out-of-range-token"]);
        assert!(!catalogue.diagnostics().has_blocking());
        assert!(catalogue.resolve("aurora").unwrap().theme.spacing.padding_px > 0.0);
    }

    #[test]
    fn every_finding_about_a_package_names_the_package() {
        // An author reading it is looking at their own slides, and the answer
        // is not in them.
        let mut theme = builtin::minimal();
        theme.id = "aurora".into();
        theme.spacing.padding_px = 0.0;
        theme.font_mono = "{}".into();

        let catalogue =
            Catalogue::read(&[Published::new("@example/theme-aurora", document(&theme))]);

        for diagnostic in catalogue.diagnostics().iter() {
            assert!(diagnostic.message.contains("@example/theme-aurora"), "{}", diagnostic.message);
            assert!(diagnostic.help.is_some(), "{}", diagnostic.code);
        }
    }

    /// A theme resolved out of a package holding `broken`.
    fn resolving(broken: impl FnOnce(&mut Theme)) -> Resolved {
        let mut theme = builtin::minimal();
        theme.id = "aurora".into();
        broken(&mut theme);

        Catalogue::read(&[Published::new("@example/theme-aurora", document(&theme))])
            .resolve("aurora")
            .expect("the package resolves")
    }

    #[test]
    fn a_package_theme_faces_the_same_audit_a_built_in_does() {
        // The point of a theme being tokens. A published theme that ships text
        // nobody at the back can read is the failure this project exists to
        // catch, and it is caught by the rules that already judge a deck.
        let resolved = resolving(|theme| theme.light.text = theme.light.surface);
        let audited = resolved.audit(&LintOptions::default());

        assert!(
            audited.iter().any(|diagnostic| diagnostic.code.starts_with("contrast/")),
            "{audited:?}"
        );
    }

    #[test]
    fn an_audit_finding_keeps_the_rules_own_code_and_gains_the_origin() {
        let resolved = resolving(|theme| theme.scale.base_px = 12.0);
        let audited = resolved.audit(&LintOptions::default());
        let finding = audited
            .iter()
            .find(|d| d.code == "legibility/font-size")
            .expect("the size floor is a rule a package faces too");

        assert!(finding.message.contains("@example/theme-aurora"), "{}", finding.message);
    }

    #[test]
    fn a_built_in_is_not_re_audited_on_every_build() {
        // This crate's own suite already holds all four to these rules in every
        // room slidx models, so a second pass per build is arithmetic nobody
        // reads.
        let catalogue = Catalogue::default();
        let resolved = catalogue.resolve("contrast").unwrap();

        assert!(resolved.audit(&LintOptions::default()).is_empty());
    }

    #[test]
    fn a_theme_a_deck_never_names_cannot_fail_its_build() {
        // Installing two themes and using one must not mean being judged by
        // both. The one that renders is the one that matters.
        let mut illegible = builtin::minimal();
        illegible.id = "murk".into();
        illegible.light.text = illegible.light.surface;

        let catalogue = Catalogue::read(&[
            published(),
            Published::new("@example/theme-murk", document(&illegible)),
        ]);

        assert!(catalogue.diagnostics().is_empty(), "{:?}", catalogue.diagnostics());
        assert!(catalogue.resolve("aurora").unwrap().audit(&LintOptions::default()).is_empty());
    }

    #[test]
    fn an_installed_theme_is_offered_alongside_the_built_ins() {
        let names = Catalogue::read(&[published()]).names();

        for theme in builtin::all() {
            assert!(names.contains(&theme.id), "the picker lost `{}`", theme.id);
        }
        assert_eq!(names.last().map(String::as_str), Some("aurora"));
    }

    #[test]
    fn a_project_with_no_theme_packages_still_resolves_every_built_in() {
        let catalogue = Catalogue::default();

        for theme in builtin::all() {
            assert_eq!(catalogue.resolve(&theme.id).map(|found| found.theme), Some(theme));
        }
        assert!(catalogue.diagnostics().is_empty());
    }

    #[test]
    fn a_theme_document_written_before_a_field_existed_still_loads() {
        // The compatibility `Theme` already promises, exercised through the path
        // a real package takes rather than through serde alone.
        let mut value = serde_json::to_value(builtin::minimal()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("id".into(), "aurora".into());
        object.remove("motion");
        object["light"].as_object_mut().unwrap().remove("syntax");
        object["dark"].as_object_mut().unwrap().remove("syntax");

        let catalogue = Catalogue::read(&[Published::new("@example/theme", value.to_string())]);

        assert_eq!(codes(&catalogue), Vec::<String>::new());
        assert!(catalogue.resolve("aurora").is_some());
    }

    #[test]
    fn every_installed_theme_is_reachable_for_a_report() {
        let catalogue = Catalogue::read(&[published()]);
        let listed: Vec<&str> = catalogue.installed().map(|(source, _)| source).collect();

        assert_eq!(listed, vec!["@example/theme-aurora"]);
    }
}
