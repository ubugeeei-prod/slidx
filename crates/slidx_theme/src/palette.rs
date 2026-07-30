//! Colour roles.
//!
//! A palette names *roles*, not colours: `text`, `muted`, `accent`. That is
//! what lets the linter check a theme it has never seen, and what lets a deck
//! swap themes without rewriting a slide.
//!
//! Every palette carries both a light and a dark variant, because the room's
//! lighting is usually unknown until the day and switching at the venue must
//! not mean re-authoring anything.

use serde::{Deserialize, Serialize};
use slidx_highlight::Token;
use slidx_lint::Rgba;

/// The colours one slide is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// Behind the slide, visible as letterboxing when the aspect does not fit.
    pub canvas: Rgba,
    /// The slide itself.
    pub surface: Rgba,
    pub text: Rgba,
    /// Secondary text: captions, footers, attributions.
    pub muted: Rgba,
    pub heading: Rgba,
    /// Links, strong text, and the accent line.
    pub accent: Rgba,
    pub border: Rgba,
    pub code_surface: Rgba,
    pub code_text: Rgba,
    /// What the highlighter draws with, as the theme declared it.
    ///
    /// Optional for the same reason `motion` is: a theme package published
    /// before highlighting existed is a JSON file someone else owns and does
    /// not republish, and a required field here would break decks that never
    /// asked for the feature. Read it through [`Palette::syntax`], which
    /// always resolves to something drawable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<SyntaxPalette>,
}

/// One colour per thing the highlighter can recognise.
///
/// Named after [`slidx_highlight::Token`] rather than after a grammar, so a
/// theme cannot declare a colour for a token no scanner emits and cannot miss
/// one that every scanner does.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxPalette {
    pub comment: Rgba,
    pub string: Rgba,
    pub number: Rgba,
    pub keyword: Rgba,
    /// `type` is a Rust keyword, so the field carries the longer name and the
    /// theme file keeps the short one.
    #[serde(rename = "type")]
    pub type_name: Rgba,
    pub punctuation: Rgba,
}

impl SyntaxPalette {
    /// Every token drawn in one colour.
    ///
    /// What a theme written before highlighting existed resolves to: code that
    /// looks exactly as it did before, rather than code in colours nobody chose
    /// for it and nobody checked against the surface it sits on.
    pub fn monochrome(color: Rgba) -> Self {
        Self {
            comment: color,
            string: color,
            number: color,
            keyword: color,
            type_name: color,
            punctuation: color,
        }
    }

    pub fn get(&self, token: Token) -> Rgba {
        match token {
            Token::Comment => self.comment,
            Token::String => self.string,
            Token::Number => self.number,
            Token::Keyword => self.keyword,
            Token::Type => self.type_name,
            Token::Punctuation | Token::Plain => self.punctuation,
        }
    }
}

/// The palette role a token is drawn from, as the linter names it.
pub(crate) fn role_name(token: Token) -> &'static str {
    match token {
        Token::Comment => "codeComment",
        Token::String => "codeString",
        Token::Number => "codeNumber",
        Token::Keyword => "codeKeyword",
        Token::Type => "codeType",
        Token::Punctuation => "codePunctuation",
        Token::Plain => "codeText",
    }
}

/// Which variant of a theme is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];
}

/// Parses a hex colour in a test fixture.
///
/// Only tests write a colour as text now. Every colour a theme actually ships is
/// mixed by [`crate::builtin::recipe`] from a hue, a chroma and a lightness —
/// which is what `scripts/check-borrowed.mjs` enforces, because a hex literal in
/// a palette is precisely how a borrowed framework scale got in here.
///
/// Panics on a malformed literal, which is correct: these are constants in this
/// crate's own source, so a bad one is a bug rather than user input.
#[cfg(test)]
pub(crate) fn hex(text: &str) -> Rgba {
    slidx_lint::color::parse(text).unwrap_or_else(|| panic!("fixture colour `{text}` is malformed"))
}

impl Palette {
    /// What the highlighter draws with, resolved.
    ///
    /// The field says what the theme declared; this says what actually reaches
    /// a slide. Every caller wants the second one.
    pub fn syntax(&self) -> SyntaxPalette {
        self.syntax.unwrap_or_else(|| SyntaxPalette::monochrome(self.code_text))
    }

