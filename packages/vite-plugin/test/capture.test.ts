/**
 * The retry has to be narrow enough to stay useful.
 *
 * A retry that swallows everything turns a broken build into a slow broken
 * build, and a test suite that goes green by being run enough times is not
 * evidence of anything. These pin both edges: the one documented transient
 * fault is retried, and nothing else is.
 */

import { describe, expect, it } from "vite-plus/test";

import { capture } from "../src/capture";

const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47]);

/** Fails `times` times with `message`, then succeeds. */
function flaky(times: number, message: string) {
  let called = 0;

  return {
    get calls() {
      return called;
    },
    take: async () => {
      called += 1;
      if (called <= times) throw new Error(message);
      return PNG;
    },
  };
}

describe("capture", () => {
  it("returns the first answer when the browser gives one", async () => {
    const subject = flaky(0, "");

    expect(await capture(subject.take)).toBe(PNG);
    expect(subject.calls).toBe(1);
  });

  it("asks again when the frame was gone before the capture landed", async () => {
    // The exact message macOS CI produced, three times in one afternoon.
    const subject = flaky(
      2,
      "page.screenshot: Protocol error (Page.captureScreenshot): Unable to capture screenshot",
    );

    expect(await capture(subject.take)).toBe(PNG);
    expect(subject.calls).toBe(3);
  });

  it("gives up rather than asking forever", async () => {
    const subject = flaky(99, "Unable to capture screenshot");

    await expect(capture(subject.take)).rejects.toThrow("Unable to capture screenshot");
    expect(subject.calls).toBe(3);
  });

  it("does not retry a failure that will fail the same way", async () => {
    // A selector that found nothing is permanent. Retrying it costs three
    // browser round trips and reports the same thing at the end.
    const subject = flaky(99, "locator.screenshot: Timeout 30000ms exceeded");

    await expect(capture(subject.take)).rejects.toThrow("Timeout");
    expect(subject.calls).toBe(1);
  });

  it("throws the browser's own message, so a real breakage reads as it did", async () => {
    const subject = flaky(99, "Unable to capture screenshot: the renderer went away");

    await expect(capture(subject.take)).rejects.toThrow("the renderer went away");
  });
});
