/**
 * Entering presentation mode.
 *
 * The feature has an honest split down the middle, and pretending otherwise
 * is the thing to avoid:
 *
 * **What a browser can do** — hold a wake lock so the screen does not dim
 * mid-slide, and go fullscreen.
 *
 * **What a browser cannot do** — silence the operating system. No web API
 * turns on Do Not Disturb, and none should: a page that could mute your
 * machine would be a page that could hide a phishing alert. So the rest is a
 * checklist the speaker acts on, with the setting named and located.
 *
 * A tool that claimed to handle notifications and then let one appear during a
 * demo would be worse than one that said "go and turn this on" — the speaker
 * would have stopped checking.
 *
 * **Why the camera is here.** A webcam needs a live user gesture and belongs to
 * the act of presenting rather than to the deck, which is exactly what this
 * function already is: the one thing a speaker does when they say "I am about
 * to start". Nothing on a built slide calls it, so a deck opened from a link
 * has no path to a camera at all — the gate is the call graph rather than a
 * check somebody has to remember to write.
 */

import { startCamera } from "./camera";
import type { CameraSession, CameraStatus } from "./camera";
import { browserCameraEnvironment } from "./camera";

/** What a platform's settings are called, so a speaker can find them. */
export type Platform = "macos" | "windows" | "linux" | "unknown";

/** Something the speaker has to do, because the browser cannot. */
export interface ChecklistItem {
  title: string;
  /** Where the setting lives on this platform. */
  where: string;
}

/** What presentation mode managed to get, and how to end it. */
export interface PresentationSession {
  /** True when the screen is being kept awake. */
  readonly wakeLock: boolean;
  readonly fullscreen: boolean;
  /**
   * What the camera is, which is `off` unless the deck placed one *and* this
   * machine gave it up. Every other value leaves the deck presentable.
   */
  readonly camera: CameraStatus;
  exit(): Promise<void>;
}

/** Injected so this is testable, and so a missing API is a value not a crash. */
export interface PresentationEnvironment {
  requestWakeLock?: () => Promise<{ release: () => void }>;
  requestFullscreen?: () => Promise<void>;
  exitFullscreen?: () => Promise<void>;
  /**
   * The speaker's own camera. Left off, presentation mode simply never has one
   * — which is what makes this the only door a camera can come through.
   */
  startCamera?: () => Promise<CameraSession>;
  /**
   * Called when the browser leaves fullscreen without being asked (Escape, or
   * the browser's own control). Returns an unsubscribe.
   */
  subscribeFullscreenExit?: (listener: () => void) => () => void;
  platform?: Platform;
}

interface WakeLockSentinelLike {
  release: () => void;
}

/**
 * The real browser, for callers that are not a test.
 *
 * A hook is left off rather than stubbed when the API is missing, so
 * `enterPresentation` reports the capability as absent instead of claiming it.
 */
export function browserPresentationEnvironment(
  view: Window = globalThis.window,
): PresentationEnvironment {
  const page = view.document;
  const root = page.documentElement;
  const wakeLock = (
    view.navigator as Navigator & {
      wakeLock?: { request: (type: "screen") => Promise<WakeLockSentinelLike> };
    }
  ).wakeLock;

  // Spread rather than assigned `undefined`, so a missing API genuinely leaves
  // the key off. `attempt` treats an absent hook and a hook that is present
  // and holds `undefined` the same way, but the type says otherwise, and the
  // paragraph above is only true of one of them.
  return {
    ...(wakeLock === undefined ? {} : { requestWakeLock: () => wakeLock.request("screen") }),
    ...(root.requestFullscreen === undefined
      ? {}
      : { requestFullscreen: () => root.requestFullscreen() }),
    ...(page.exitFullscreen === undefined ? {} : { exitFullscreen: () => page.exitFullscreen() }),

    // Always wired, and still gated twice: `startCamera` finds no tile on a
    // deck whose author placed none, and nothing calls `enterPresentation`
    // except a speaker starting a talk.
    startCamera: () => startCamera(page, browserCameraEnvironment(view)),

    subscribeFullscreenExit: (listener) => {
      // The event fires for entering too, and says nothing about which; the
      // document is the only thing that knows.
      const handler = () => {
        if (!page.fullscreenElement) listener();
      };

      page.addEventListener("fullscreenchange", handler);
      return () => page.removeEventListener("fullscreenchange", handler);
    },

    platform: detectPlatform(view.navigator.userAgent),
  };
}

