//! Behaviour inlined into the presenter page.
//!
//! Kept apart from the presenter document and its layout so adding a control
//! does not turn one renderer into a script, a stylesheet, and a template
//! tangled together. The output stays inline: that is what lets a presenter
//! page remain usable from a `file://` fallback.

use serde::Serialize;
use slidx_core::{Deck, Slide};

/// One slide, as the two readers on this page need it.
///
/// One array rather than two. Both the rehearsal session and pacing want a
/// slide's budget, and two payloads carrying it would be two answers to the
/// same question about the same slide — the drift this project spends its
/// architecture avoiding. Each reader takes the fields it knows and ignores
/// the rest.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresenterSlide<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u64>,
    /// Left out unless true, because pacing reads its absence as "not optional"
    /// and a `false` on every slide is bytes that say nothing.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    optional: bool,
}

pub(crate) fn render(
    deck: &Deck,
    slide: &Slide,
    runtime_src: &str,
    presenter_runtime_src: &str,
    rehearsal_src: &str,
) -> String {
    format!(
        r#"import {{
  createMirror,
  createNavigator,
  createStopCursor,
}} from "{runtime_src}";
import {{
  assessPace,
  createTimer,
  describePace,
  detectPlatform,
  enterPresentation,
  formatDuration,
  presentationChecklist,
}} from "{presenter_runtime_src}";
import {{
  formatDelta,
  formatSpan,
  openRehearsalSession,
  trackRehearsals,
}} from "{rehearsal_src}";

// Slide one's presenter lives at the deck root and the rest live one directory
// down. Resolving that root produces one stable storage key across every page.
const root = {index} === 0 ? "../" : "../../";
const budgetMs = {budget};
const deckSlides = {presenter_slides};
const rehearsalSlideId = {slide_id};

// Pacing addresses a slide by position and the rehearsal session by id, so the
// index is added here rather than serialised twice.
const paceSlides = deckSlides.map((slide, index) => ({{ ...slide, index }}));

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
  slides: deckSlides,
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
const pace = document.querySelector("[data-slidx-pace]");
const present = document.querySelector('[data-slidx-action="present"]');
const presentPanel = document.querySelector("[data-slidx-present]");
const presentState = document.querySelector("[data-slidx-present-state]");
const presentList = document.querySelector("[data-slidx-present-checklist]");
const rehearsalStatus = document.querySelector("[data-slidx-rehearsal-status]");
const rehearsalReport = document.querySelector("[data-slidx-rehearsal-report]");
const rehearsalAdvice = document.querySelector("[data-slidx-rehearsal-advice]");
const rehearsalTrend = document.querySelector("[data-slidx-rehearsal-trend]");
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

  // The clock answers how long the speaker has been talking. This answers
  // whether the time left is enough for the slides left, which is the question
  // they actually have — and `describePace` returns "" when there is no slot to
  // measure against, so a deck without one shows a blank rather than a guess.
  const reading = assessPace({{
    slides: paceSlides,
    position: {index},
    elapsedMs: state.elapsedMs,
    budgetMs,
    running: state.running,
  }});
  pace.dataset.slidxPaceState = reading.pace;
  pace.textContent = describePace(reading);
}}

// Two states rather than one, because they answer to different things. The
// panel is open because the speaker asked; the session exists once a browser
// has answered. A browser that neither grants fullscreen nor refuses it — and
// there are several — would otherwise leave the button inert, which is the
// worst thing this control could be two minutes before a talk.
let presenting = false;
let presentation;

function paintPresentation() {{
  present.textContent = presenting ? "Stop presenting" : "Present";
  present.setAttribute("aria-expanded", String(presenting));
  presentPanel.hidden = !presenting;
  if (!presenting) return;

  presentState.textContent = presentation === undefined ? asking() : reporting(presentation);

  presentList.replaceChildren(
    ...presentationChecklist(detectPlatform(navigator.userAgent)).map((step) => {{
      const item = document.createElement("li");
      const title = document.createElement("span");
      title.className = "slidx-present-item";
      title.textContent = step.title;
      const where = document.createElement("span");
      where.className = "slidx-present-where";
      where.textContent = step.where;
      item.append(title, where);
      return item;
    }}),
  );
}}

