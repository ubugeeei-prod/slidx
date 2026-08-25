//! The window the speaker looks at.
//!
//! Everything here answers a question a speaker asks mid-sentence, so the
//! layout is ordered by how urgent the answer is rather than by how much space
//! it needs:
//!
//! | Question | Where |
//! |---|---|
//! | How am I doing for time? | largest element, top left |
//! | What am I saying now? | notes, the biggest panel |
//! | What comes next? | preview, so the transition is not a surprise |
//! | Where am I? | slide count, smallest |
//!
//! The notes get more room than the current slide's preview. The speaker can
//! already see the slide — it is on the wall behind them — and what they
//! cannot see is what they meant to say about it.

use slidx_core::{Deck, Slide};
use slidx_theme::{css, transition::Transition, Theme};

use crate::markdown::{render, MarkdownOptions};
use crate::presenter_layout;
use crate::presenter_script;
use crate::shell::{render_static_preview, ShellOptions};

/// How to build the presenter page.
#[derive(Debug, Clone)]
pub struct PresenterOptions {
    pub theme: Theme,
    pub markdown: MarkdownOptions,
    /// Module URL of the runtime, imported for the clock and mirroring.
    pub runtime_src: String,
    /// Module URL of the presenter-only rehearsal recorder.
    /// Where the presenter's own half of the runtime is imported from.
    ///
    /// Not `runtime_src`: that file is on every staged slide, and the timer and
    /// the pacing model are on no slide at all.
    pub presenter_runtime_src: String,
    pub rehearsal_src: String,
    /// Module URL a clip's level is measured from.
    ///
    /// Its own file rather than part of `runtime_src`, because that one is on
    /// every staged slide and this is for the few decks that place a clip.
    /// See `crate::media_script`.
    pub media_src: String,
}

impl Default for PresenterOptions {
    fn default() -> Self {
        Self {
            theme: slidx_theme::default_theme(),
            markdown: MarkdownOptions::default(),
            runtime_src: "./runtime.js".to_string(),
            presenter_runtime_src: "./presenter.js".to_string(),
            rehearsal_src: "./rehearsal.js".to_string(),
            media_src: "./media.js".to_string(),
        }
    }
}

