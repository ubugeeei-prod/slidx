//! Behaviour inlined into the presenter page.
//!
//! Kept apart from the presenter document and its layout so adding a control
//! does not turn one renderer into a script, a stylesheet, and a template
//! tangled together. The output stays inline: that is what lets a presenter
//! page remain usable from a `file://` fallback.

use serde::Serialize;
use slidx_core::{Deck, Slide};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RehearsalSlide<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u64>,
}

pub(crate) fn render(deck: &Deck, slide: &Slide, runtime_src: &str, rehearsal_src: &str) -> String {
    format!(
        r#"import {{
  createTimer,
  formatDuration,
  createMirror,
  createNavigator,
  createStopCursor,
}} from "{runtime_src}";
import {{
  formatDelta,
  formatSpan,
  openRehearsalSession,
}} from "{rehearsal_src}";

// Slide one's presenter lives at the deck root and the rest live one directory
// down. Resolving that root produces one stable storage key across every page.
const root = {index} === 0 ? "../" : "../../";
const budgetMs = {budget};
const rehearsalSlides = {rehearsal_slides};
const rehearsalSlideId = {slide_id};

let browserStorage;
try {{
  browserStorage = window.localStorage;
}} catch {{
  // Some browsers expose a file URL but deny its storage. The session keeps
  // recording in memory and reports that navigation persistence is absent.
}}

const rehearsal = openRehearsalSession({{
  key: `slidx:rehearsal:${{new URL(root, location.href).pathname}}`,
  slideId: rehearsalSlideId,
  slides: rehearsalSlides,
  storage: browserStorage,
}});
const openingRehearsal = rehearsal.state();
const timer = createTimer({{ budgetMs, initialElapsedMs: openingRehearsal.elapsedMs }});
if (openingRehearsal.status === "recording") timer.start();
const mirror = createMirror();

const clock = document.querySelector("[data-slidx-clock]");
const elapsed = document.querySelector("[data-slidx-elapsed]");
const remaining = document.querySelector("[data-slidx-remaining]");
const toggle = document.querySelector('[data-slidx-action="toggle"]');
const reset = document.querySelector('[data-slidx-action="reset"]');
const rehearse = document.querySelector('[data-slidx-action="rehearse"]');
const finishRehearsal = document.querySelector('[data-slidx-action="finish-rehearsal"]');
const abandonRehearsal = document.querySelector('[data-slidx-action="abandon-rehearsal"]');
const newRehearsal = document.querySelector('[data-slidx-action="new-rehearsal"]');
const rehearsalStatus = document.querySelector("[data-slidx-rehearsal-status]");
const rehearsalReport = document.querySelector("[data-slidx-rehearsal-report]");
const rehearsalAdvice = document.querySelector("[data-slidx-rehearsal-advice]");
const rehearsalTotal = document.querySelector("[data-slidx-rehearsal-total]");
const rehearsalSlidesList = document.querySelector("[data-slidx-rehearsal-slides]");

function paint() {{
  const state = timer.state();
  elapsed.textContent = formatDuration(state.elapsedMs);
  clock.dataset.slidxStatus = state.status;
  toggle.textContent = state.running ? "Pause" : "Start";
  if (state.remainingMs !== undefined) {{
    remaining.textContent = state.overrun
      ? `${{formatDuration(state.remainingMs)}} over`
      : `${{formatDuration(state.remainingMs)}} left`;
  }}
}}

const ended = (status) => status === "finished" || status === "abandoned";

function describeSlide(slide) {{
  if (slide.verdict === "skipped") return `${{formatSpan(slide.actualMs)}} · skipped`;
  if (slide.budgetMs === undefined) {{
    return `${{formatSpan(slide.actualMs)}} · no budget`;
  }}
  return (
    `${{formatSpan(slide.actualMs)}} / ${{formatSpan(slide.budgetMs)}}` +
    ` · ${{formatDelta(slide.deltaMs)}}`
  );
}}

function paintReport(report) {{
  rehearsalAdvice.textContent = report.advice;
  rehearsalTotal.textContent =
    report.totals.deltaMs === undefined
      ? `${{formatSpan(report.totals.actualMs)}} recorded · no complete budget`
      : (
          `${{formatSpan(report.totals.actualMs)}} / ` +
          `${{formatSpan(report.totals.budgetMs)}} · ` +
          formatDelta(report.totals.deltaMs)
        );

  rehearsalSlidesList.replaceChildren(
    ...report.slides.map((slide) => {{
      const item = document.createElement("li");
      item.dataset.slidxVerdict = slide.verdict;
      item.textContent = `Slide ${{slide.index}} — ${{describeSlide(slide)}}`;
      return item;
    }}),
  );
  rehearsalReport.hidden = false;
}}

