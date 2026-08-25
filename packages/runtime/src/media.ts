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

/**
 * Peak and RMS of a buffer of samples, in dBFS.
 *
 * The decoder hands this a mix-down. Tests hand it a buffer they wrote, which
 * is how the numbers stay honest without an AudioContext.
 */
export function levelsFromSamples(samples: ArrayLike<number>): Levels {
  let peak = 0;
  let sumSq = 0;
  const n = samples.length;

  for (let i = 0; i < n; i++) {
    const sample = samples[i] ?? 0;
    const magnitude = Math.abs(sample);
    if (magnitude > peak) peak = magnitude;
    sumSq += sample * sample;
  }

  return {
    peakDb: db(peak),
    integratedDb: db(n === 0 ? 0 : Math.sqrt(sumSq / n)),
  };
}

function db(linear: number): number {
  if (!(linear > 0)) return Number.NEGATIVE_INFINITY;
  return 20 * Math.log10(linear);
}

type DecodedBuffer = {
  readonly numberOfChannels: number;
  readonly length: number;
  getChannelData(channel: number): Float32Array;
};

type OfflineAudioContextLike = {
  decodeAudioData(data: ArrayBuffer): Promise<DecodedBuffer>;
};

type OfflineAudioContextCtor = new (
  channels: number,
  length: number,
  sampleRate: number,
) => OfflineAudioContextLike;

/**
 * Decode a clip's audio without playing it.
 *
 * Throws when the browser will not decode the file, when the fetch fails, or
 * when there is no AudioContext — which is what `inspect` turns into
 * `unknown` rather than a guessed "ok".
 */
export async function decodeLevels(url: string): Promise<Levels> {
  const host = globalThis as typeof globalThis & {
    OfflineAudioContext?: OfflineAudioContextCtor;
    webkitOfflineAudioContext?: OfflineAudioContextCtor;
  };
  const Ctor = host.OfflineAudioContext ?? host.webkitOfflineAudioContext;
  if (typeof Ctor !== "function") throw new Error("no audio context");
  if (typeof fetch !== "function") throw new Error("no fetch");

  const response = await fetch(url);
  if (!response.ok) throw new Error(`fetch ${String(response.status)}`);

  const bytes = await response.arrayBuffer();
  const ctx = new Ctor(1, 1, 44_100);
  const decoded = await ctx.decodeAudioData(bytes.slice(0));
  return levelsFromDecoded(decoded);
}

function levelsFromDecoded(buffer: DecodedBuffer): Levels {
  const channels = buffer.numberOfChannels;
  const length = buffer.length;
  if (channels === 0 || length === 0) {
    return { peakDb: Number.NEGATIVE_INFINITY, integratedDb: Number.NEGATIVE_INFINITY };
  }

  const mixed = new Float32Array(length);
  for (let c = 0; c < channels; c++) {
    const data = buffer.getChannelData(c);
    for (let i = 0; i < length; i++) {
      mixed[i] = (mixed[i] ?? 0) + (data[i] ?? 0) / channels;
    }
  }

  return levelsFromSamples(mixed);
}

/**
 * Measure a clip at a URL, without playing it.
 *
 * The presenter page uses this against the *next* slide's files, which that
 * page does not render — so there is no `<video>` to hand `inspect`. A
 * relative `src` is resolved by the caller.
 */
export async function measureClip(url: string): Promise<LevelReport> {
  return createMediaController({
    measure: () => decodeLevels(url),
  }).inspect({
    src: url,
    volume: 1,
    muted: false,
    paused: true,
    duration: 0,
    play: async () => undefined,
    pause: () => undefined,
  });
}
