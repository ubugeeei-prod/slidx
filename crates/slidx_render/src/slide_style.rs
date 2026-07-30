//! A slide's Markdown-authored custom properties at the HTML boundary.
//!
//! The source writes them in `<style data-slidx>`, but the rendered document
//! puts them on the slide element itself. A slide page contains one slide; the
//! print shell contains all of them. Keeping the declarations element-local is
//! what makes both documents render the same answer.

use slidx_core::Slide;

/// CSS declarations, ready to append inside a slide's `style` attribute.
pub(crate) fn declarations(slide: &Slide) -> String {
    slide
        .style
        .iter()
        // The shell owns these two. Letting a Markdown style replace the design
        // box would make the page and its aspect-ratio calculation disagree.
        .filter(|(name, _)| !matches!(name.as_str(), "slide-width" | "slide-height"))
        .map(|(name, value)| format!(" --slidx-{}: {};", escape_name(name), escape_value(value)))
        .collect()
}

/// A complete optional attribute for an element that does not already have one.
pub(crate) fn attribute(slide: &Slide) -> String {
    let declarations = declarations(slide);
    if declarations.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", declarations.trim_start())
    }
}

fn escape_name(name: &str) -> String {
    name.chars()
        .filter(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || *character == '-'
        })
        .collect()
}

fn escape_value(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn properties_are_scoped_to_one_slide_element() {
        let slide = Slide {
            style: BTreeMap::from([
                ("color-surface".into(), "oklch(20% 0.02 260)".into()),
                ("layout".into(), "aside".into()),
            ]),
            ..Slide::default()
        };

        assert_eq!(
            attribute(&slide),
            " style=\"--slidx-color-surface: oklch(20% 0.02 260); --slidx-layout: aside;\""
        );
    }

    #[test]
    fn source_cannot_replace_the_shells_design_box() {
        let slide = Slide {
            style: BTreeMap::from([
                ("slide-width".into(), "1".into()),
                ("slide-height".into(), "1".into()),
            ]),
            ..Slide::default()
        };

        assert!(attribute(&slide).is_empty());
    }

    #[test]
    fn an_html_boundary_cannot_be_left_through_a_css_value() {
        let slide = Slide {
            style: BTreeMap::from([(
                "font-sans".into(),
                "sans-serif\"></article><script>alert(1)</script>".into(),
            )]),
            ..Slide::default()
        };

        let attribute = attribute(&slide);
        assert!(!attribute.contains("</article>"));
        assert!(!attribute.contains("<script>"));
        assert!(attribute.contains("&quot;"));
    }
}
