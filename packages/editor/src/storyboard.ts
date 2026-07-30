/**
 * The deck as a talk: one message per slide, laid against the clock.
 *
 * A grid of thumbnails answers "what does slide 14 look like", which is rarely
 * the question. The questions a speaker has are "what am I saying here", "does
 * this fit in twenty minutes", and "what do I cut if I am behind" — so this
 * surface answers those three and draws no picture of any slide. The canvas is
 * already the one answer about appearance, and a second one rendered a different
 * way would be a second source of truth about layout.
 *
 * # A mode rather than a panel
 *
 * It covers the editor while it is open. Working out where a talk's time goes is
 * a whole-deck job that wants the whole width, and the two numbers it edits —
 * the notes and the budget — are exactly the fields the outline and the canvas
 * cannot show you all of at once.
 *
 * Closed, it draws nothing at all: rebuilding sixty rows on every keystroke
 * somewhere else in the editor is work nobody asked for.
 *
 * # Everything here is an operation
 *
 * Reordering is `moveSlide`, so the slides that move keep the bytes they already
 * had and a reordered deck diffs as moved lines. The budget goes to the file as
 * the text the author typed, never as a number this side parsed: `budget:`
 * accepts `90`, `90s`, `1m30s` and `1:30`, and reading those in a browser would
 * be the project's second duration parser. One gesture is one operation, which
 * is one press of undo.
 */

import { element, fill } from "./dom";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import type { EditorState } from "./session";
import { formatSeconds, planTime, type Plan } from "./storyboard/plan";
import { slideRows } from "./storyboard/rows";
import { timeBar } from "./storyboard/strip";
import { applyStoryboardStyles } from "./storyboard/styles";

export interface StoryboardHandlers {
  select(slide: number): void;
  /**
   * Awaited, unlike the other surfaces' — a reorder has to move the selection
   * to where the slide landed, and the answer to the operation is what resets
   * it. Without that, holding Alt and an arrow key moves a different slide on
   * the second press.
   */
  run(op: EditOp): void | Promise<void>;
}

export function createStoryboard(handlers: StoryboardHandlers): Surface {
  applyStoryboardStyles(document);

  const launch = element(
    "button",
    { type: "button", class: "slidx-sb-launch", "aria-expanded": "false" },
    ["Storyboard"],
  );
  const close = element("button", { type: "button", class: "slidx-sb-close" }, ["Close"]);
  const summary = element("p", { class: "slidx-sb-summary" });
  const slack = element("p", { class: "slidx-sb-slack" });
  const untimed = element("p", { class: "slidx-sb-untimed" });
  const bar = element("div", { class: "slidx-sb-bar-holder" });
  const rows = element("div", { class: "slidx-sb-rows", role: "list" });

  const sheet = element(
    "section",
    { class: "slidx-sb-sheet", "aria-label": "Storyboard", tabindex: -1, hidden: true },
    [
      element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["Storyboard"]), close]),
      element("div", { class: "slidx-sb-plan" }, [summary, bar, slack, untimed]),
      rows,
    ],
  );

  const root = element("div", { class: "slidx-storyboard" }, [launch, sheet]);

  let latest: EditorState | undefined;
  let showing = false;
  let dragging: number | undefined;

  /** One operation, then the selection follows the slide it moved. */
  async function move(from: number, to: number): Promise<void> {
    await handlers.run({ op: "moveSlide", slide: from, to });
    handlers.select(to);
  }

  function draw(state: EditorState): void {
    const plan = planTime(state.slides, state.durationSeconds);
    const at = state.selection.slide;

    summary.textContent = summarise(plan);
    slack.textContent = slackOf(plan);
    untimed.textContent = untimedOf(plan);
    fill(bar, [timeBar(plan, at, handlers.select)]);
    fill(
      rows,
      slideRows(plan.slides, at, {
        select: handlers.select,
        run: (op) => void handlers.run(op),
        lift: (from) => {
          dragging = from;
        },
        drop: (into) => {
          if (dragging === undefined) return;

          // A gap is counted between slides and `moveSlide` counts its
          // destination after the slide has been lifted out, so every gap below
          // the slide names a position one lower than it looks.
          const from = dragging;
          const to = into > from ? into - 1 : into;
          dragging = undefined;

          if (to !== from) void move(from, to);
        },
      }),
    );
  }

  function show(): void {
    showing = true;
    launch.setAttribute("aria-expanded", "true");
    sheet.removeAttribute("hidden");
    if (latest) draw(latest);
    sheet.focus();
  }

  function hide(): void {
    showing = false;
    launch.setAttribute("aria-expanded", "false");
    sheet.setAttribute("hidden", "true");
    fill(rows, []);
    fill(bar, []);
    launch.focus();
  }

  launch.addEventListener("click", () => (showing ? hide() : show()));
  close.addEventListener("click", hide);
  sheet.addEventListener("keydown", (event) => {
    if (latest) keyed(event, latest, handlers, { hide, focus: () => sheet.focus(), move });
  });

  return {
    root,
    render(state) {
      latest = state;
      if (showing) draw(state);
    },
  };
}