const asking = () => "Asking this browser for the screen. What is below is yours either way.";

// What it gave and what it refused, read from the session rather than from what
// was asked for: a wake lock a browser declined is a screen that will dim, and
// a speaker who was told otherwise has stopped checking.
function reporting(session) {{
  const got = [];
  const refused = [];
  (session.fullscreen ? got : refused).push("the whole screen");
  (session.wakeLock ? got : refused).push("the screen kept awake");

  // Assembled from the halves that are there rather than one sentence with a
  // hole in it. A browser that refused everything is the case a speaker most
  // needs to read, and it is the case a template would word worst.
  const gave = got.length === 0 ? "" : `gave ${{got.join(" and ")}}`;
  const denied = refused.length === 0 ? "" : `refused ${{refused.join(" and ")}}`;

  return refused.length === 0
    ? `This browser ${{gave}}. What is left is below, because no web API can do it.`
    : `This browser ${{[gave, denied].filter(Boolean).join(" and ")}} — so that is yours too, along with what is below.`;
}}

present.addEventListener("click", async () => {{
  if (presenting) {{
    const leaving = presentation;
    presenting = false;
    presentation = undefined;
    paintPresentation();
    await leaving?.exit();
    return;
  }}

  presenting = true;
  paintPresentation();

  // Asked for on the gesture that is still live. `enterPresentation` never
  // rejects: a browser that refuses everything still leaves the checklist,
  // which is the half no browser could have done anyway.
  const session = await enterPresentation();

  // The speaker may have pressed it again while a permission prompt was up.
  if (!presenting) {{
    await session.exit();
    return;
  }}

  presentation = session;
  paintPresentation();
}});

// Escape leaves fullscreen without telling anyone. The session already
// releases the wake lock when that happens, so what is left is to stop saying
// on screen that the talk is still in presentation mode.
document.addEventListener("fullscreenchange", () => {{
  if (document.fullscreenElement === null && presentation?.fullscreen) {{
    presenting = false;
    presentation = undefined;
    paintPresentation();
  }}
}});

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

/**
 * One rehearsal says where the time went; three say whether the changes are
 * working. The history is every run that has ended, so the run being painted is
 * already in it and the comparison is against the one before.
 */
function paintTrend() {{
  const trend = trackRehearsals(rehearsal.history());
  rehearsalTrend.textContent = trend.note;

  return new Map(trend.slides.map((slide) => [slide.id, slide]));
}}

function paintReport(report) {{
  const trend = paintTrend();
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

      // The direction is only added where there is one. A slide given for the
      // first time has nothing to compare, and "steady" on every row of a
      // twenty-slide deck is a column of noise around the two that moved.
      const moved = trend.get(slide.id);
      const direction =
        moved === undefined || moved.direction === "new" || moved.direction === "steady"
          ? ""
          : ` · ${{formatSpan(Math.abs(moved.deltaMs))}} ${{moved.direction}} than last time`;

      item.textContent = `Slide ${{slide.index}} — ${{describeSlide(slide)}}${{direction}}`;
      if (moved !== undefined) item.dataset.slidxTrend = moved.direction;
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
        presenter_runtime_src = presenter_runtime_src,
        rehearsal_src = rehearsal_src,
        presenter_slides = presenter_slides(deck),
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

fn presenter_slides(deck: &Deck) -> String {
    let slides = deck
        .slides
        .iter()
        .map(|slide| PresenterSlide {
            id: &slide.id,
            title: slide.title.as_deref(),
            budget_ms: slide.budget_seconds.map(|seconds| u64::from(seconds) * 1000),
            optional: slide.optional,
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&slides).expect("presenter slides are JSON")
}
