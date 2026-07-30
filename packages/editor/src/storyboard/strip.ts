/**
 * The time budget, drawn as width.
 *
 * This is the whole point of the storyboard: a speaker should be able to *see*
 * their twenty minutes rather than compute it. The band underneath is the slot,
 * the blocks on top are the slides, and a deck that does not fit carries on past
 * where the band stops. No arithmetic is done here — [`plan`](./plan) already
 * did it — so the only thing that can be wrong is the picture.
 *
 * Optional slides are drawn in the muted colour rather than the accent, because
 * that is what they are: slack. The coloured part of the bar is the talk a
 * speaker is committed to, and the grey part is what is left if they need it.
 */

import { element } from "../dom";
import { formatSeconds, type Plan, type SlideTime } from "./plan";

/** A width, short enough to read in a style attribute. */
function percent(share: number): string {
  return `${Number((share * 100).toFixed(4))}%`;
}

/**
 * The bar, or a sentence saying why there is nothing to draw.
 *
 * A deck with no budgets, no notes and no slot has no length at all, and a
 * zero-height bar would read as a broken one.
 */
export function timeBar(plan: Plan, selected: number, jump: (slide: number) => void): HTMLElement {
  if (plan.empty) {
    return element("p", { class: "slidx-sb-hint" }, [
      "Nothing here has a budget or a message yet, so there is no length to draw.",
    ]);
  }

  const children: HTMLElement[] = [
    element("div", { class: "slidx-sb-track", style: `width: ${percent(plan.slotShare ?? 1)}` }),
    element(
      "div",
      { class: "slidx-sb-segments" },
      plan.slides
        .filter((slide) => slide.seconds > 0)
        .map((slide) => segment(slide, selected, jump)),
    ),
  ];

  // Hatched over whatever is past the slot, because a segment is opaque: the
  // band ending underneath it is not something a reader can see, and "the last
  // two slides are outside your twenty minutes" is the whole point of the
  // picture.
  if (plan.overSeconds > 0 && plan.slotShare !== undefined) {
    children.push(
      element("div", {
        class: "slidx-sb-overrun",
        style: `left: ${percent(plan.slotShare)}`,
        "aria-hidden": "true",
      }),
    );
  }

  // The label sits inside the band, to the left of the line that ends it, so it
  // has somewhere to go whether the slot ends at the far right or a third of
  // the way along.
  if (plan.slotSeconds !== undefined && plan.slotShare !== undefined) {
    children.push(
      element("div", { class: "slidx-sb-slot", style: `left: ${percent(plan.slotShare)}` }, [
        element("span", {}, [formatSeconds(plan.slotSeconds)]),
      ]),
    );
  }

  return element(
    "div",
    { class: "slidx-sb-bar", "data-slot": plan.slotSeconds === undefined ? "none" : "declared" },
    children,
  );
}

/**
 * One slide's share of the bar.
 *
 * A button rather than a block, because the bar is also the fastest way to
 * reach the slide that is taking too long.
 */
function segment(slide: SlideTime, selected: number, jump: (slide: number) => void): HTMLElement {
  const node = element("button", {
    type: "button",
    class: "slidx-sb-segment",
    style: `width: ${percent(slide.share)}`,
    "data-source": slide.source,
    "data-optional": slide.optional,
    "aria-current": slide.index === selected,
    title: `${slide.index + 1}. ${slide.title} — ${formatSeconds(slide.seconds)} ${
      slide.source === "budget" ? "budgeted" : "from notes"
    }`,
  });

  node.addEventListener("click", () => jump(slide.index));

  return node;
}