interface KeyActions {
  hide(): void;
  focus(): void;
  move(from: number, to: number): Promise<void>;
}

/**
 * The keyboard equivalent of every gesture on the sheet.
 *
 * Bound on the sheet rather than on the document, so the storyboard cannot take
 * a key from a surface that is not open. Nothing but Escape is bound while a
 * field has focus: `o` is a letter before it is a shortcut, and a tool that
 * steals keys from the box an author is typing in is a tool they fight.
 */
function keyed(
  event: KeyboardEvent,
  state: EditorState,
  handlers: StoryboardHandlers,
  actions: KeyActions,
): void {
  const at = state.selection.slide;
  const last = state.slides.length - 1;

  if (event.key === "Escape") {
    event.preventDefault();
    // Leaving a field commits it, here as everywhere else in this editor, so
    // Escape out of one goes back to the sheet rather than throwing the edit
    // away. Pressing it again closes.
    if (typing(event.target)) actions.focus();
    else actions.hide();
    return;
  }

  if (typing(event.target)) return;

  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const to = at + (event.key === "ArrowDown" ? 1 : -1);
    if (to < 0 || to > last) return;

    if (event.altKey) void actions.move(at, to);
    else handlers.select(to);
    return;
  }

  if (event.key === "o") {
    event.preventDefault();
    const slide = state.slides[at];
    if (slide) handlers.run({ op: "setField", slide: at, key: "optional", value: !slide.optional });
  }
}

function typing(target: EventTarget | null): boolean {
  return target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement;
}

/**
 * The deck against its slot, in one sentence.
 *
 * Numbers and no verdict. Whether a total is too long, or too thin, is the
 * linter's judgement and it is already reported two panels down; a second
 * opinion here would eventually be a different one.
 */
function summarise(plan: Plan): string {
  const planned = formatSeconds(plan.plannedSeconds);

  if (plan.slotSeconds === undefined) {
    return `${planned} planned, and no \`duration:\` on the deck to lay it against.`;
  }

  const slot = `${planned} planned against a ${formatSeconds(plan.slotSeconds)} slot`;

  if (plan.overSeconds > 0) return `${slot}, ${formatSeconds(plan.overSeconds)} over.`;
  if (plan.spareSeconds > 0) return `${slot}, ${formatSeconds(plan.spareSeconds)} spare.`;

  return `${slot}, exactly full.`;
}

/**
 * What is prepared to be cut.
 *
 * The point of `optional:` is that the answer to "what do I drop" exists before
 * the talk rather than being invented during it, so this says what dropping them
 * would actually buy — and says when it would not be enough.
 */
function slackOf(plan: Plan): string {
  const marked = plan.slides.filter((slide) => slide.optional).length;

  if (plan.overSeconds === 0) {
    if (marked === 0) return "";

    return `${formatSeconds(plan.slackSeconds)} of that is marked optional, so there is that much to drop if you fall behind.`;
  }

  if (marked === 0) return "Nothing is marked optional, so there is nothing prepared to cut.";

  const left = plan.plannedSeconds - plan.slackSeconds;
  const slides = marked === 1 ? "the slide" : `the ${marked} slides`;
  const then =
    left <= (plan.slotSeconds ?? 0)
      ? "which fits"
      : `still ${formatSeconds(left - (plan.slotSeconds ?? 0))} over`;

  return `Dropping ${slides} marked optional brings it to ${formatSeconds(left)}, ${then}.`;
}

/**
 * How much of the deck nothing accounts for.
 *
 * A slide with no budget and no notes contributes nothing to the bar, so without
 * this the bar would quietly describe a shorter talk than the one being written.
 */
function untimedOf(plan: Plan): string {
  if (plan.untimed === 0) return "";

  return plan.untimed === 1
    ? "1 slide has no message and no budget, so nothing here accounts for it."
    : `${plan.untimed} slides have no message and no budget, so nothing here accounts for them.`;
}