/** The browser when there is one, and nothing at all when there is not. */
function defaultEnvironment(): PresentationEnvironment {
  return typeof globalThis.window === "undefined" ? {} : browserPresentationEnvironment();
}

/**
 * Takes what the browser will give and reports the rest.
 *
 * Never rejects. Every capability here is optional in some browser a talk will
 * eventually be given in, and a missing wake lock costs a dimmed screen while
 * a thrown error costs the talk.
 */
export async function enterPresentation(
  environment: PresentationEnvironment = defaultEnvironment(),
): Promise<PresentationSession> {
  // All three are asked for before any is awaited: fullscreen and the camera
  // both need the user gesture that is still live, and awaiting the wake lock
  // first can outlast it.
  const [lock, fullscreen, camera] = await Promise.all([
    attempt(environment.requestWakeLock),
    attempt(environment.requestFullscreen),
    attempt(environment.startCamera),
  ]);

  let exited = false;
  let unsubscribe: (() => void) | undefined;

  const session: PresentationSession = {
    wakeLock: lock !== null,
    fullscreen: fullscreen !== null,
    // What it is, not what was asked for. A refused camera costs a camera.
    camera: camera?.status ?? "off",

    async exit() {
      // Exiting twice is normal: a keyboard shortcut, Escape, and the
      // browser's own fullscreen control all land here. Releasing a lock
      // twice throws.
      if (exited) return;
      exited = true;

      unsubscribe?.();
      lock?.release();
      // A camera left running is a light on the speaker's laptop after they
      // sat down, and a device the next application cannot open.
      camera?.stop();
      await attempt(environment.exitFullscreen);
    },
  };

  // Escape leaves fullscreen without telling us, and a wake lock nobody
  // released keeps a laptop awake in a bag.
  unsubscribe = environment.subscribeFullscreenExit?.(() => void session.exit());

  return session;
}

/** Runs a capability that may not exist, and reports absence as `null`. */
async function attempt<T>(operation: (() => Promise<T>) | undefined): Promise<T | null> {
  if (!operation) return null;

  try {
    return await operation();
  } catch {
    return null;
  }
}

/**
 * What the speaker still has to do.
 *
 * Every item names the setting *and* where to find it. "Turn on Do Not
 * Disturb" is useless advice in the two minutes before a talk if you cannot
 * remember which menu it is under.
 */
export function presentationChecklist(platform: Platform = "unknown"): ChecklistItem[] {
  const shared: ChecklistItem[] = [
    {
      title: "Quit anything that shows a notification",
      where:
        "Chat, mail, and calendar clients — quit rather than mute; a badge still steals a glance",
    },
    {
      title: "Set the display to never sleep while presenting",
      where: describe(platform, {
        macos: "System Settings → Lock Screen → Turn display off",
        windows: "Settings → System → Power → Screen and sleep",
        linux: "Settings → Power → Screen Blank",
      }),
    },
  ];

  return [
    {
      title: "Turn on Do Not Disturb (Focus)",
      where: describe(platform, {
        macos: "Control Centre → Focus → Do Not Disturb",
        windows: "Settings → System → Notifications → Turn on Do not disturb",
        linux: "Notification settings → Do Not Disturb",
      }),
    },
    ...shared,
  ];
}

/**
 * The platform's wording, or a description of the setting when unknown.
 *
 * An unknown platform gets the *concept* rather than an empty string: a Linux
 * laptop or a kiosk browser should still be told what to look for.
 */
function describe(
  platform: Platform,
  paths: { macos: string; windows: string; linux: string },
): string {
  switch (platform) {
    case "macos":
      return paths.macos;
    case "windows":
      return paths.windows;
    case "linux":
      return paths.linux;
    default:
      return `${paths.linux} — or your system's equivalent`;
  }
}

/** Guesses the platform from the user agent, for the checklist's wording only. */
export function detectPlatform(userAgent: string): Platform {
  const agent = userAgent.toLowerCase();

  if (agent.includes("mac")) return "macos";
  if (agent.includes("win")) return "windows";
  if (agent.includes("linux") || agent.includes("x11")) return "linux";

  return "unknown";
}
