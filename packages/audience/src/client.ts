/**
 * What a deck page loads.
 *
 * The channel is an enhancement, and this module's first responsibility is to
 * behave like one. A deck whose Worker is unreachable, whose room slug is a
 * typo, or which is opened from a `file://` URL with no WebSocket at all, still
 * presents — so nothing here throws into the page, and every failure has a
 * status rather than an exception. A speaker discovering a broken Q&A widget
 * has thirty seconds and no console.
 *
 * The socket, the clock, the scheduler and the randomness are all injected.
 * That is what lets reconnection be a specification instead of a hope.
 */

import { createBackoff, type Backoff, type BackoffOptions } from "./backoff";
import {
  isRoomSlug,
  parseServerFrame,
  validateClientMessage,
  type ClientMessage,
  type ReactionKind,
  type RejectionReason,
  type RoomEndReason,
  type RoomSnapshot,
} from "./protocol";

export type ChannelStatus =
  /** No WebSocket in this environment, or nothing worth connecting to. Never retries. */
  | "unavailable"
  | "connecting"
  | "open"
  /** Disconnected, with a retry already scheduled. */
  | "waiting"
  /** The room ended, or the deck closed the channel. Final. */
  | "closed";

/** What the client needs from a socket, which is less than a WebSocket offers. */
export interface ChannelSocket {
  send(data: string): void;
  close(): void;
}

export interface ChannelSocketHandlers {
  onOpen(): void;
  onMessage(data: string): void;
  onClose(): void;
}

export type SocketFactory = (url: string, handlers: ChannelSocketHandlers) => ChannelSocket;

/** Schedules a retry and returns its cancel. Injected so tests need no real time. */
export type Scheduler = (handler: () => void, delayMs: number) => () => void;

export interface AudienceChannelOptions {
  /** Where the Worker is, e.g. `https://audience.example.workers.dev`. */
  endpoint: string;
  /** The room slug, from the deck's frontmatter. */
  room: string;
  /** The speaker's key. Only the presenter view has one. */
  hostKey?: string;
  socket?: SocketFactory;
  schedule?: Scheduler;
  backoff?: BackoffOptions;
}

export interface AudienceChannel {
  /** False when the channel can never work here, so a deck can hide the widget. */
  readonly available: boolean;
  status(): ChannelStatus;
  /** The last snapshot received, or null before the first one. */
  state(): RoomSnapshot | null;
  /** False when the message could not be sent. Never throws. */
  ask(text: string, name?: string): boolean;
  upvote(questionId: string): boolean;
  react(kind: ReactionKind): boolean;
  onState(handler: (state: RoomSnapshot) => void): () => void;
  onStatus(handler: (status: ChannelStatus) => void): () => void;
  onRejected(handler: (reason: RejectionReason) => void): () => void;
  onClosed(handler: (reason: RoomEndReason) => void): () => void;
  /** Hangs up and stops retrying. */
  close(): void;
}

/**
 * The socket URL for a room.
 *
 * `https` becomes `wss` rather than being assumed: a deck served over plain
 * `http` in a workshop on a laptop should reach its room over plain `ws`,
 * because the alternative is a mixed-content failure with no visible cause.
 */
export function socketUrl(endpoint: string, room: string, hostKey?: string): string | null {
  if (!isRoomSlug(room)) return null;

  let url: URL;
  try {
    url = new URL(endpoint);
  } catch {
    return null;
  }

  if (url.protocol === "https:") url.protocol = "wss:";
  else if (url.protocol === "http:") url.protocol = "ws:";
  else if (url.protocol !== "wss:" && url.protocol !== "ws:") return null;

  url.pathname = `/rooms/${encodeURIComponent(room)}/socket`;
  if (hostKey !== undefined) url.searchParams.set("key", hostKey);

  return url.toString();
}

/**
 * A WebSocket, where the environment has one.
 *
 * Returns null rather than throwing when it does not. A deck opened from a file
 * or inside an embedded webview is exactly the situation where a speaker has no
 * time to debug an exception.
 */
function browserSocket(url: string, handlers: ChannelSocketHandlers): ChannelSocket | null {
  if (typeof WebSocket === "undefined") return null;

  const socket = new WebSocket(url);

  socket.addEventListener("open", () => handlers.onOpen());
  socket.addEventListener("message", (event: MessageEvent<unknown>) => {
    if (typeof event.data === "string") handlers.onMessage(event.data);
  });
  // A socket that errors also closes, so one path handles both and the retry
  // is scheduled once rather than twice.
  socket.addEventListener("close", () => handlers.onClose());
  socket.addEventListener("error", () => handlers.onClose());

  return { send: (data) => socket.send(data), close: () => socket.close() };
}

const defaultSchedule: Scheduler = (handler, delayMs) => {
  const handle = setTimeout(handler, delayMs);
  return () => clearTimeout(handle);
};

