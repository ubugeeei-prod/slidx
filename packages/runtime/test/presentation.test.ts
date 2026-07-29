/**
 * Entering presentation mode.
 *
 * This is the specification for what happens when a speaker says "I am about
 * to start". The honest shape of the feature is a split:
 *
 * - What a **browser** can do: hold a wake lock so the screen does not sleep,
 *   go fullscreen, and stop asking for notification permission.
 * - What a browser **cannot** do: silence the operating system. No web API
 *   turns on Do Not Disturb, and none ever will — a page that could mute your
 *   machine would be a page that could hide a phishing alert.
 *
 * So the second half is *told*, not done. The tests below hold that line: the
 * checklist is part of the contract, and anything that quietly pretended to
 * mute the OS would be worse than saying nothing.
 *
 * Everything is injected, because none of these APIs exist in a test
 * environment and half of them do not exist in Safari either.
 */

import { describe, expect, it, vi } from "vitest";

import { enterPresentation, presentationChecklist } from "../src/presentation";

function environment(overrides: Record<string, unknown> = {}) {
  const released = vi.fn();

  return {
    requestWakeLock: vi.fn(async () => ({ release: released })),
    requestFullscreen: vi.fn(async () => undefined),
    exitFullscreen: vi.fn(async () => undefined),
    platform: "macos" as const,
    released,
    ...overrides,
  };
}

describe("what the browser can actually do", () => {
  it("holds a wake lock so the screen does not sleep mid-talk", async () => {
    // A screen that dims during a long slide is the failure this prevents,
    // and it happens exactly when a speaker is talking rather than clicking.
    const env = environment();
    await enterPresentation(env);

    expect(env.requestWakeLock).toHaveBeenCalled();
  });

  it("goes fullscreen", async () => {
    const env = environment();
    await enterPresentation(env);

    expect(env.requestFullscreen).toHaveBeenCalled();
  });

  it("releases the lock and leaves fullscreen when it ends", async () => {
    // A wake lock left held keeps a laptop awake in a bag.
    const env = environment();
    const session = await enterPresentation(env);
    await session.exit();

    expect(env.released).toHaveBeenCalled();
    expect(env.exitFullscreen).toHaveBeenCalled();
  });

  it("carries on when the wake lock is refused", async () => {
    // Safari refuses it outside a user gesture, and some browsers have none.
    // A missing wake lock costs a dimmed screen; throwing costs the talk.
    const env = environment({
      requestWakeLock: vi.fn(async () => {
        throw new Error("denied");
      }),
    });

    const session = await enterPresentation(env);

    expect(session.wakeLock).toBe(false);
    expect(env.requestFullscreen).toHaveBeenCalled();
  });

  it("carries on when fullscreen is refused", async () => {
    const env = environment({
      requestFullscreen: vi.fn(async () => {
        throw new Error("denied");
      }),
    });

    const session = await enterPresentation(env);
    expect(session.fullscreen).toBe(false);
  });

  it("reports what it managed to get", async () => {
    // The presenter view shows this, so a speaker knows whether to go and
    // change a setting themselves.
    const session = await enterPresentation(environment());

    expect(session.wakeLock).toBe(true);
    expect(session.fullscreen).toBe(true);
  });

  it("is safe to exit twice", async () => {
    const env = environment();
    const session = await enterPresentation(env);

    await session.exit();
    await session.exit();

    expect(env.released).toHaveBeenCalledTimes(1);
  });
});

describe("what the browser cannot do, and says so", () => {
  it("tells the speaker to silence the machine itself", () => {
    // No web API turns on Do Not Disturb, and none should: a page that could
    // mute your machine could hide a phishing alert.
    const items = presentationChecklist("macos");

    expect(items.some((item) => /do not disturb|focus/i.test(item.title))).toBe(true);
  });

  it("names the actual setting for the platform", async () => {
    // "Turn on Do Not Disturb" is useless if you cannot find it. Each item
    // says where.
    const mac = presentationChecklist("macos");
    const windows = presentationChecklist("windows");

    expect(mac.find((item) => /focus/i.test(item.title))?.where).toMatch(/Control Cent/i);
    expect(windows.find((item) => /focus/i.test(item.title))?.where).toMatch(/Notifications/i);
  });

  it("covers the failures that actually happen on stage", () => {
    const titles = presentationChecklist("macos").map((item) => item.title.toLowerCase());

    for (const expected of ["notification", "display", "sleep"]) {
      expect(
        titles.some((title) => title.includes(expected)),
        expected,
      ).toBe(true);
    }
  });

  it("still says something useful on an unknown platform", () => {
    // A Linux laptop or a kiosk browser should not get an empty checklist.
    expect(presentationChecklist("unknown").length).toBeGreaterThan(0);
  });

  it("gives every item somewhere to go", () => {
    // A checklist item a speaker cannot act on is a checklist item that
    // wastes the two minutes before a talk.
    for (const platform of ["macos", "windows", "linux", "unknown"] as const) {
      for (const item of presentationChecklist(platform)) {
        expect(item.where, `${platform}: ${item.title}`).toBeTruthy();
      }
    }
  });

  it("is the same list every time it is asked", () => {
    expect(presentationChecklist("macos")).toEqual(presentationChecklist("macos"));
  });
});