    /// Every text role paired with the background it is drawn on.
    ///
    /// This is the list the linter walks, so a role missing from it is a role
    /// nobody checks. Adding a colour to [`Palette`] without adding it here is
    /// caught by `every_palette_role_is_audited`.
    ///
    /// The syntax colours are here in full even when a theme declared none. A
    /// monochrome fallback audits six times over trivially, and the alternative
    /// — a role list whose length depends on the theme — is a list that reports
    /// a different number of checks for two themes and explains neither.
    pub fn pairs(&self) -> Vec<(&'static str, Rgba, Rgba)> {
        let syntax = self.syntax();

        let mut pairs = vec![
            ("text", self.text, self.surface),
            ("muted", self.muted, self.surface),
            ("heading", self.heading, self.surface),
            ("accent", self.accent, self.surface),
            ("codeText", self.code_text, self.code_surface),
        ];

        pairs.extend(
            Token::COLOURED
                .into_iter()
                .map(|token| (role_name(token), syntax.get(token), self.code_surface)),
        );

        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Palette {
        Palette {
            canvas: hex("#f4f4f5"),
            surface: hex("#ffffff"),
            text: hex("#18181b"),
            muted: hex("#52525b"),
            heading: hex("#09090b"),
            accent: hex("#1d4ed8"),
            border: hex("#e4e4e7"),
            code_surface: hex("#f4f4f5"),
            code_text: hex("#18181b"),
            syntax: None,
        }
    }

    #[test]
    fn every_text_role_is_paired_with_the_background_it_sits_on() {
        let pairs = sample().pairs();
        let names: Vec<&str> = pairs.iter().map(|(name, _, _)| *name).collect();

        assert_eq!(
            names,
            vec![
                "text",
                "muted",
                "heading",
                "accent",
                "codeText",
                "codeComment",
                "codeString",
                "codeNumber",
                "codeKeyword",
                "codeType",
                "codePunctuation",
            ]
        );
    }

    #[test]
    fn a_theme_that_declares_no_syntax_colours_draws_code_in_one_colour() {
        // A theme package published before highlighting existed keeps rendering
        // code exactly as it did, rather than in colours nobody chose and
        // nobody checked against the surface they sit on.
        let palette = sample();

        for token in Token::COLOURED {
            assert_eq!(palette.syntax().get(token), palette.code_text, "{}", token.as_token());
        }
    }

    #[test]
    fn a_declared_syntax_colour_is_the_one_that_reaches_the_slide() {
        let palette = Palette {
            syntax: Some(SyntaxPalette {
                comment: hex("#465064"),
                ..SyntaxPalette::monochrome(hex("#18181b"))
            }),
            ..sample()
        };

        assert_eq!(palette.syntax().get(Token::Comment), hex("#465064"));
        assert_eq!(palette.syntax().get(Token::String), hex("#18181b"));
    }

    #[test]
    fn every_syntax_role_is_audited_against_the_code_surface() {
        // Code is drawn on the code surface, not the slide. Checking a comment
        // colour against the slide background would pass a comment that is
        // invisible everywhere it is actually shown.
        let palette = sample();

        for (name, _, background) in palette.pairs() {
            if name.starts_with("code") {
                assert_eq!(background, palette.code_surface, "{name} is checked on the slide");
            }
        }
    }

    #[test]
    fn a_theme_file_spells_the_type_colour_without_rusts_keyword_in_the_way() {
        let syntax = SyntaxPalette {
            type_name: hex("#5b21b6"),
            ..SyntaxPalette::monochrome(hex("#000000"))
        };
        let json = serde_json::to_value(syntax).unwrap();

        assert_eq!(json["type"], serde_json::json!(hex("#5b21b6")));
    }

    #[test]
    fn code_text_is_paired_with_the_code_surface_not_the_slide() {
        let palette = sample();
        let (_, _, background) =
            palette.pairs().into_iter().find(|(name, _, _)| *name == "codeText").unwrap();

        assert_eq!(background, palette.code_surface);
    }

    #[test]
    fn schemes_round_trip_through_their_tokens() {
        assert_eq!(Scheme::Light.as_token(), "light");
        assert_eq!(Scheme::Dark.as_token(), "dark");
        assert_eq!(Scheme::ALL.len(), 2);
    }

    #[test]
    #[should_panic(expected = "malformed")]
    fn a_malformed_built_in_colour_fails_loudly() {
        hex("#not-a-colour");
    }
}
