/**
 * What publishing would do, before any of it is done.
 *
 * Planning is separated from performing for the same reason a build has a dry
 * run: publishing is a set of one-way operations, spread across services that
 * do not agree on what an edit means, carried out by someone who has just come
 * off stage. A plan is the thing that can be read, diffed against last time,
 * and argued with while everything is still reversible.
 *
 * A plan is data. Every step is either *ready*, carrying a complete payload for
 * its destination, or *blocked*, carrying reasons that name the field to add.
 * One missing field never stops the rest of the plan being produced — the whole
 * point is to learn everything that is wrong in one pass rather than one
 * failure at a time.
 *
 * The same deck plans the same way every time: no clock, no filesystem, no
 * network, no iteration order that depends on anything but the deck. That is
 * what makes two plans comparable, and it is why this module has no
 * dependencies at all.
 */

import {
  composeArchive,
  composeBlog,
  composeDocswell,
  composeResources,
  composeSocial,
  composeSpeakerDeck,
  describeArchive,
  describeBlog,
  describeDocswell,
  describeResources,
  describeSocial,
  describeSpeakerDeck,
  type ArchiveRecord,
  type BlogScaffold,
  type DocswellUpload,
  type ResourcesPage,
  type SocialOptions,
  type SocialPost,
  type SpeakerDeckUpload,
} from "./targets";
import type { Artifact, BlockedReason, DeckMetadata, DeckSlide, DeckSource } from "./types";

/**
 * Every destination, in the order a plan lists them.
 *
 * The order is the order the work happens in: the uploads first, because the
 * URL they produce is what the post links to, and the written pages last. A
 * caller asking for a subset gets it in this order regardless of how they
 * asked, so two people planning the same deck get the same plan.
 *
 * `archive` is last because it is the only one that will be run again. It
 * records what the others produced, and it is re-run months later when the
 * conference finally publishes the video.
 */
export const PUBLISH_TARGETS = [
  "speakerdeck",
  "docswell",
  "social",
  "blog",
  "resources",
  "archive",
] as const;

export type PublishTarget = (typeof PUBLISH_TARGETS)[number];

/** A step that has everything it needs, and the payload to prove it. */
export type ReadyStep =
  | ReadyStepOf<"speakerdeck", SpeakerDeckUpload>
  | ReadyStepOf<"docswell", DocswellUpload>
  | ReadyStepOf<"social", SocialPost>
  | ReadyStepOf<"blog", BlogScaffold>
  | ReadyStepOf<"resources", ResourcesPage>
  | ReadyStepOf<"archive", ArchiveRecord>;

interface ReadyStepOf<Target extends PublishTarget, Payload> {
  status: "ready";
  target: Target;
  /** One line, for a printed plan. */
  summary: string;
  payload: Payload;
}

/** A step that cannot run, and the fields that would unblock it. */
export interface BlockedStep {
  status: "blocked";
  target: PublishTarget;
  summary: string;
  reasons: BlockedReason[];
}

export type PublishStep = ReadyStep | BlockedStep;

export interface PublishPlan {
  /** The deck's title, or a stand-in, for the plan's header line. */
  deck: string;
  steps: PublishStep[];
}

export interface PlanOptions {
  meta: DeckMetadata;
  /** In any order; everything derived per slide is sorted by index. */
  slides?: readonly DeckSlide[];
  /** What the build produced. Absent is normal, and is reported per target. */
  artifacts?: readonly Artifact[];
  /** A subset to plan. Defaults to all of {@link PUBLISH_TARGETS}. */
  targets?: readonly PublishTarget[];
  social?: SocialOptions;
}