function paintRehearsal() {{
  const state = rehearsal.state();
  const active = state.status === "recording" || state.status === "paused";
  const unavailable =
    rehearsal.persistence() === "unavailable" ? " · not saved across slides" : "";

  rehearse.textContent =
    state.status === "recording"
      ? "Pause rehearsal"
      : ended(state.status)
        ? "New rehearsal"
        : state.status === "paused"
          ? "Resume rehearsal"
          : "Rehearse";
  finishRehearsal.hidden = !active;
  abandonRehearsal.hidden = !active;
  // The rehearsal buttons own the same clock while recording. Disabling the
  // generic controls prevents the two measurements from drifting apart.
  toggle.disabled = active;
  reset.disabled = active;
  rehearsalStatus.textContent =
    state.status === "idle"
      ? `Ready${{unavailable}}`
      : `${{state.status}} · ${{formatSpan(state.elapsedMs)}}${{unavailable}}`;
}}

toggle.addEventListener("click", () => {{ timer.toggle(); paint(); }});
reset.addEventListener("click", () => {{ timer.reset(); paint(); }});

rehearse.addEventListener("click", () => {{
  const state = rehearsal.state();
  if (state.status === "recording") {{
    rehearsal.pause();
    timer.pause();
  }} else {{
    if (ended(state.status)) {{
      rehearsal.reset();
      timer.reset();
      rehearsalReport.hidden = true;
    }}
    if (rehearsal.state().status === "idle") timer.reset();
    rehearsal.start();
    timer.start();
  }}
  paint();
  paintRehearsal();
}});

finishRehearsal.addEventListener("click", () => {{
  timer.pause();
  paintReport(rehearsal.finish());
  paint();
  paintRehearsal();
}});

abandonRehearsal.addEventListener("click", () => {{
  timer.pause();
  paintReport(rehearsal.abandon());
  paint();
  paintRehearsal();
}});

newRehearsal.addEventListener("click", () => {{
  rehearsal.reset();
  timer.reset();
  rehearsalReport.hidden = true;
  paint();
  paintRehearsal();
}});

addEventListener("pagehide", () => {{
  if (rehearsal.state().status !== "idle") rehearsal.checkpoint();
}});

// Repainting on an interval is display only — the timer derives its own value
// from the clock, so a missed frame costs a frame rather than a second.
setInterval(() => {{ paint(); paintRehearsal(); }}, 250);
paint();
paintRehearsal();
if (ended(openingRehearsal.status)) paintReport(rehearsal.report());

// Slide one's presenter lives at the deck root and the rest live one directory
// down, so `slide + 1` is a path for every slide except the first — and the
// first is the one a speaker reaches for when they need to start over.
const hrefFor = (slide, step) => {{
  const path = slide === 0 ? `${{root}}presenter/` : `${{root}}${{slide + 1}}/presenter/`;
  return step === undefined ? path : `${{path}}?step=${{step}}`;
}};

// The presenter page does not render the slide, so its stage only counts.
// Every rule about clamping and about when a step becomes a slide change
// stays in the navigator, where it is already tested.
const opening = new URLSearchParams(location.search).get("step");

const deck = createNavigator({{
  stage: createStopCursor({stops}),
  slide: {index},
  slideCount: {count},
  step: opening === null ? undefined : Number(opening),
  hrefFor,
}});

// This is the window the speaker is looking at, so this is the window a
// clicker sends its keys to. Without this the deck can only be driven by
// focusing the projector, which is on the other screen.
addEventListener("keydown", (event) => deck.handleKey(event));

const stopLabel = document.querySelector("[data-slidx-stop]");
const paintStop = () => {{
  if (stopLabel && {stops} > 1) stopLabel.textContent = `stop ${{deck.step + 1}} of {stops}`;
}};

deck.subscribe((position) => {{
  paintStop();
  mirror.send(position);
}});

// And follow the projector when it is driven from there instead.
mirror.subscribe((position) => {{
  if (position.slide !== {index}) {{
    location.href = hrefFor(position.slide, position.step === 0 ? undefined : position.step);
    return;
  }}
  deck.show(position);
  paintStop();
}});

mirror.send({{ slide: {index}, step: deck.step }});
paintStop();
"#,
        runtime_src = runtime_src,
        rehearsal_src = rehearsal_src,
        rehearsal_slides = rehearsal_slides(deck),
        slide_id = serde_json::to_string(&slide.id).expect("slide ids are JSON strings"),
        budget = deck
            .meta
            .duration_seconds
            .map(|seconds| (u64::from(seconds) * 1000).to_string())
            .unwrap_or_else(|| "undefined".to_string()),
        index = slide.index,
        count = deck.slides.len(),
        stops = slide.timeline.frames().len(),
    )
}

fn rehearsal_slides(deck: &Deck) -> String {
    let slides = deck
        .slides
        .iter()
        .map(|slide| RehearsalSlide {
            id: &slide.id,
            budget_ms: slide.budget_seconds.map(|seconds| u64::from(seconds) * 1000),
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&slides).expect("rehearsal slides are JSON")
}
