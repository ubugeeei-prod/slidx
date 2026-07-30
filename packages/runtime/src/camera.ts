/**
 * The speaker's camera, filling the tile the deck declared.
 *
 * The feature is two opt-ins and refuses to work on one of them.
 *
 * **The author's**, in the file: `camera: side` puts an empty tile in a named
 * region of the slide. That is all the build emits — no video element, no
 * script, nothing that could reach a device. A published deck is a static page
 * somebody opens from a link, a QR code, or an archive years later, and a page
 * that asks for a webcam is a page people close.
 *
 * **The speaker's**, at presentation time: this module is reached from
 * `enterPresentation`, which happens on an explicit gesture and nowhere else.
 * A page nobody is presenting from never calls it, so a reader is never asked.
 *
 * `startCamera` on a document with no tile in it returns without touching
 * `getUserMedia` at all. The gate is structural rather than remembered: there
 * is no order of calls that opens a camera on a deck whose author did not place
 * one.
 *
 * # Failure is ordinary
 *
 * No camera, permission refused, and a camera another application is already
 * holding all happen minutes before a talk. None of them is exceptional and
 * none of them may cost the deck, so nothing here rejects: the tile ends up
 * saying what went wrong, in the speaker's words, on the slide they are looking
 * at. A tile reading "no camera found" is better than a black rectangle, and
 * far better than an exception thrown into a slide.
 *
 * # Nothing leaves the machine
 *
 * The stream reaches exactly one place — the `<video>` element this module puts
 * in the tile — and there is no way to get it back out. That is a camera on the
 * speaker's own screen, which is what the audience is already looking at, and
 * not a broadcast.
 */

/** Attribute naming the region a tile occupies. Mirrors `slidx_core::camera`. */
export const CAMERA_ATTRIBUTE = "data-slidx-camera";

/** Attribute carrying what the camera is doing. Mirrors `slidx_core::camera`. */
export const CAMERA_STATE_ATTRIBUTE = "data-slidx-camera-state";

/**
 * What the camera actually is, which is rarely what was asked for.
 *
 * Every value except `live` leaves the deck presentable. They are separate
 * rather than one `failed` because the speaker's next action differs: a refused
 * permission is a browser setting, a busy device is an application to quit, and
 * an absent one is nothing they can do anything about in the two minutes they
 * have.
 */
export type CameraStatus =
  /** The deck asked for none, or nobody started one. */
  | "off"
  | "live"
  /** The speaker, or the browser's stored decision, said no. */
  | "denied"
  /** No camera on this machine. */
  | "absent"
  /** Something else has it — usually the conferencing app for the same talk. */
  | "busy"
  /** This browser has no camera API to ask. */
  | "unsupported";

/** What presentation mode got, and how to give it back. */
export interface CameraSession {
  readonly status: CameraStatus;
  /** One sentence, written for a speaker rather than for a log. */
  readonly detail: string;
  /** Releases the device. Safe to call twice. */
  stop(): void;
}

/** Only the part of a stream this module can use, so a test can supply one. */
export interface MediaStreamLike {
  getTracks(): { stop(): void }[];
}

/**
 * Injected so this is testable, and so a missing API is a value not a crash.
 *
 * One capability, deliberately. There is nowhere for a stream to go except the
 * element on the slide.
 */
export interface CameraEnvironment {
  /** Absent when the browser has no camera API at all. */
  requestStream?: () => Promise<MediaStreamLike>;
}

/** The real browser, for callers that are not a test. */
export function browserCameraEnvironment(view: Window = globalThis.window): CameraEnvironment {
  const devices = (view.navigator as Navigator & { mediaDevices?: MediaDevices }).mediaDevices;

  // Spread rather than assigned `undefined`, so a browser without the API
  // genuinely leaves the key off and `startCamera` reports `unsupported`
  // instead of failing somewhere less legible.
  return {
    ...(devices === undefined
      ? {}
      : {
          // Video only. A talk's audio comes from the room's microphone, and
          // asking for a device this feature does not use is asking a speaker
          // to grant a permission for nothing.
          requestStream: () => devices.getUserMedia({ video: true, audio: false }),
        }),
  };
}

