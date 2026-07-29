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
 */

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
  exit(): Promise<void>;
}

/** Injected so this is testable, and so a missing API is a value not a crash. */
export interface PresentationEnvironment {
  requestWakeLock?: () => Promise<{ release: () => void }>;
  requestFullscreen?: () => Promise<void>;
  exitFullscreen?: () => Promise<void>;
  platform?: Platform;
}

/**
 * Takes what the browser will give and reports the rest.
 *
 * Never rejects. Every capability here is optional in some browser a talk will
 * eventually be given in, and a missing wake lock costs a dimmed screen while
 * a thrown error costs the talk.
 */
export async function enterPresentation(
  environment: PresentationEnvironment = {},
): Promise<PresentationSession> {
  const lock = await attempt(environment.requestWakeLock);
  const fullscreen = (await attempt(environment.requestFullscreen)) !== null;

  let exited = false;

  return {
    wakeLock: lock !== null,
    fullscreen,

    async exit() {
      // Exiting twice is normal: a keyboard shortcut and the browser's own
      // fullscreen exit both land here. Releasing a lock twice throws.
      if (exited) return;
      exited = true;

      lock?.release();
      await attempt(environment.exitFullscreen);
    },
  };
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
