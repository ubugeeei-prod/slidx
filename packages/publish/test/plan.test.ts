/**
 * The plan.
 *
 * This is the specification for the thing this package exists to produce, and
 * every claim below is about publishing being reviewable before it is done.
 * Publishing is a set of one-way operations across services that do not agree
 * on what an edit means, carried out by someone who has just come off stage —
 * so the plan has to be readable, complete, and comparable with the last one.
 *
 * The failure modes guarded here:
 *
 * - One missing field stopping the rest of the plan. The value of a plan is
 *   highest when the deck is least ready.
 * - A plan that differs between runs, which cannot be diffed and therefore
 *   cannot be reviewed. No clock, no filesystem, no dependence on the order
 *   the caller happened to pass slides in.
 * - A blocked step that says a target is unavailable rather than saying which
 *   frontmatter key to add.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  blockedSteps,
  formatPlan,
  isReady,
  planPublish,
  PUBLISH_TARGETS,
  readySteps,
  type PublishPlan,
} from "../src";
import { PDF, slide, TALK, without } from "./support";

const SLIDES = [
  slide(0, { title: "Why plain HTML", notes: ["A deck is a document."] }),
  slide(1, { notes: ["The docs: https://slidx.dev/docs"] }),
];

/** A deck with everything: metadata, a built PDF, notes, and a link. */
function complete(): PublishPlan {
  return planPublish({ meta: TALK, artifacts: [PDF], slides: SLIDES });
}

function targetsOf(plan: PublishPlan): string[] {
  return plan.steps.map((step) => step.target);
}

describe("what gets planned", () => {
  it("plans every destination, in the order the work happens", () => {
    // The uploads first: the URL they produce is what the post links to.
    expect(targetsOf(complete())).toEqual([...PUBLISH_TARGETS]);
  });

  it("plans a requested subset", () => {
    const plan = planPublish({ meta: TALK, slides: SLIDES, targets: ["blog"] });

    expect(targetsOf(plan)).toEqual(["blog"]);
  });

  it("ignores the order a subset was asked in", () => {
    // Two people planning the same deck get the same plan, whichever way they
    // typed the flags.
    const plan = planPublish({
      meta: TALK,
      artifacts: [PDF],
      targets: ["resources", "speakerdeck"],
    });

    expect(targetsOf(plan)).toEqual(["speakerdeck", "resources"]);
  });

  it("plans a target once however often it is listed", () => {
    const plan = planPublish({ meta: TALK, slides: SLIDES, targets: ["blog", "blog"] });

    expect(targetsOf(plan)).toEqual(["blog"]);
  });

  it("plans nothing when nothing is asked for", () => {
    expect(planPublish({ meta: TALK, targets: [] }).steps).toEqual([]);
  });

  it("names the deck, for the header of a printed plan", () => {
    expect(complete().deck).toBe("Zero-JavaScript Slides");
  });

  it("stands in for a deck with no title, rather than printing nothing", () => {
    expect(planPublish({ meta: {} }).deck).toBe("Untitled deck");
  });
});

describe("ready steps", () => {
  it("carries the destination's own payload", () => {
    const step = complete().steps.find((entry) => entry.target === "speakerdeck");

    expect(step?.status).toBe("ready");
    expect(step?.status === "ready" && step.target === "speakerdeck" && step.payload.pdf).toBe(
      "dist/deck.pdf",
    );
  });

  it("summarises what would happen in one line", () => {
    const step = complete().steps.find((entry) => entry.target === "social");

    expect(step?.summary).toContain("character post");
  });

  it("hands Cloudflare Pages a file rather than a login", () => {
    const step = complete().steps.find((entry) => entry.target === "cloudflare");

    expect(step?.status).toBe("ready");
    expect(
      step?.status === "ready" &&
        step.target === "cloudflare" &&
        step.payload.command === "wrangler pages deploy" &&
        step.payload.toml.includes("pages_build_output_dir"),
    ).toBe(true);
  });

  it("is ready only when every step is", () => {
    expect(isReady(complete())).toBe(true);
  });

  it("is not ready when there is nothing to do", () => {
    // An empty plan publishes nothing, which is not the same as being ready.
    expect(isReady(planPublish({ meta: TALK, targets: [] }))).toBe(false);
  });
});

