/**
 * The speaker's camera, and the two opt-ins it needs.
 *
 * The specification these tests hold is short and the first line matters most:
 * a deck somebody opens from a link must never be asked for a webcam. That is
 * checked here as "the device is never even requested", because a permission
 * dialog dismissed is still a permission dialog shown.
 *
 * The rest is about the minute before a talk. No camera, a refused permission,
 * a device the conferencing app for the same talk is already holding — none of
 * these is exceptional, all of them happen, and each has to leave the deck
 * presentable with the reason on screen.
 */

import { describe, expect, it, vi } from "vite-plus/test";

import {
  CAMERA_ATTRIBUTE,
  CAMERA_STATE_ATTRIBUTE,
  startCamera,
  type MediaStreamLike,
} from "../src/camera";

/** A document with a camera tile in it, as `slidx_render` emits one. */
function declared(): Document {
  document.body.innerHTML = `<figure class="slidx-camera" ${CAMERA_ATTRIBUTE}="side" ${CAMERA_STATE_ATTRIBUTE}="idle"></figure>`;
  return document;
}

/** A document with no camera anywhere in it. The common case, by a long way. */
function undeclared(): Document {
  document.body.innerHTML = `<div class="slidx-region"><h1>Ordinary</h1></div>`;
  return document;
}

/**
 * A stream the DOM will accept, with tracks this test can watch being stopped.
 *
 * Built on the real prototype rather than as a plain object: `srcObject` is
 * type-checked at runtime by browsers and by happy-dom alike, and something
 * merely stream-shaped is refused.
 */
function fakeStream(): { stream: MediaStreamLike; stopped: () => number } {
  let stopped = 0;
  const tracks = [{ stop: () => (stopped += 1) }, { stop: () => (stopped += 1) }];

  const stream = Object.create(MediaStream.prototype) as MediaStreamLike;
  stream.getTracks = () => tracks;

  return { stream, stopped: () => stopped };
}

/** A rejection shaped like the ones `getUserMedia` actually produces. */
function refusal(name: string) {
  const error = new Error(name);
  error.name = name;

  return vi.fn(async () => {
    throw error;
  });
}

function tile(): Element {
  const found = document.querySelector(`[${CAMERA_ATTRIBUTE}]`);
  if (!found) throw new Error("no tile");
  return found;
}

describe("the author's opt-in, which is the file", () => {
  it("never asks for a device on a deck that placed no camera", () => {
    // The whole constraint, in one assertion. A published deck is a page
    // strangers open, and a page that prompts for a webcam is a page they
    // close. There is no ordering of calls that gets past this.
    const requestStream = vi.fn();

    return startCamera(undeclared(), { requestStream }).then((session) => {
      expect(requestStream).not.toHaveBeenCalled();
      expect(session.status).toBe("off");
    });
  });

  it("says the slide places no camera rather than claiming a failure", async () => {
    // "off" and "denied" are different sentences. A presenter view that showed
    // the second for a deck that never asked would send a speaker to a browser
    // setting that was never the problem.
    const session = await startCamera(undeclared(), { requestStream: vi.fn() });

    expect(session.detail).toMatch(/places no camera/i);
  });
});

describe("the speaker's opt-in, which is presentation time", () => {
  it("fills the declared tile with what the browser gave it", async () => {
    const { stream } = fakeStream();
    const session = await startCamera(declared(), { requestStream: async () => stream });

    expect(session.status).toBe("live");
    expect(tile().querySelector("video")).not.toBeNull();
    expect(tile().getAttribute(CAMERA_STATE_ATTRIBUTE)).toBe("live");
  });

  it("mutes the self-view, because the room already has the speaker's voice", async () => {
    // An unmuted self-view feeds the room's speakers back into the room, and a
    // browser refuses to autoplay it anyway — so the tile would sit on a play
    // button in front of an audience.
    const { stream } = fakeStream();
    await startCamera(declared(), { requestStream: async () => stream });
    const video = tile().querySelector("video");

    expect(video?.muted).toBe(true);
    expect(video?.autoplay).toBe(true);
  });

  it("gives the device back when presentation mode ends", async () => {
    // A camera left running is a light on the laptop after the speaker sat
    // down, and a device the next application cannot open.
    const { stream, stopped } = fakeStream();
    const session = await startCamera(declared(), { requestStream: async () => stream });

    session.stop();

    expect(stopped()).toBe(2);
    expect(tile().querySelector("video")).toBeNull();
    expect(tile().getAttribute(CAMERA_STATE_ATTRIBUTE)).toBe("idle");
  });

  it("is safe to stop twice", async () => {
    // Escape, the keyboard shortcut and the browser's own fullscreen control
    // all reach `exit`, which reaches this.
    const { stream, stopped } = fakeStream();
    const session = await startCamera(declared(), { requestStream: async () => stream });

    session.stop();
    session.stop();

    expect(stopped()).toBe(2);
  });
});

describe("failure, which is ordinary", () => {
  it("leaves the deck presentable whatever the camera does", async () => {
    // Every one of these happens in the ten minutes before a talk. Not one of
    // them may throw into a slide.
    for (const name of [
      "NotAllowedError",
      "NotFoundError",
      "NotReadableError",
      "OverconstrainedError",
      "SecurityError",
      "AbortError",
      "SomethingNobodyHasSeen",
    ]) {
      const session = await startCamera(declared(), { requestStream: refusal(name) });

      expect(session.status, name).not.toBe("live");
      expect(session.detail, name).toBeTruthy();
    }
  });

  it("tells the speaker which failure it was, because the next action differs", async () => {
    // A refused permission is a browser setting, a busy device is an
    // application to quit, and an absent one is nothing anybody can do in the
    // two minutes they have. One "failed" would be useless to all three.
    const cases: [string, string][] = [
      ["NotAllowedError", "denied"],
      ["NotFoundError", "absent"],
      ["NotReadableError", "busy"],
    ];

    for (const [name, status] of cases) {
      const session = await startCamera(declared(), { requestStream: refusal(name) });
      expect(session.status, name).toBe(status);
    }
  });

  it("puts the reason on the slide the speaker is looking at", async () => {
    // A tile that says "no camera found" is better than a black rectangle, and
    // the slide is where the speaker's eyes already are.
    await startCamera(declared(), { requestStream: refusal("NotFoundError") });

    expect(tile().textContent).toMatch(/no camera/i);
    expect(tile().getAttribute(CAMERA_STATE_ATTRIBUTE)).toBe("absent");
  });

  it("names the conferencing app problem in the speaker's terms", async () => {
    // The single most likely one: the app for the same hybrid talk has the
    // camera. "NotReadableError" tells a speaker nothing.
    const session = await startCamera(declared(), { requestStream: refusal("NotReadableError") });

    expect(session.detail).toMatch(/another application/i);
  });

  it("reports a browser with no camera API rather than pretending", async () => {
    const session = await startCamera(declared(), {});

    expect(session.status).toBe("unsupported");
    expect(tile().textContent).toBeTruthy();
  });
});

describe("nothing leaves the machine", () => {
  it("hands the stream to the slide and to nothing else", async () => {
    // A camera on the speaker's own screen, not a broadcast. The session has no
    // way to give the stream to a caller, which is what makes that structural
    // rather than a promise in a comment.
    const { stream } = fakeStream();
    const session = await startCamera(declared(), { requestStream: async () => stream });

    expect(Object.keys(session).sort()).toEqual(["detail", "status", "stop"]);
  });
});
