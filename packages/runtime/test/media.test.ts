/**
 * Video and audio on a slide.
 *
 * This is the specification for the failure everyone has watched happen: a
 * speaker plays a clip and the room hears nothing, or the room hears it at a
 * volume that makes people flinch. Both are unrecoverable in the moment —
 * you cannot re-play a clip to an audience that has already reacted.
 *
 * So the behaviours here are mostly about *before*:
 *
 * - The level is measured while the deck is still on an earlier slide, and
 *   reported, so a speaker can fix it before anyone hears it.
 * - Playback is normalised against a target so one loud clip in a deck of
 *   quiet ones does not arrive at full scale.
 * - Media never autoplays with sound, because browsers refuse it and a slide
 *   that silently did nothing is worse than one that shows a play button.
 *
 * Everything is injected: no real `<video>`, no real AudioContext, no sleeps.
 */

import { describe, expect, it, vi } from "vitest";

import {
  createMediaController,
  describeLevel,
  LOUDNESS_TARGET_DB,
  type MediaElementLike,
} from "../src/media";

function element(overrides: Partial<MediaElementLike> = {}): MediaElementLike {
  return {
    src: "clip.mp4",
    volume: 1,
    muted: false,
    paused: true,
    duration: 12,
    play: vi.fn(async () => undefined),
    pause: vi.fn(() => undefined),
    ...overrides,
  };
}

/** A meter that reports a fixed peak, standing in for real analysis. */
function meter(peakDb: number) {
  return vi.fn(async () => ({ peakDb, integratedDb: peakDb - 6 }));
}

describe("measuring before anyone hears it", () => {
  it("measures a clip without playing it aloud", async () => {
    // The whole point: the level is known while the deck is still on an
    // earlier slide, so the speaker can fix it before the room reacts.
    const media = element();
    const controller = createMediaController({ measure: meter(-3) });

    await controller.inspect(media);

    expect(media.play).not.toHaveBeenCalled();
  });

  it("reports a clip that will arrive too loud", async () => {
    const controller = createMediaController({ measure: meter(-0.5) });
    const report = await controller.inspect(element());

    expect(report.status).toBe("too-loud");
    expect(report.remedy).toBeTruthy();
  });

  it("reports a clip nobody at the back will hear", async () => {
    const controller = createMediaController({ measure: meter(-40) });
    const report = await controller.inspect(element());

    expect(report.status).toBe("too-quiet");
    expect(report.remedy).toBeTruthy();
  });

  it("passes a clip that is already at a sensible level", async () => {
    const controller = createMediaController({ measure: meter(LOUDNESS_TARGET_DB) });
    const report = await controller.inspect(element());

    expect(report.status).toBe("ok");
  });

  it("says so rather than guessing when it cannot measure", async () => {
    // A codec the browser will not decode, or a cross-origin file. A false
    // "ok" here is worse than no answer, because the speaker stops checking.
    const controller = createMediaController({
      measure: vi.fn(async () => {
        throw new Error("cannot decode");
      }),
    });

    const report = await controller.inspect(element());

    expect(report.status).toBe("unknown");
    expect(report.remedy).toMatch(/play it once/i);
  });
});

describe("playing it", () => {
  it("normalises towards the target rather than playing at full scale", async () => {
    // One loud clip in a deck of quiet ones is the common case, and it is the
    // one that makes a room flinch.
    const media = element();
    const controller = createMediaController({ measure: meter(-3) });

    await controller.prepare(media);

    expect(media.volume).toBeLessThan(1);
  });

  it("leaves a quiet clip alone rather than amplifying it", async () => {
    // Raising the gain raises the noise floor with it. A quiet clip is a
    // problem to fix in the file, which is what the report says.
    const media = element();
    const controller = createMediaController({ measure: meter(-40) });

    await controller.prepare(media);

    expect(media.volume).toBe(1);
  });

  it("never autoplays with sound", async () => {
    // Browsers refuse it, and a slide that silently did nothing is worse than
    // one that shows a play button.
    const media = element();
    const controller = createMediaController({ measure: meter(-12) });

    await controller.prepare(media);

    expect(media.play).not.toHaveBeenCalled();
  });

  it("stops a clip when its slide is left", async () => {
    // Audio continuing over the next slide is the second-worst thing a deck
    // can do to a speaker, after not playing at all.
    const media = element({ paused: false });
    const controller = createMediaController({ measure: meter(-12) });

    controller.release(media);

    expect(media.pause).toHaveBeenCalled();
  });

  it("is safe to release something already stopped", () => {
    const media = element({ paused: true });
    const controller = createMediaController({ measure: meter(-12) });

    expect(() => controller.release(media)).not.toThrow();
  });
});

describe("saying it to a person", () => {
  it("describes a level in words rather than decibels", () => {
    // "-0.5 dBFS" means nothing to most speakers two minutes before a talk.
    expect(describeLevel(-0.5)).toMatch(/loud/i);
    expect(describeLevel(-40)).toMatch(/quiet/i);
    expect(describeLevel(LOUDNESS_TARGET_DB)).toMatch(/fine|good|ok/i);
  });

  it("still gives the number, for anyone who wants it", () => {
    expect(describeLevel(-0.5)).toContain("-0.5");
  });
});
