/**
 * Video and audio on a slide.
 *
 * This exists for the failure everyone has watched happen: a speaker plays a
 * clip and the room hears nothing, or hears it at a volume that makes people
 * flinch. Both are unrecoverable in the moment — you cannot re-play a clip to
 * an audience that has already reacted.
 *
 * So the work happens *before*. A clip is measured while the deck is still on
 * an earlier slide and reported in words a speaker can act on, and playback is
 * normalised towards a target so one loud clip in a deck of quiet ones does
 * not arrive at full scale.
 *
 * Everything the browser supplies is injected, which is what makes this
 * testable without a real `<video>` and an AudioContext.
 */

/** The level a clip should arrive at, in dBFS peak. */
export const LOUDNESS_TARGET_DB = -14;

/** Above this, a clip will make a room flinch. */
const TOO_LOUD_DB = -3;

/** Below this, the back of the room hears nothing. */
const TOO_QUIET_DB = -30;

/** Just enough of `HTMLMediaElement` to prepare and stop a clip. */
export interface MediaElementLike {
  src: string;
  volume: number;
  muted: boolean;
  readonly paused: boolean;
  readonly duration: number;
  play(): Promise<void>;
  pause(): void;
}

/** What analysing a clip's audio produced. */
export interface Levels {
  /** Loudest sample, in dBFS. */
  peakDb: number;
  /** Average perceived loudness, in dBFS. */
  integratedDb: number;
}

export type LevelStatus = "ok" | "too-loud" | "too-quiet" | "unknown";

export interface LevelReport {
  status: LevelStatus;
  levels: Levels | null;
  /** What the speaker should do. Present whenever the status is not `ok`. */
  remedy: string | null;
}

export interface MediaControllerOptions {
  /** Analyses a clip without playing it aloud. Injected; browsers differ. */
  measure: (media: MediaElementLike) => Promise<Levels>;
}

export interface MediaController {
  /** Measures a clip and says what to do about it. Never plays it aloud. */
  inspect(media: MediaElementLike): Promise<LevelReport>;
  /** Sets the clip up to be played, without starting it. */
  prepare(media: MediaElementLike): Promise<LevelReport>;
  /** Stops a clip, for when its slide is left. */
  release(media: MediaElementLike): void;
}

export function createMediaController(options: MediaControllerOptions): MediaController {
  const inspect = async (media: MediaElementLike): Promise<LevelReport> => {
    let levels: Levels;
    try {
      levels = await options.measure(media);
    } catch {
      // A codec the browser will not decode, or a cross-origin file. A false
      // "ok" here is worse than no answer, because the speaker stops checking.
      return {
        status: "unknown",
        levels: null,
        remedy:
          "This clip could not be measured — play it once with the room's sound before you start.",
      };
    }

    return { ...verdict(levels.peakDb), levels };
  };

  return {
    inspect,

    async prepare(media) {
      const report = await inspect(media);

      // Attenuate towards the target, never amplify: raising the gain raises
      // the noise floor with it, and a quiet clip is a problem to fix in the
      // file — which is what the report says.
      if (report.levels && report.levels.peakDb > LOUDNESS_TARGET_DB) {
        media.volume = gainFor(report.levels.peakDb);
      }

      // Deliberately not played. Browsers refuse autoplay with sound, and a
      // slide that silently did nothing is worse than one showing a play
      // button the speaker can press.
      return report;
    },

    release(media) {
      // Audio continuing over the next slide is the second-worst thing a deck
      // can do to a speaker, after not playing at all.
      if (!media.paused) media.pause();
    },
  };
}

function verdict(peakDb: number): { status: LevelStatus; remedy: string | null } {
  if (peakDb > TOO_LOUD_DB) {
    return {
      status: "too-loud",
      remedy: "Lower this clip's level in the file, or the room will flinch when it starts.",
    };
  }

  if (peakDb < TOO_QUIET_DB) {
    return {
      status: "too-quiet",
      remedy: "Raise this clip's level in the file — turning the room up raises its noise with it.",
    };
  }

  return { status: "ok", remedy: null };
}

/**
 * Linear gain that brings a peak down to the target.
 *
 * dB is logarithmic and `volume` is linear, which is the conversion people get
 * wrong: 10 dB down is not 0.9.
 */
function gainFor(peakDb: number): number {
  return Math.min(1, 10 ** ((LOUDNESS_TARGET_DB - peakDb) / 20));
}

/**
 * A level, in words.
 *
 * "-0.5 dBFS" means nothing to most speakers two minutes before a talk. The
 * number is kept for the ones it does mean something to.
 */
export function describeLevel(peakDb: number): string {
  const measured = `${peakDb} dBFS`;

  if (peakDb > TOO_LOUD_DB) return `Loud — will startle the room (${measured})`;
  if (peakDb < TOO_QUIET_DB) return `Quiet — the back row will miss it (${measured})`;

  return `Fine for a room (${measured})`;
}