describe("blocked steps", () => {
  it("does not stop the targets that are fine", () => {
    // A deck built without a PDF can still have its post composed, which is
    // the whole reason blocking is per step.
    const plan = planPublish({ meta: TALK, slides: SLIDES, artifacts: [] });

    expect(blockedSteps(plan).map((step) => step.target)).toEqual(["speakerdeck", "docswell"]);
    expect(readySteps(plan).map((step) => step.target)).toEqual([
      "social",
      "blog",
      "resources",
      "cloudflare",
      "archive",
    ]);
  });

  it("names the fields to add in its summary", () => {
    const plan = planPublish({ meta: TALK, slides: SLIDES, artifacts: [] });

    expect(blockedSteps(plan)[0]?.summary).toBe("needs pdf");
  });

  it("names each field once, however many reasons mention it", () => {
    // Both too many tags and one that is too long, which is two reasons about
    // a single line of frontmatter.
    const tags = Array.from({ length: 21 }, (_, index) => `${"a".repeat(25)}-${index}`);
    const plan = planPublish({ meta: { ...TALK, tags }, artifacts: [PDF], targets: ["docswell"] });

    expect(blockedSteps(plan)[0]?.reasons).toHaveLength(2);
    expect(blockedSteps(plan)[0]?.summary).toBe("needs tags");
  });

  it("carries a message that says what to write", () => {
    const plan = planPublish({ meta: without(TALK, "url"), targets: ["social"] });

    expect(blockedSteps(plan)[0]?.reasons[0]?.message).toContain("`url:`");
  });

  it("makes the whole plan not ready", () => {
    expect(isReady(planPublish({ meta: TALK, slides: SLIDES, artifacts: [] }))).toBe(false);
  });

  it("blocks the write-ups a bare deck cannot produce, and nothing else", () => {
    const plan = planPublish({ meta: TALK, artifacts: [PDF] });

    expect(blockedSteps(plan).map((step) => step.target)).toEqual(["blog"]);
  });
});

describe("determinism", () => {
  it("plans the same deck identically, key order included", () => {
    // Key order matters: a plan is diffed as text, and a field that moves
    // between runs is a diff that says nothing.
    expect(JSON.stringify(complete())).toBe(JSON.stringify(complete()));
  });

  it("does not depend on the order the slides arrived in", () => {
    const forwards = planPublish({ meta: TALK, artifacts: [PDF], slides: SLIDES });
    const backwards = planPublish({ meta: TALK, artifacts: [PDF], slides: [...SLIDES].reverse() });

    expect(JSON.stringify(backwards)).toBe(JSON.stringify(forwards));
  });

  it("leaves the caller's arrays alone", () => {
    // Frozen, so sorting them in place would throw rather than pass quietly.
    const slides = Object.freeze([...SLIDES].reverse());
    const artifacts = Object.freeze([PDF]);

    expect(() => planPublish({ meta: TALK, slides, artifacts })).not.toThrow();
    expect(slides[0]?.index).toBe(1);
  });

  it("plans two equal decks equally, whoever built the objects", () => {
    // A plan is a function of the deck and nothing else — no clock, no
    // filesystem, no state left over from the last call.
    const other = planPublish({
      meta: { ...TALK },
      artifacts: [{ ...PDF }],
      slides: SLIDES.map((entry) => ({ ...entry })),
    });

    expect(other).toEqual(complete());
  });
});

describe("printing", () => {
  it("heads the plan with the deck and ends it with a count", () => {
    const lines = formatPlan(complete()).split("\n");

    expect(lines[0]).toBe("publish plan: Zero-JavaScript Slides");
    expect(lines.at(-1)).toBe("7 ready, 0 blocked");
  });

  it("counts what is blocked", () => {
    const plan = planPublish({ meta: TALK, slides: SLIDES, artifacts: [] });

    expect(formatPlan(plan).split("\n").at(-1)).toBe("5 ready, 2 blocked");
  });

  it("prints a blocked step's reasons under it", () => {
    const plan = planPublish({ meta: without(TALK, "url"), targets: ["social"] });

    expect(formatPlan(plan)).toContain("add `url:`");
  });

  it("keeps its columns still between plans of different sizes", () => {
    // A two-step plan has to line up against a five-step one, or a diff shows
    // what moved instead of what changed.
    const whole = formatPlan(complete()).split("\n");
    const part = formatPlan(
      planPublish({ meta: TALK, artifacts: [PDF], slides: SLIDES, targets: ["blog"] }),
    ).split("\n");

    const line = (lines: string[]) => lines.find((entry) => entry.includes("blog"));

    expect(line(part)).toBe(line(whole));
  });

  it("prints the same text for the same deck", () => {
    expect(formatPlan(complete())).toBe(formatPlan(complete()));
  });
});

describe("options", () => {
  it("passes a shorter budget through to the post", () => {
    const plan = planPublish({ meta: TALK, targets: ["social"], social: { limit: 120 } });
    const step = plan.steps[0];

    expect(step?.status === "ready" && step.target === "social" && step.payload.limit).toBe(120);
  });

  it("plans without any artifacts at all", () => {
    const plan = planPublish({ meta: TALK, slides: SLIDES });

    expect(readySteps(plan).map((step) => step.target)).toEqual([
      "social",
      "blog",
      "resources",
      "cloudflare",
      "archive",
    ]);
  });
});