/// Renders the presenter view for one slide.
pub fn render_presenter(deck: &Deck, slide: &Slide, options: &PresenterOptions) -> String {
    let next = deck.slides.get(slide.index as usize + 1);

    format!(
        r#"<!doctype html>
<html lang="{lang}" data-slidx-presenter>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Presenter — {deck_title}</title>
{noindex}
<style>
{theme_css}
{layout_css}
</style>
</head>
<body>
<main class="slidx-presenter">
  <header class="slidx-presenter-bar">
    <div class="slidx-clock" data-slidx-clock data-slidx-status="untimed">
      <span class="slidx-clock-value" data-slidx-elapsed>0:00</span>
      <span class="slidx-clock-budget" data-slidx-remaining>{budget_label}</span>
      <span class="slidx-pace" data-slidx-pace data-slidx-pace-state="unknown" aria-live="polite"
        ></span>
    </div>
    <div class="slidx-presenter-actions">
      <button type="button" data-slidx-action="toggle" aria-label="Start or pause the timer">
        Start
      </button>
      <button type="button" data-slidx-action="reset" aria-label="Reset the timer">Reset</button>
      <span class="slidx-presenter-divider" aria-hidden="true"></span>
      <button
        type="button"
        data-slidx-action="keys"
        aria-expanded="false"
        aria-controls="slidx-keys"
        aria-label="Show the keyboard shortcuts"
      >
        ?
      </button>
      <button
        type="button"
        data-slidx-action="present"
        aria-expanded="false"
        aria-controls="slidx-present"
      >
        Present
      </button>
      <span class="slidx-presenter-divider" aria-hidden="true"></span>
      <button
        type="button"
        data-slidx-action="rehearse"
        aria-label="Start or pause rehearsal recording"
      >
        Rehearse
      </button>
      <button type="button" data-slidx-action="finish-rehearsal" hidden>
        Finish rehearsal
      </button>
      <button type="button" data-slidx-action="abandon-rehearsal" hidden>
        End early
      </button>
      <span class="slidx-rehearsal-status" data-slidx-rehearsal-status aria-live="polite"></span>
      <span class="slidx-demo-state" data-slidx-demo-state aria-live="polite"></span>
      <span class="slidx-clip-level" data-slidx-clip-level aria-live="polite"></span>
      <span class="slidx-presenter-position">{number} / {count}</span>
      <span class="slidx-presenter-stop" data-slidx-stop>{stops}</span>
    </div>
  </header>

  <section class="slidx-keys" id="slidx-keys" data-slidx-keys aria-label="Keyboard" hidden>
    <dl class="slidx-keys-list">
{keys}    </dl>
  </section>

  <section
    class="slidx-present"
    id="slidx-present"
    data-slidx-present
    aria-label="Presentation mode"
    hidden
  >
    <p class="slidx-present-state" data-slidx-present-state aria-live="polite"></p>
    <ul class="slidx-present-checklist" data-slidx-present-checklist></ul>
  </section>

  <section class="slidx-presenter-notes" aria-label="Speaker notes">
{notes}
  </section>

  <aside class="slidx-presenter-next" aria-label="Next slide">
    <header class="slidx-presenter-next-header">
      <h2 class="slidx-presenter-label">Next</h2>
      <span class="slidx-presenter-next-position">{next_position}</span>
    </header>
{next_preview}
  </aside>

  <section
    class="slidx-rehearsal-report"
    data-slidx-rehearsal-report
    aria-labelledby="slidx-rehearsal-title"
    hidden
  >
    <header class="slidx-rehearsal-report-header">
      <h2 id="slidx-rehearsal-title">Rehearsal report</h2>
      <button type="button" data-slidx-action="new-rehearsal">New rehearsal</button>
    </header>
    <p class="slidx-rehearsal-advice" data-slidx-rehearsal-advice></p>
    <p class="slidx-rehearsal-trend" data-slidx-rehearsal-trend></p>
    <p class="slidx-rehearsal-total" data-slidx-rehearsal-total></p>
    <ol class="slidx-rehearsal-slides" data-slidx-rehearsal-slides></ol>
  </section>
</main>
<script type="module">
{script}
</script>
{media}
</body>
</html>
"#,
        // The notes are the deck's own language, and this page is mostly notes.
        lang = escape(deck.language()),
        deck_title = escape(deck.meta.title.as_deref().unwrap_or("slidx")),
        keys = crate::keys::rows(deck),
        // Never indexed, whatever the deck says about being published. This page
        // holds the speaker's notes — the half of a talk written to be said and
        // not read — and it is one URL away from every audience slide.
        noindex = crate::seo::NOINDEX,
        theme_css = css::render(&options.theme),
        layout_css = presenter_layout::STYLESHEET,
        budget_label = budget_label(deck),
        number = slide.index + 1,
        count = deck.slides.len(),
        notes = notes_html(slide, options),
        next_position = next
            .map(|next| format!("{} / {}", next.index + 1, deck.slides.len()))
            .unwrap_or_default(),
        next_preview = next_preview(deck, next, options),
        stops = stop_label(1, slide.timeline.frames().len()),
        script = presenter_script::render(
            deck,
            slide,
            &options.runtime_src,
            &options.presenter_runtime_src,
            &options.rehearsal_src,
        ),
        media = crate::media_script::presenter_script(
            deck,
            slide,
            &options.media_src,
            &options.markdown,
        ),
    )
}

/// Where the speaker is inside the slide, or nothing when there is no inside.
///
/// A slide with one stop has no build to be partway through, and a counter
/// reading "1 of 1" is a number that never changes — noise on the one screen a
/// speaker glances at mid-sentence.
fn stop_label(stop: usize, stops: usize) -> String {
    if stops < 2 {
        return String::new();
    }

    format!("stop {stop} of {stops}")
}

