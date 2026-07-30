//! Behaviour inlined into the presenter page.
//!
//! Kept apart from the presenter document and its layout so adding a control
//! does not turn one renderer into a script, a stylesheet, and a template
//! tangled together. The output stays inline: that is what lets a presenter
//! page remain usable from a `file://` fallback.

use slidx_core::{Deck, Slide};

pub(crate) fn render(deck: &Deck, slide: &Slide, runtime_src: &str) -> String {
    format!(
        r#"import {{
  createTimer,
  formatDuration,
  createMirror,
  createNavigator,
  createStopCursor,
}} from "{runtime_src}";

const budgetMs = {budget};
const timer = createTimer({{ budgetMs }});
const mirror = createMirror();

const clock = document.querySelector("[data-slidx-clock]");
const elapsed = document.querySelector("[data-slidx-elapsed]");
const remaining = document.querySelector("[data-slidx-remaining]");
const toggle = document.querySelector('[data-slidx-action="toggle"]');

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

toggle.addEventListener("click", () => {{ timer.toggle(); paint(); }});
document
  .querySelector('[data-slidx-action="reset"]')
  .addEventListener("click", () => {{ timer.reset(); paint(); }});

// Repainting on an interval is display only — the timer derives its own value
// from the clock, so a missed frame costs a frame rather than a second.
setInterval(paint, 250);
paint();

// Slide one's presenter lives at the deck root and the rest live one directory
// down, so `slide + 1` is a path for every slide except the first — and the
// first is the one a speaker reaches for when they need to start over.
const root = {index} === 0 ? "../" : "../../";
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
