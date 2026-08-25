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
 * what makes two plans comparable, and it is what `slidx publish --plan` reads
 * out before anybody means it.
 */

import {
  ask,
  source,
  type Artifact,
  type DeckMetadata,
  type DeckSlide,
  type PublishPlan,
  type PublishStep,
  type PublishTarget,
  type SocialOptions,
} from "./boundary";

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
 *
 * Written out rather than fetched because a value is not a literal type, and
 * these names are how a caller spells one. `satisfies` catches a name that is
 * not a destination; the order is pinned by the test that plans a whole deck
 * and compares the steps against this list.
 */
export const PUBLISH_TARGETS = [
  "speakerdeck",
  "docswell",
  "social",
  "blog",
  "resources",
  "cloudflare",
  "archive",
] as const satisfies readonly PublishTarget[];

/** A step that has everything it needs, and the payload to prove it. */
export type ReadyStep = Extract<PublishStep, { status: "ready" }>;

/** A step that cannot run, and the fields that would unblock it. */
export type BlockedStep = Extract<PublishStep, { status: "blocked" }>;

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
  return ask<PublishPlan>({
    op: "plan",
    ...source(options),
    social: options.social ?? {},
    // Absent rather than `undefined`: leaving the key out is how the planner
    // is told "all of them", and an explicit `undefined` is a different
    // sentence in a language that has both.
    ...(options.targets === undefined ? {} : { targets: [...options.targets] }),
  });
}

/**
 * The two filters below stay on this side of the boundary on purpose.
 *
 * A filter on a tag is not a decision — `status` already says which a step is,
 * and there is nothing for two implementations to disagree about. `isReady` is
 * a decision, because an empty plan publishes nothing and that is not the same
 * as being ready, so it is asked rather than answered here.
 */
export function readySteps(plan: PublishPlan): ReadyStep[] {
  return plan.steps.filter((step): step is ReadyStep => step.status === "ready");
}

export function blockedSteps(plan: PublishPlan): BlockedStep[] {
  return plan.steps.filter((step): step is BlockedStep => step.status === "blocked");
}

/** True when nothing is blocked. An empty plan is not ready: it does nothing. */
export function isReady(plan: PublishPlan): boolean {
  return ask<boolean>({ op: "isReady", plan });
}

/**
 * The plan as text, for printing and for diffing against the last one.
 *
 * Fixed column widths, taken from the full target list rather than from what
 * this plan happens to contain, so a plan with two steps lines up against a
 * plan with five and a diff shows what changed rather than what moved.
 */
export function formatPlan(plan: PublishPlan): string {
  return ask<string>({ op: "formatPlan", plan });
}

export type { PublishPlan, PublishStep, PublishTarget } from "./boundary";