/// The slot length, as a speaker reads it before starting.
///
/// The deck's declared `duration`, not the sum of per-slide budgets: the slot
/// is what the speaker was given, and the sum is what they planned. The clock
/// counts against the one they cannot change.
fn budget_label(deck: &Deck) -> String {
    match deck.meta.duration_seconds {
        Some(seconds) => format!("of {}", human_duration(seconds)),
        None => "untimed".to_string(),
    }
}

fn human_duration(seconds: u32) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;

    match (hours, minutes, seconds) {
        (0, 0, seconds) => format!("{seconds}s"),
        (0, minutes, 0) => format!("{minutes}m"),
        (0, minutes, seconds) => format!("{minutes}m {seconds}s"),
        (hours, 0, 0) => format!("{hours}h"),
        (hours, minutes, 0) => format!("{hours}h {minutes}m"),
        (hours, 0, seconds) => format!("{hours}h {seconds}s"),
        (hours, minutes, seconds) => format!("{hours}h {minutes}m {seconds}s"),
    }
}

/// Notes, or a reminder that there are none.
///
/// An empty panel reads as broken. Saying the slide has no notes says the
/// speaker is looking at the right place and there is simply nothing there.
fn notes_html(slide: &Slide, options: &PresenterOptions) -> String {
    if slide.notes.is_empty() {
        return "    <p class=\"slidx-presenter-empty\">No notes for this slide.</p>".to_string();
    }

    slide.notes.iter().map(|note| render(note, &options.markdown)).collect::<Vec<_>>().join("\n")
}

/// The actual next slide, isolated so a preview can never run authored code.
fn next_preview(deck: &Deck, next: Option<&Slide>, options: &PresenterOptions) -> String {
    match next {
        Some(slide) => {
            let shell = ShellOptions {
                markdown: options.markdown,
                include_runtime: false,
                ..ShellOptions::default()
            }
            .with_theme(options.theme.clone());
            let preview = render_static_preview(deck, slide, &shell);
            let (width, height) = deck.meta.aspect.dimensions();

            format!(
                concat!(
                    "    <iframe class=\"slidx-presenter-preview\" ",
                    "title=\"Next: {title}\" tabindex=\"-1\" sandbox ",
                    "style=\"aspect-ratio: {width} / {height};\" ",
                    "srcdoc=\"{preview}\"></iframe>\n",
                    "{cues}",
                ),
                title = escape(&slide.display_title()),
                width = width,
                height = height,
                preview = escape(&preview),
                cues = delivery_cues(deck, slide),
            )
        }
        None => "    <p class=\"slidx-presenter-empty\">Last slide.</p>".to_string(),
    }
}

