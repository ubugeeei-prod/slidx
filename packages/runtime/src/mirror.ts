/**
 * Keeping two windows on the same slide.
 *
 * A presenter has notes on a laptop and the deck on a projector. Both windows
 * can be clicked, so this is symmetric: there is no leader, only the most
 * recent position.
 *
 * Ordering is settled by a counter rather than a timestamp. Two windows on the
 * same machine share a clock, but the two are not guaranteed to be — and a
 * clock that steps backwards would make a live message look stale, which is
 * the one failure a presenter cannot work around.
 *
 * Mirroring is an enhancement. Where the transport is missing the deck still
 * presents, because one working window beats two broken ones.
 */

/** Where in the deck a window is. */
export interface Position {
  slide: number;
  step: number;
}

/** What crosses the channel. */
export interface MirrorMessage {
  type: "position" | "request";
  position?: Position;
  /** Monotonic per sender. Higher wins. */
  sequence: number;
  /**
   * Which window sent it.
   *
   * Load-bearing, because `sequence` is monotonic *per sender* and nothing
   * makes two senders agree about where their counters are. A deck is
   * multi-page HTML, so every move reloads a window and restarts its counter at
   * one — and a single watermark across all senders therefore let the first
   * window to announce itself silence every later one.
   *
   * That is not hypothetical. The presenter view announces its position on load
   * so a projector opening later can sync, which set its own watermark to 1;
   * from then on it ignored every position the projector sent, because a freshly
   * loaded projector page can only ever count to 1 as well. The speaker drove
   * from the projector — where a clicker's keys usually land — and their notes
   * quietly stopped following.
   *
   * Optional so a window running an older build is still heard: those messages
   * share one watermark between them, which is exactly the behaviour they had.
   */
  from?: string;
}

/** The channel itself, injectable so this is testable without two windows. */
export interface MirrorTransport {
  post(message: MirrorMessage): void;
  listen(handler: (message: MirrorMessage) => void): () => void;
  close(): void;
}

export interface Mirror {
  /** False when no transport exists, so the UI can say mirroring is off. */
  readonly available: boolean;
  send(position: Position): void;
  /** Sends at an explicit sequence. Exists so ordering can be tested. */
  sendAt(position: Position, sequence: number): void;
  /** Asks the other windows where they are. Used when joining mid-talk. */
  requestPosition(): void;
  position(): Position | null;
  subscribe(handler: (position: Position) => void): () => void;
  close(): void;
}

export interface MirrorOptions {
  /** Channel name. Decks on one machine must not drive each other. */
  name?: string;
  /** Pass `null` to disable, or a transport to substitute one. */
  transport?: MirrorTransport | null;
  /** This window's identity on the channel. Injected so ordering is testable. */
  id?: string;
}

/**
 * A name for this window, unique among the windows showing this deck.
 *
 * Unique per *window*, not per machine or per deck: two tabs of the same slide
 * are two senders with two counters, and telling them apart is the whole job.
 * `randomUUID` needs a secure context, which a deck opened from a USB stick
 * over `file://` is not.
 */
function identity(): string {
  return globalThis.crypto?.randomUUID?.() ?? `w${Math.random().toString(36).slice(2)}`;
}

export function createMirror(options: MirrorOptions = {}): Mirror {
  const transport =
    options.transport === undefined ? broadcastChannel(options.name ?? "slidx") : options.transport;

  const handlers = new Set<(position: Position) => void>();
  const me = options.id ?? identity();
  /**
   * The highest sequence seen from each sender.
   *
   * One entry per window that has spoken, and a window is a page load — so a
   * talk adds an entry per slide change and the map stays in the tens. Bounding
   * it would mean evicting a sender that might speak again, which is the
   * failure this map exists to prevent.
   */
  const seen = new Map<string, number>();
  let current: Position | null = null;
  let sequence = 0;

  const apply = (message: MirrorMessage) => {
    if (message.type === "request") {
      // Answer only if there is something to answer with; a window that has
      // not moved has no opinion about where the deck is.
      if (current) sendAt(current, ++sequence);
      return;
    }

    // Absent `from` shares one watermark, which is what a window running an
    // older build already had.
    const sender = message.from ?? "";
    if (!message.position || message.sequence <= (seen.get(sender) ?? -1)) return;

    seen.set(sender, message.sequence);
    if (samePosition(current, message.position)) return;

    current = message.position;
    for (const handler of handlers) handler(message.position);
  };

  const post = (message: MirrorMessage) => transport?.post({ ...message, from: me });
  const stop = transport?.listen(apply);

  const sendAt = (position: Position, at: number) => {
    sequence = Math.max(sequence, at);
    current = position;
    // Nothing to record here: a sender's own messages never come back to it,
    // and every other sender is counted under its own key.
    post({ type: "position", position, sequence: at });
  };

  return {
    available: transport !== null,

    send: (position) => sendAt(position, ++sequence),

    sendAt,

    requestPosition() {
      post({ type: "request", sequence: ++sequence });
    },

    position: () => current,

    subscribe(handler) {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },

    close() {
      handlers.clear();
      stop?.();
      transport?.close();
    },
  };
}

function samePosition(a: Position | null, b: Position): boolean {
  return a !== null && a.slide === b.slide && a.step === b.step;
}

/**
 * A BroadcastChannel, where the browser has one.
 *
 * Supported everywhere slidx targets, but a deck can also be opened from a
 * file:// URL or an embedded webview where it is absent. Returning `null`
 * rather than throwing keeps the deck presentable in exactly the situation
 * where a speaker has no time to debug.
 */
function broadcastChannel(name: string): MirrorTransport | null {
  if (typeof BroadcastChannel === "undefined") return null;

  const channel = new BroadcastChannel(`slidx:${name}`);

  return {
    post: (message) => channel.postMessage(message),
    listen(handler) {
      const listener = (event: MessageEvent<MirrorMessage>) => handler(event.data);
      channel.addEventListener("message", listener);
      return () => channel.removeEventListener("message", listener);
    },
    close: () => channel.close(),
  };
}