/** A session that holds nothing, for every path that never opened a device. */
function idle(status: CameraStatus, detail: string): CameraSession {
  return { status, detail, stop: () => {} };
}

/**
 * Fills the tile this document declares, if it declares one.
 *
 * Never rejects. Called from `enterPresentation`, alongside the wake lock and
 * fullscreen, and reports what it got the same way they do.
 */
export async function startCamera(
  root: Document,
  environment: CameraEnvironment,
): Promise<CameraSession> {
  const tile = root.querySelector(`[${CAMERA_ATTRIBUTE}]`);

  // The author's opt-in, enforced before anything else can happen. A deck with
  // no camera in it has no path from here to a device.
  if (tile === null) return idle("off", "this slide places no camera");

  if (!environment.requestStream) {
    return show(tile, "unsupported", "this browser has no camera API");
  }

  let stream: MediaStreamLike;
  try {
    stream = await environment.requestStream();
  } catch (error) {
    const [status, detail] = describe(error);
    return show(tile, status, detail);
  }

  const video = root.createElement("video");
  video.autoplay = true;
  // Muted and inline are not preferences. An unmuted self-view feeds the room's
  // own speakers back into the room, and a browser will refuse to autoplay it
  // anyway — so the tile would sit on a play button in front of an audience.
  video.muted = true;
  video.playsInline = true;
  // The DOM types `srcObject` as the platform's whole `MediaStream`. Taking
  // that interface here would make the module untestable without a device, and
  // the only thing it ever does with a stream is stop its tracks.
  video.srcObject = stream as unknown as MediaProvider;

  tile.replaceChildren(video);
  tile.setAttribute(CAMERA_STATE_ATTRIBUTE, "live");

  let stopped = false;

  return {
    status: "live",
    detail: "the camera is on the slide",

    stop() {
      // Exiting twice is normal: a keyboard shortcut, Escape, and the browser's
      // own fullscreen control all land in `exit`. A track stopped twice is
      // harmless, but a tile emptied twice would clear a later session's video.
      if (stopped) return;
      stopped = true;

      for (const track of stream.getTracks()) track.stop();

      // Back to the state the build emitted, which draws as nothing. Left as
      // `live` the slide would keep a frozen last frame of the speaker on it.
      tile.replaceChildren();
      tile.setAttribute(CAMERA_STATE_ATTRIBUTE, "idle");
    },
  };
}

/**
 * Puts the reason on the slide and hands back a session that holds nothing.
 *
 * The speaker is looking at the slide, so that is where this belongs. `slidx
 * doctor` is the other half and the better one — it says the same thing before
 * anyone is on stage — but a permission can be refused after the pre-flight and
 * a camera can be taken by a conferencing app in the minute between them.
 */
function show(tile: Element, status: CameraStatus, detail: string): CameraSession {
  const message = tile.ownerDocument.createElement("p");
  message.className = "slidx-camera-status";
  message.textContent = detail;

  tile.replaceChildren(message);
  tile.setAttribute(CAMERA_STATE_ATTRIBUTE, status);

  return idle(status, detail);
}

/**
 * What a refusal actually was.
 *
 * The names are the ones `getUserMedia` rejects with, and each maps to a
 * different next action. Anything unrecognised is `unsupported` rather than
 * `denied`: telling a speaker to go and change a permission that was never the
 * problem costs them the minute they had.
 */
function describe(error: unknown): [CameraStatus, string] {
  const name = error instanceof Error ? error.name : "";

  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
      return ["denied", "camera permission was refused for this page"];
    case "NotFoundError":
    case "OverconstrainedError":
      return ["absent", "no camera found on this machine"];
    case "NotReadableError":
    case "AbortError":
      return ["busy", "another application is using the camera"];
    default:
      return ["unsupported", "the camera could not be started"];
  }
}