/// What the speaker needs to know before advancing, in glance order.
fn delivery_cues(deck: &Deck, slide: &Slide) -> String {
    let mut cues = Vec::with_capacity(3);

    if slide.optional {
        cues.push(("optional", "Safe to skip".to_string()));
    }

    if let Some(seconds) = slide.budget_seconds {
        cues.push(("timing", format!("Plan · {}", human_duration(seconds))));
    } else {
        let estimate = slide.estimated_seconds();
        if estimate > 0 {
            cues.push(("timing", format!("Estimate · ≈ {}", human_duration(estimate))));
        }
    }

    let transition = slide
        .transition
        .as_deref()
        .or(deck.meta.transition.as_deref())
        .and_then(Transition::parse)
        .unwrap_or_default();
    cues.push(("arrival", format!("Arrival · {}", transition.name())));

    let items = cues
        .into_iter()
        .map(|(kind, label)| {
            format!("      <li data-slidx-cue=\"{kind}\">{}</li>\n", escape(&label))
        })
        .collect::<String>();

    format!(
        "    <ul class=\"slidx-presenter-cues\" aria-label=\"Next slide delivery\">\n{items}    </ul>"
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn presenter(source: &str, index: usize) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render_presenter(&deck, &deck.slides[index], &PresenterOptions::default())
    }

    #[test]
    fn shows_the_notes_for_this_slide() {
        let html = presenter("# One\n\n<!-- notes: say this out loud -->\n", 0);
        assert!(html.contains("say this out loud"));
    }

    #[test]
    fn says_so_when_a_slide_has_no_notes() {
        // An empty panel reads as broken; this says the speaker is in the
        // right place and there is simply nothing there.
        assert!(presenter("# One\n", 0).contains("No notes for this slide"));
    }

    #[test]
    fn shows_what_comes_next() {
        // A transition should never be a surprise to the person causing it.
        let html = presenter("# One\n\n---\n\n# Two\n", 0);
        assert!(html.contains("Two"));
        assert!(html.contains("<iframe class=\"slidx-presenter-preview\""));
        assert!(html.contains("sandbox"));
        assert!(html.contains("tabindex=\"-1\""));
    }

    #[test]
    fn the_next_preview_uses_the_same_layout_frame_as_the_audience() {
        let html =
            presenter("# One\n\n---\nlayout: split\n---\n\n# Left\n\n{.right}\nBeside it.\n", 0);
        let preview = preview_srcdoc(&html);

        assert!(preview.contains("data-slidx-layout=&quot;split&quot;"), "wrong frame:\n{preview}");
        assert!(
            preview.contains("data-slidx-region=&quot;right&quot;"),
            "no right region:\n{preview}"
        );
        assert!(preview.contains("Beside it."));
    }

    #[test]
    fn the_next_preview_is_inert_even_when_the_slide_is_not() {
        let html = presenter(
            concat!(
                "# One\n\n",
                "---\n",
                "transition: fade\n",
                "camera: body\n",
                "demo:\n",
                "  live: https://app.example.com\n",
                "  fallback: ./demo.mp4\n",
                "---\n\n",
                "# Live\n\n",
                "- first <!-- step -->\n",
            ),
            0,
        );
        let preview = preview_srcdoc(&html);

        for marker in [
            "&lt;script",
            "&lt;figure class=&quot;slidx-demo",
            "&lt;figure class=&quot;slidx-camera",
            "@view-transition",
        ] {
            assert!(!preview.contains(marker), "preview contains {marker}:\n{preview}");
        }
        assert!(!preview.contains("https://app.example.com"));
    }

    #[test]
    fn the_next_slide_carries_its_delivery_plan() {
        let html = presenter(
            concat!(
                "---\ntransition: push\n---\n\n",
                "# One\n\n",
                "---\ntransition: fade\nbudget: 90s\noptional: true\n---\n\n",
                "# Two\n",
            ),
            0,
        );

        assert!(html.contains("Safe to skip"));
        assert!(html.contains("Plan · 1m 30s"));
        assert!(html.contains("Arrival · Fade"));
        assert!(html.contains("2 / 2"));
    }

    #[test]
    fn notes_supply_an_estimate_when_the_next_slide_has_no_plan() {
        let html =
            presenter("# One\n\n---\n\n# Two\n\n<!-- notes: one two three four five -->\n", 0);

        assert!(html.contains("Estimate · ≈ 2s"), "estimate absent:\n{html}");
        assert!(html.contains("Arrival · Cut"));
    }

    #[test]
    fn says_so_on_the_last_slide() {
        assert!(presenter("# One\n\n---\n\n# Two\n", 1).contains("Last slide"));
    }

    #[test]
    fn shows_where_in_the_deck_the_speaker_is() {
        assert!(presenter("# One\n\n---\n\n# Two\n", 1).contains("2 / 2"));
    }

    #[test]
    fn reads_the_slot_length_from_the_deck() {
        let html = presenter("---\nduration: 20m\n---\n\n# One\n", 0);

        assert!(html.contains("of 20m"));
        assert!(html.contains("const budgetMs = 1200000"));
    }

    #[test]
    fn an_untimed_talk_says_so_rather_than_showing_a_wrong_number() {
        let html = presenter("# One\n", 0);

        assert!(html.contains("untimed"));
        assert!(html.contains("const budgetMs = undefined"));
    }

    #[test]
    fn a_long_budget_reads_in_hours() {
        assert!(presenter("---\nduration: 90m\n---\n\n# One\n", 0).contains("of 1h 30m"));
        assert!(presenter("---\nduration: 120m\n---\n\n# One\n", 0).contains("of 2h"));
    }

    #[test]
    fn a_partial_minute_keeps_the_seconds_the_speaker_planned() {
        assert_eq!(human_duration(90), "1m 30s");
        assert_eq!(human_duration(45), "45s");
    }

    #[test]
    fn the_timer_can_be_started_and_reset() {
        let html = presenter("# One\n", 0);

        assert!(html.contains(r#"data-slidx-action="toggle""#));
        assert!(html.contains(r#"data-slidx-action="reset""#));
    }

    #[test]
    fn a_rehearsal_can_be_recorded_finished_and_started_again() {
        let html = presenter("# One\n", 0);

        for action in ["rehearse", "finish-rehearsal", "abandon-rehearsal", "new-rehearsal"] {
            assert!(
                html.contains(&format!(r#"data-slidx-action="{action}""#)),
                "missing {action}:\n{html}"
            );
        }
        assert!(html.contains("openRehearsalSession"));
        assert!(html.contains("data-slidx-rehearsal-report"));
    }

    #[test]
    fn rehearsal_receives_every_slide_and_its_declared_budget() {
        let html = presenter("---\nbudget: 30s\n---\n\n# One\n\n---\n\n# Two\n", 0);

        assert!(
            html.contains(
                r#"const deckSlides = [{"id":"one","title":"One","budgetMs":30000},{"id":"two","title":"Two"}]"#
            ),
            "deck plan is absent:\n{html}"
        );
    }

    #[test]
    fn an_optional_slide_says_so_where_pacing_can_read_it() {
        // Pacing only ever offers optional slides to drop, so this flag is the
        // difference between advice and a suggestion to cut the argument.
        let html = presenter("# One\n\n---\n\n---\noptional: true\n---\n\n# Two\n", 0);

        assert!(html.contains(r#"{"id":"two","title":"Two","optional":true}"#), "{html}");
    }

    #[test]
    fn a_slide_that_is_not_optional_carries_no_flag_for_it() {
        // Absence is what pacing reads as "not optional", so a `false` on every
        // slide would be bytes that say nothing.
        let html = presenter("# One\n", 0);

        assert!(html.contains(r#"const deckSlides = [{"id":"one","title":"One"}]"#), "{html}");
    }

    #[test]
    fn the_bar_holds_a_line_for_whether_the_talk_will_fit() {
        let html = presenter("---\nduration: 20m\n---\n\n# One\n", 0);

        assert!(html.contains("data-slidx-pace"), "no pace line:\n{html}");
        assert!(html.contains("assessPace"), "nothing computes it:\n{html}");
        assert!(html.contains("describePace"), "nothing words it:\n{html}");
    }

    #[test]
    fn the_pace_line_starts_empty_rather_than_claiming_a_reading() {
        // The page is rendered before a timer has run, and a talk that has not
        // started is not behind. `describePace` fills this in.
        let html = presenter("---\nduration: 20m\n---\n\n# One\n", 0);

        assert!(html.contains(r#"data-slidx-pace-state="unknown""#), "{html}");
        assert!(
            html.contains(
                r#"aria-live="polite"
        ></span>"#
            ) || html.contains("></span>"),
            "{html}"
        );
    }

    #[test]
    fn rehearsal_import_uses_the_source_the_builder_supplies() {
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let options = PresenterOptions {
            rehearsal_src: "/assets/rehearse.js".to_string(),
            ..PresenterOptions::default()
        };
        let html = render_presenter(&deck, &deck.slides[0], &options);

        assert!(html.contains(r#"from "/assets/rehearse.js""#));
    }

    #[test]
    fn the_controls_are_reachable_without_a_mouse() {
        // A speaker driving from a clicker or a keyboard has no pointer.
        let html = presenter("# One\n", 0);
        assert!(html.contains("aria-label=\"Start or pause the timer\""));
        assert!(html.contains("aria-label=\"Start or pause rehearsal recording\""));
    }

    #[test]
    fn it_mirrors_its_position_to_the_other_window() {
        // The step is whatever the page opened at rather than a literal zero:
        // a presenter page reached from a `?step=` link is already partway
        // into a build, and announcing stop zero would drag the projector back.
        let html = presenter("# One\n\n---\n\n# Two\n", 1);

        assert!(html.contains("mirror.send({ slide: 1, step: deck.step })"));
        assert!(html.contains("deck.subscribe("), "moves are not announced:\n{html}");
    }

    #[test]
    fn the_speaker_can_drive_the_deck_from_the_window_they_are_looking_at() {
        // A clicker sends its keys to whichever window is focused, and that is
        // the speaker's own screen. Without this the deck can only be advanced
        // by focusing the projector, which is on the other machine.
        let html = presenter("# One\n\n---\n\n# Two\n", 0);

        assert!(html.contains("createNavigator"), "no navigator:\n{html}");
        assert!(html.contains("deck.handleKey(event)"), "keys are not handled:\n{html}");
    }

    #[test]
    fn it_counts_stops_only_when_there_is_a_build_to_be_partway_through() {
        // "stop 1 of 1" is a number that never changes, on the one screen a
        // speaker glances at mid-sentence.
        let staged = presenter("- one <!-- step -->\n- two <!-- step -->\n", 0);
        let plain = presenter("# One\n", 0);

        assert!(staged.contains("stop 1 of 3"), "no stop counter on a staged slide:\n{staged}");
        assert!(plain.contains("data-slidx-stop></span>"), "a stop counter for one stop:\n{plain}");
    }

    #[test]
    fn a_clip_on_the_next_slide_is_named_on_this_one() {
        let html =
            presenter("# One\n\n---\n\n# Two\n\n<video src=\"./loud.mp4\" controls></video>\n", 0);

        assert!(html.contains("data-slidx-clip-level"), "no clip line:\n{html}");
        assert!(html.contains("measureClip"), "nothing measures it:\n{html}");
        assert!(html.contains("./loud.mp4"), "the file is unnamed:\n{html}");
        assert!(html.contains(r#"from "./media.js""#), "{html}");
    }

    #[test]
    fn a_deck_with_no_clip_does_not_import_the_decoder() {
        let html = presenter("# One\n", 0);

        assert!(!html.contains("measureClip"), "{html}");
        assert!(!html.contains("media.js"), "{html}");
    }

    #[test]
    fn media_import_uses_the_source_the_builder_supplies() {
        let deck = parse_deck(
            "# One\n\n---\n\n# Two\n\n<audio src=\"./a.mp3\"></audio>\n",
            &DeckParseOptions::default(),
        );
        let options = PresenterOptions {
            media_src: "/assets/media.js".to_string(),
            ..PresenterOptions::default()
        };
        let html = render_presenter(&deck, &deck.slides[0], &options);

        assert!(html.contains(r#"from "/assets/media.js""#));
    }

    #[test]
    fn going_back_to_the_first_slide_names_the_deck_root() {
        // Slide one's presenter lives at the root and the rest live one
        // directory down, so `slide + 1` addresses a directory that does not
        // exist for exactly the slide a speaker starts over from.
        let html = presenter("# One\n\n---\n\n# Two\n", 1);

        assert!(html.contains("`${root}presenter/`"), "slide one is unreachable:\n{html}");
    }

    #[test]
    fn nothing_in_the_presenter_page_is_remote() {
        // The presenter view is what a speaker falls back to when the network
        // has already failed them once.
        let html = presenter("# One\n", 0);

        for marker in ["http://", "https://", "//cdn"] {
            assert!(!html.contains(marker), "presenter reaches for {marker}");
        }
    }

    #[test]
    fn a_deck_title_containing_markup_is_escaped() {
        let html = presenter("---\ntitle: \"a <script> b\"\n---\n\n# One\n", 0);

        assert!(!html.contains("<title>Presenter — a <script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    fn preview_srcdoc(html: &str) -> &str {
        let start = html.find("srcdoc=\"").expect("a preview srcdoc") + "srcdoc=\"".len();
        let rest = &html[start..];
        &rest[..rest.find('\"').expect("a closed srcdoc attribute")]
    }
}