/** Plans every requested target, in {@link PUBLISH_TARGETS} order. */
export function planPublish(options: PlanOptions): PublishPlan {
  const source: DeckSource = {
    meta: options.meta,
    slides: [...(options.slides ?? [])],
    artifacts: [...(options.artifacts ?? [])],
  };

  const requested = new Set(options.targets ?? PUBLISH_TARGETS);
  const steps = PUBLISH_TARGETS.filter((target) => requested.has(target)).map((target) =>
    planStep(target, source, options),
  );

  return { deck: source.meta.title?.trim() || "Untitled deck", steps };
}

/**
 * One step.
 *
 * Written out per target rather than driven by a table: the payload type is
 * what makes a ready step worth having, and a table that produced them all
 * would have to erase it to have one signature.
 */
function planStep(target: PublishTarget, source: DeckSource, options: PlanOptions): PublishStep {
  switch (target) {
    case "speakerdeck": {
      const result = composeSpeakerDeck(source);
      return result.ok
        ? {
            status: "ready",
            target,
            summary: describeSpeakerDeck(result.value),
            payload: result.value,
          }
        : blockedStep(target, result.reasons);
    }
    case "docswell": {
      const result = composeDocswell(source);
      return result.ok
        ? {
            status: "ready",
            target,
            summary: describeDocswell(result.value),
            payload: result.value,
          }
        : blockedStep(target, result.reasons);
    }
    case "social": {
      const result = composeSocial(source, options.social ?? {});
      return result.ok
        ? { status: "ready", target, summary: describeSocial(result.value), payload: result.value }
        : blockedStep(target, result.reasons);
    }
    case "blog": {
      const result = composeBlog(source);
      return result.ok
        ? { status: "ready", target, summary: describeBlog(result.value), payload: result.value }
        : blockedStep(target, result.reasons);
    }
    case "resources": {
      const result = composeResources(source);
      return result.ok
        ? {
            status: "ready",
            target,
            summary: describeResources(result.value),
            payload: result.value,
          }
        : blockedStep(target, result.reasons);
    }
    case "archive": {
      const result = composeArchive(source);
      return result.ok
        ? { status: "ready", target, summary: describeArchive(result.value), payload: result.value }
        : blockedStep(target, result.reasons);
    }
  }
}

/** The summary names the fields, because that is what the author acts on. */
function blockedStep(target: PublishTarget, reasons: BlockedReason[]): BlockedStep {
  const fields = [...new Set(reasons.map((entry) => entry.field))];
  return { status: "blocked", target, summary: `needs ${fields.join(", ")}`, reasons };
}

export function readySteps(plan: PublishPlan): ReadyStep[] {
  return plan.steps.filter((step): step is ReadyStep => step.status === "ready");
}

export function blockedSteps(plan: PublishPlan): BlockedStep[] {
  return plan.steps.filter((step): step is BlockedStep => step.status === "blocked");
}

/** True when nothing is blocked. An empty plan is not ready: it does nothing. */
export function isReady(plan: PublishPlan): boolean {
  return plan.steps.length > 0 && plan.steps.every((step) => step.status === "ready");
}

/** Widest target name, so the columns do not move between plans. */
const TARGET_COLUMN = Math.max(...PUBLISH_TARGETS.map((target) => target.length));

/**
 * The plan as text, for printing and for diffing against the last one.
 *
 * Fixed column widths, taken from the full target list rather than from what
 * this plan happens to contain, so a plan with two steps lines up against a
 * plan with five and a diff shows what changed rather than what moved.
 */
export function formatPlan(plan: PublishPlan): string {
  const lines = [`publish plan: ${plan.deck}`, ""];

  for (const step of plan.steps) {
    const target = step.target.padEnd(TARGET_COLUMN);
    lines.push(`  ${step.status.padEnd(7)} ${target}  ${step.summary}`);

    if (step.status === "blocked") {
      for (const entry of step.reasons)
        lines.push(`${" ".repeat(TARGET_COLUMN + 12)}${entry.message}`);
    }
  }

  const ready = readySteps(plan).length;
  lines.push("", `${ready} ready, ${plan.steps.length - ready} blocked`);

  return lines.join("\n");
}
