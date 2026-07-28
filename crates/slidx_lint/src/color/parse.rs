//! Reading colours out of themes and Markdown.
//!
//! Split from the colour model because they are different problems: the model
//! is arithmetic on values, this is tolerating the notations people actually
//! write. Deliberately partial and deliberately strict — a colour this module
//! cannot read is reported as unreadable rather than silently treated as
//! black, because a wrong contrast reading is worse than a missing one.

use super::Rgba;

/// Parses the colour notations that appear in slide themes and Markdown.
///
/// Returns `None` rather than guessing: a colour the linter cannot read is
/// reported as unreadable, never silently treated as black.
pub fn parse(text: &str) -> Option<Rgba> {
    let text = text.trim();

    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(body) = strip_call(text, "rgb").or_else(|| strip_call(text, "rgba")) {
        return parse_rgb(body);
    }
    if let Some(body) = strip_call(text, "hsl").or_else(|| strip_call(text, "hsla")) {
        return parse_hsl(body);
    }

    named(&text.to_ascii_lowercase())
}

fn strip_call<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(name)?.trim_start();
    rest.strip_prefix('(')?.strip_suffix(')')
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    let digits: Vec<u8> =
        hex.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;

    let expand = |value: u8| value * 17;
    match digits.len() {
        3 => Some(Rgba::opaque(expand(digits[0]), expand(digits[1]), expand(digits[2]))),
        4 => Some(Rgba {
            r: expand(digits[0]),
            g: expand(digits[1]),
            b: expand(digits[2]),
            a: f64::from(expand(digits[3])) / 255.0,
        }),
        6 => Some(Rgba::opaque(
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
        )),
        8 => Some(Rgba {
            r: digits[0] * 16 + digits[1],
            g: digits[2] * 16 + digits[3],
            b: digits[4] * 16 + digits[5],
            a: f64::from(digits[6] * 16 + digits[7]) / 255.0,
        }),
        _ => None,
    }
}

/// Splits the comma or space separated arguments of a colour function.
fn arguments(body: &str) -> Vec<&str> {
    body.split([',', '/', ' ']).map(str::trim).filter(|part| !part.is_empty()).collect()
}

fn parse_rgb(body: &str) -> Option<Rgba> {
    let parts = arguments(body);
    if parts.len() < 3 {
        return None;
    }

    let channel = |text: &str| -> Option<u8> {
        let value = match text.strip_suffix('%') {
            Some(percent) => percent.trim().parse::<f64>().ok()? * 2.55,
            None => text.parse::<f64>().ok()?,
        };
        Some(value.round().clamp(0.0, 255.0) as u8)
    };

    Some(Rgba {
        r: channel(parts[0])?,
        g: channel(parts[1])?,
        b: channel(parts[2])?,
        a: parts.get(3).and_then(|part| alpha(part)).unwrap_or(1.0),
    })
}

fn alpha(text: &str) -> Option<f64> {
    let value = match text.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f64>().ok()? / 100.0,
        None => text.parse::<f64>().ok()?,
    };
    Some(value.clamp(0.0, 1.0))
}

fn parse_hsl(body: &str) -> Option<Rgba> {
    let parts = arguments(body);
    if parts.len() < 3 {
        return None;
    }

    let hue = parts[0].trim_end_matches("deg").parse::<f64>().ok()?.rem_euclid(360.0) / 360.0;
    let saturation = parts[1].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    let lightness = parts[2].trim_end_matches('%').parse::<f64>().ok()? / 100.0;

    let chroma = if lightness <= 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let base = 2.0 * lightness - chroma;

    let component = |offset: f64| {
        let mut position = (hue + offset).rem_euclid(1.0);
        position = match position {
            p if p < 1.0 / 6.0 => base + (chroma - base) * 6.0 * p,
            p if p < 0.5 => chroma,
            p if p < 2.0 / 3.0 => base + (chroma - base) * (2.0 / 3.0 - p) * 6.0,
            _ => base,
        };
        (position * 255.0).round().clamp(0.0, 255.0) as u8
    };

    Some(Rgba {
        r: component(1.0 / 3.0),
        g: component(0.0),
        b: component(-1.0 / 3.0),
        a: parts.get(3).and_then(|part| alpha(part)).unwrap_or(1.0),
    })
}

/// The CSS keywords a slide theme realistically uses.
///
/// Deliberately partial. An unrecognised keyword is reported as unreadable,
/// which is more useful than silently checking the wrong colour.
fn named(name: &str) -> Option<Rgba> {
    let color = match name {
        "transparent" => Rgba { r: 0, g: 0, b: 0, a: 0.0 },
        "white" => Rgba::WHITE,
        "black" => Rgba::BLACK,
        "red" => Rgba::opaque(255, 0, 0),
        "green" => Rgba::opaque(0, 128, 0),
        "blue" => Rgba::opaque(0, 0, 255),
        "yellow" => Rgba::opaque(255, 255, 0),
        "orange" => Rgba::opaque(255, 165, 0),
        "purple" => Rgba::opaque(128, 0, 128),
        "gray" | "grey" => Rgba::opaque(128, 128, 128),
        "silver" => Rgba::opaque(192, 192, 192),
        "navy" => Rgba::opaque(0, 0, 128),
        "teal" => Rgba::opaque(0, 128, 128),
        _ => return None,
    };
    Some(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_notations_all_parse() {
        assert_eq!(parse("#fff"), Some(Rgba::WHITE));
        assert_eq!(parse("#FFFFFF"), Some(Rgba::WHITE));
        assert_eq!(parse("#000000"), Some(Rgba::BLACK));
        assert_eq!(parse("#1d76db"), Some(Rgba::opaque(29, 118, 219)));
    }

    #[test]
    fn hex_alpha_notations_parse() {
        assert_eq!(parse("#00000080").map(|c| (c.a * 100.0).round()), Some(50.0));
        assert_eq!(parse("#000f").map(|c| c.a), Some(1.0));
    }

    #[test]
    fn functional_notations_parse() {
        assert_eq!(parse("rgb(255, 255, 255)"), Some(Rgba::WHITE));
        assert_eq!(parse("rgb(255 255 255)"), Some(Rgba::WHITE));
        assert_eq!(parse("rgba(0, 0, 0, 0.5)").map(|c| c.a), Some(0.5));
        assert_eq!(parse("rgb(100%, 0%, 0%)"), Some(Rgba::opaque(255, 0, 0)));
    }

    #[test]
    fn hsl_maps_onto_the_expected_corners() {
        assert_eq!(parse("hsl(0, 100%, 50%)"), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(parse("hsl(120, 100%, 50%)"), Some(Rgba::opaque(0, 255, 0)));
        assert_eq!(parse("hsl(240deg 100% 50%)"), Some(Rgba::opaque(0, 0, 255)));
        assert_eq!(parse("hsl(0, 0%, 100%)"), Some(Rgba::WHITE));
    }

    #[test]
    fn unreadable_colours_are_none_rather_than_a_guess() {
        assert_eq!(parse("var(--accent)"), None);
        assert_eq!(parse("#12345"), None);
        assert_eq!(parse("rebeccapurple"), None, "unknown keywords must not resolve to black");
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(parse("#1d76db").unwrap().to_hex(), "#1d76db");
    }
}