export function createAudienceChannel(options: AudienceChannelOptions): AudienceChannel {
  const url = socketUrl(options.endpoint, options.room, options.hostKey);
  const schedule = options.schedule ?? defaultSchedule;
  const backoff: Backoff = createBackoff(options.backoff ?? {});

  const stateHandlers = new Set<(state: RoomSnapshot) => void>();
  const statusHandlers = new Set<(status: ChannelStatus) => void>();
  const rejectedHandlers = new Set<(reason: RejectionReason) => void>();
  const closedHandlers = new Set<(reason: RoomEndReason) => void>();

  let socket: ChannelSocket | null = null;
  let cancelRetry: (() => void) | null = null;
  let snapshot: RoomSnapshot | null = null;
  let status: ChannelStatus = url === null ? "unavailable" : "connecting";

  /**
   * Handlers belong to the page, and a page's handler can throw.
   *
   * When it does, that is the page's defect and not the channel's: swallowing
   * it here keeps one broken widget from taking down the rest of the deck
   * mid-talk.
   */
  function notify<T>(handlers: Set<(value: T) => void>, value: T): void {
    for (const handler of handlers) {
      try {
        handler(value);
      } catch {
        // Deliberately ignored. See above.
      }
    }
  }

  function moveTo(next: ChannelStatus): void {
    if (status === next) return;

    status = next;
    notify(statusHandlers, next);
  }

  function finish(reason: RoomEndReason | null): void {
    const already = status === "closed";

    // The flag is set before the socket is touched, because closing a socket
    // fires its own close handler — and that handler schedules a retry unless
    // it can see that this shutdown was deliberate.
    status = "closed";

    cancelRetry?.();
    cancelRetry = null;

    try {
      socket?.close();
    } catch {
      // Closing a socket that is already gone is not a failure.
    }

    socket = null;
    if (!already) notify(statusHandlers, "closed");
    if (reason) notify(closedHandlers, reason);
  }

  function receive(raw: string): void {
    const message = parseServerFrame(raw);
    // Not a slidx room, or a version that disagrees with this one. Ignored
    // rather than surfaced: there is nothing a participant could do about it.
    if (!message) return;

    switch (message.type) {
      case "state":
        snapshot = message.state;
        notify(stateHandlers, message.state);
        return;

      case "rejected":
        notify(rejectedHandlers, message.reason);
        return;

      case "closed":
        // The room is over. Retrying would be a loop against a 404 for as long
        // as the tab stays open.
        finish(message.reason);
        return;

      case "accepted":
        return;
    }
  }

  function retry(): void {
    if (status === "closed") return;

    moveTo("waiting");
    cancelRetry = schedule(connect, backoff.next());
  }

  function connect(): void {
    if (status === "closed" || url === null) return;

    cancelRetry = null;
    moveTo("connecting");

    let opened: ChannelSocket | null = null;
    try {
      opened = (options.socket ?? browserSocket)(url, {
        onOpen: () => {
          backoff.reset();
          moveTo("open");
        },
        onMessage: receive,
        onClose: () => {
          socket = null;
          retry();
        },
      });
    } catch {
      // A factory that throws is a transport that is not there right now. It
      // may be there in a minute — a laptop that woke up on a different
      // network is the ordinary case — so this is a retry, not a defeat.
      opened = null;
    }

    if (!opened) {
      // No WebSocket at all is permanent, and retrying it forever would burn a
      // timer for the length of the talk.
      if (typeof WebSocket === "undefined" && options.socket === undefined) {
        moveTo("unavailable");
        return;
      }

      retry();
      return;
    }

    socket = opened;
  }

  function send(message: ClientMessage): boolean {
    // Checked locally first: it costs a round trip to be told what this client
    // already knows, and the room checks again regardless.
    if (!validateClientMessage(message).ok) return false;
    if (!socket || status !== "open") return false;

    try {
      socket.send(JSON.stringify(message));
      return true;
    } catch {
      return false;
    }
  }

  const subscribe = <T>(handlers: Set<(value: T) => void>, handler: (value: T) => void) => {
    handlers.add(handler);
    return () => {
      handlers.delete(handler);
    };
  };

  if (url !== null) connect();

  return {
    // Decided after the first connection attempt, so an environment with no
    // WebSocket at all reports itself as unusable rather than as pending.
    available: url !== null && status !== "unavailable",

    status: () => status,

    state: () => snapshot,

    ask: (text, name) => send({ type: "question", text, ...(name === undefined ? {} : { name }) }),

    upvote: (questionId) => send({ type: "upvote", questionId }),

    react: (kind) => send({ type: "reaction", kind }),

    onState: (handler) => subscribe(stateHandlers, handler),
    onStatus: (handler) => subscribe(statusHandlers, handler),
    onRejected: (handler) => subscribe(rejectedHandlers, handler),
    onClosed: (handler) => subscribe(closedHandlers, handler),

    close: () => finish(null),
  };
}
