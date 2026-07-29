/**
 * The switch from a dead demo to its recording.
 *
 * The specification is mostly about what the switch is *not* allowed to do at
 * the moment it is used, because that moment is the worst one available: the
 * network is gone, the room is watching, and the speaker has one key.
 *
 * So these tests pin three things that are easy to regress into something that
 * works on a laptop with Wi-Fi and fails on stage:
 *
 * - The recording is fetched at load, never at the switch.
 * - Switching is an attribute write, so nothing about it can reject or throw.
 * - A page with nothing to switch to reports that instead of pretending.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { createDemoSwitch, DEMO_ATTRIBUTE } from "../src/demo";

/**
 * The markup `slidx_render` emits for a declared demo.
 *
 * The live side points at `about:blank` rather than a plausible URL: happy-dom
 * really does fetch an iframe's `src`, and a suite that resolves DNS is a suite
 * that fails in CI on the day the sandbox has no network.
 */
function mount(options: { fallback?: boolean; side?: string } = {}): HTMLElement {
  const fallback = options.fallback ?? true;

  document.body.innerHTML = `
    <figure class="slidx-demo" ${DEMO_ATTRIBUTE}="${options.side ?? "live"}">
      <iframe class="slidx-demo-live" src="about:blank"></iframe>
      ${fallback ? `<video class="slidx-demo-fallback" src="./checkout.mp4" preload="auto" muted></video>` : ""}
    </figure>
  `;

  return document.body.querySelector(".slidx-demo") as HTMLElement;
}

/** happy-dom has no media pipeline, so the parts we drive are stubbed. */
function stubVideo(readyState = 0): {
  play: ReturnType<typeof vi.fn>;
  pause: ReturnType<typeof vi.fn>;
  load: ReturnType<typeof vi.fn>;
  video: HTMLVideoElement;
} {
  const video = document.querySelector("video") as HTMLVideoElement;
  const play = vi.fn(() => Promise.resolve());
  const pause = vi.fn();
  const load = vi.fn();

  video.play = play as unknown as HTMLVideoElement["play"];
  video.pause = pause;
  video.load = load;
  Object.defineProperty(video, "readyState", { value: readyState, configurable: true });

  return { play, pause, load, video };
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("finding a demo on the page", () => {
  it("reports no switch on a slide that declares no demo", () => {
    document.body.innerHTML = "<h1>An ordinary slide</h1>";
    expect(createDemoSwitch(document)).toBeNull();
  });

  it("reports no switch when the deck declared a demo but recorded no fallback", () => {
    // The linter already told the author at their desk. On stage the key does
    // nothing rather than blanking the live demo they still have.
    mount({ fallback: false });
    expect(createDemoSwitch(document)).toBeNull();
  });

  it("starts on whichever side the markup shipped with", () => {
    mount();
    stubVideo();
    expect(createDemoSwitch(document)?.side()).toBe("live");
  });
});

describe("preloading the recording", () => {
  it("asks for the recording at load, because the switch is too late to fetch", () => {
    mount();
    const { load } = stubVideo(0);

    createDemoSwitch(document);
    expect(load).toHaveBeenCalled();
  });

  it("leaves a recording that already has data alone", () => {
    mount();
    const { load } = stubVideo(4);

    createDemoSwitch(document);
    expect(load).not.toHaveBeenCalled();
  });

  it("reports honestly that the recording is not loaded yet", () => {
    mount();
    stubVideo(0);
    expect(createDemoSwitch(document)?.ready()).toBe(false);
  });

  it("reports the recording as ready once it holds a frame to show", () => {
    mount();
    stubVideo(2);
    expect(createDemoSwitch(document)?.ready()).toBe(true);
  });
});

describe("switching sides", () => {
  it("switches to the recording with one call and nothing else", () => {
    const figure = mount();
    stubVideo(4);

    createDemoSwitch(document)?.toggle();
    expect(figure.getAttribute(DEMO_ATTRIBUTE)).toBe("fallback");
  });

  it("switches back to the live demo, for a demo that came back up", () => {
    const figure = mount();
    stubVideo(4);
    const demo = createDemoSwitch(document);

    demo?.toggle();
    demo?.toggle();
    expect(figure.getAttribute(DEMO_ATTRIBUTE)).toBe("live");
    expect(demo?.side()).toBe("live");
  });

  it("plays the recording as it appears, so one key is the whole gesture", () => {
    mount();
    const { play } = stubVideo(4);

    createDemoSwitch(document)?.show("fallback");
    expect(play).toHaveBeenCalled();
  });

  it("stops the recording when the live demo comes back, rather than leaving it running", () => {
    mount();
    const { pause } = stubVideo(4);
    const demo = createDemoSwitch(document);

    demo?.show("fallback");
    demo?.show("live");
    expect(pause).toHaveBeenCalled();
  });

  it("does not restart a recording that is already the side on screen", () => {
    // A second press of the key mid-recording would otherwise jump back to the
    // frame the speaker just talked past.
    mount();
    const { play } = stubVideo(4);
    const demo = createDemoSwitch(document);

    demo?.show("fallback");
    demo?.show("fallback");
    expect(play).toHaveBeenCalledTimes(1);
  });

  it("still switches when the browser refuses to autoplay", () => {
    // Autoplay policy rejects, and a rejection that escaped would be an
    // unhandled error thrown into the slide at the worst possible moment.
    const figure = mount();
    const { video } = stubVideo(4);
    video.play = (() => Promise.reject(new Error("NotAllowedError"))) as HTMLVideoElement["play"];

    expect(() => createDemoSwitch(document)?.show("fallback")).not.toThrow();
    expect(figure.getAttribute(DEMO_ATTRIBUTE)).toBe("fallback");
  });

  it("switches even when the recording never finished loading", () => {
    // A partly-loaded recording is still better than the dead demo it replaces.
    const figure = mount();
    stubVideo(0);

    createDemoSwitch(document)?.toggle();
    expect(figure.getAttribute(DEMO_ATTRIBUTE)).toBe("fallback");
  });
});
