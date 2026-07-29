/**
 * The deck's half, and how it fails.
 *
 * This is the specification for a component whose most important behaviour is
 * what it does when nothing works. The channel is an enhancement: a deck whose
 * Worker is unreachable, whose room slug is a typo, or which is opened from a
 * file with no WebSocket at all, still has a talk to give.
 *
 * The failure modes it guards:
 *
 * - Anything at all thrown into the page. A speaker with a broken widget has
 *   thirty seconds and no console.
 * - A retry loop against a room that has ended, or against an environment that
 *   will never have a socket.
 * - A page's own handler throwing and taking the channel down with it.
 * - A reconnect schedule that stays escalated after it succeeded.
 * - Trusting a frame from whatever the endpoint happened to point at.
 *
 * The socket, the scheduler and the randomness are injected, so every one of
 * these is deterministic.
 */

import { describe, expect, it } from "vitest";

import {
  createAudienceChannel,
  socketUrl,
  type ChannelSocketHandlers,
  type Scheduler,
  type SocketFactory,
} from "../src/client";
import type { RoomSnapshot } from "../src/protocol";

const ENDPOINT = "https://audience.example.workers.dev";

const SNAPSHOT: RoomSnapshot = {
  room: "a-talk",
  moderation: "held",
  present: 4,
  questions: [{ id: "000001", text: "why not Zig?", votes: 2, at: 1 }],
  reactions: { clap: 3, agree: 0, confused: 0, love: 0 },
  expiresAt: 9_999,
};

interface FakeSocket {
  url: string;
  sent: string[];
  closed: boolean;
  handlers: ChannelSocketHandlers;
}

function sockets() {
  const opened: FakeSocket[] = [];

  const factory: SocketFactory = (url, handlers) => {
    const socket: FakeSocket = { url, sent: [], closed: false, handlers };
    opened.push(socket);

    return {
      send: (data) => socket.sent.push(data),
      close: () => {
        socket.closed = true;
      },
    };
  };

  return {
    factory,
    opened,
    last: () => {
      const socket = opened.at(-1);
      if (!socket) throw new Error("no socket was opened");
      return socket;
    },
  };
}

function timers() {
  const queue: { run: () => void; delayMs: number }[] = [];

  const schedule: Scheduler = (handler, delayMs) => {
    const entry = { run: handler, delayMs };
    queue.push(entry);

    return () => {
      const index = queue.indexOf(entry);
      if (index >= 0) queue.splice(index, 1);
    };
  };

  return {
    schedule,
    queue,
    fire() {
      const entry = queue.shift();
      if (!entry) throw new Error("nothing was scheduled");
      entry.run();
    },
  };
}

/** The midpoint of the jitter window, so a delay is an exact number. */
const backoff = { baseMs: 100, maxMs: 800, random: () => 1 };

function channelOn(pool: ReturnType<typeof sockets>, clocks: ReturnType<typeof timers>) {
  return createAudienceChannel({
    endpoint: ENDPOINT,
    room: "a-talk",
    socket: pool.factory,
    schedule: clocks.schedule,
    backoff,
  });
}

describe("the socket URL", () => {
  it("upgrades https to wss", () => {
    expect(socketUrl(ENDPOINT, "a-talk")).toBe(
      `wss://audience.example.workers.dev/rooms/a-talk/socket`,
    );
  });

  it("leaves a plain-http workshop on plain ws", () => {
    // Forcing wss here is a mixed-content failure with no visible cause.
    expect(socketUrl("http://localhost:8787", "a-talk")).toBe(
      "ws://localhost:8787/rooms/a-talk/socket",
    );
  });

  it("carries the speaker's key in the query, because a handshake has no headers", () => {
    expect(socketUrl(ENDPOINT, "a-talk", "speaker-key-here")).toContain("key=speaker-key-here");
  });

  it("refuses a room slug that is not one", () => {
    expect(socketUrl(ENDPOINT, "../secrets")).toBeNull();
  });

  it("refuses an endpoint that is not a URL", () => {
    expect(socketUrl("not a url", "a-talk")).toBeNull();
  });

  it("refuses a scheme that is not the web's", () => {
    expect(socketUrl("file:///deck.html", "a-talk")).toBeNull();
  });
});

describe("a channel that cannot work here", () => {
  it("opens nothing when the room slug is a typo", () => {
    const pool = sockets();
    const channel = createAudienceChannel({
      endpoint: ENDPOINT,
      room: "Not A Slug",
      socket: pool.factory,
    });

    expect(channel.available).toBe(false);
    expect(channel.status()).toBe("unavailable");
    expect(pool.opened).toHaveLength(0);
  });

  it("opens nothing when the endpoint is not a URL", () => {
    const pool = sockets();
    const channel = createAudienceChannel({
      endpoint: "audience dot example",
      room: "a-talk",
      socket: pool.factory,
    });

    expect(channel.available).toBe(false);
    expect(pool.opened).toHaveLength(0);
  });

  it("does not schedule a retry where there is no WebSocket at all", () => {
    // A deck opened from a file, or inside an embedded webview. Retrying it
    // would burn a timer for the length of the talk to no purpose.
    const original = Reflect.get(globalThis, "WebSocket") as unknown;
    Reflect.deleteProperty(globalThis, "WebSocket");

    try {
      const clocks = timers();
      const channel = createAudienceChannel({
        endpoint: ENDPOINT,
        room: "a-talk",
        schedule: clocks.schedule,
      });

      expect(channel.status()).toBe("unavailable");
      expect(clocks.queue).toHaveLength(0);
    } finally {
      Reflect.set(globalThis, "WebSocket", original);
    }
  });

  it("still answers every method rather than throwing", () => {
    const channel = createAudienceChannel({ endpoint: "nonsense", room: "a-talk" });

    expect(channel.ask("why not Zig?")).toBe(false);
    expect(channel.upvote("000001")).toBe(false);
    expect(channel.react("clap")).toBe(false);
    expect(channel.state()).toBeNull();
    expect(() => channel.close()).not.toThrow();
  });
});

describe("connecting", () => {
  it("opens a socket and reports the room open", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());
    expect(channel.status()).toBe("connecting");

    pool.last().handlers.onOpen();
    expect(channel.status()).toBe("open");
  });

  it("keeps the last snapshot it was sent", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());
    pool.last().handlers.onOpen();

    pool.last().handlers.onMessage(JSON.stringify({ type: "state", state: SNAPSHOT }));

    expect(channel.state()).toEqual(SNAPSHOT);
  });

  it("ignores a frame from something that is not a slidx room", () => {
    // The endpoint comes from frontmatter, so a typo can point the deck at
    // anything at all.
    const pool = sockets();
    const channel = channelOn(pool, timers());
    pool.last().handlers.onOpen();

    expect(() => pool.last().handlers.onMessage("<!doctype html>")).not.toThrow();
    expect(channel.state()).toBeNull();
  });

  it("passes a refusal to the page so it can be shown to the asker", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());
    const seen: string[] = [];
    channel.onRejected((reason) => seen.push(reason));
    pool.last().handlers.onOpen();

    pool.last().handlers.onMessage(JSON.stringify({ type: "rejected", reason: "rate-limited" }));

    expect(seen).toEqual(["rate-limited"]);
  });

  it("survives a handler of the page's that throws", () => {
    // That is the page's defect. One broken widget must not take the rest of
    // the deck down mid-talk.
    const pool = sockets();
    const channel = channelOn(pool, timers());
    const reached: string[] = [];

    channel.onState(() => {
      throw new Error("the page is broken");
    });
    channel.onState(() => reached.push("second"));
    pool.last().handlers.onOpen();

    expect(() =>
      pool.last().handlers.onMessage(JSON.stringify({ type: "state", state: SNAPSHOT })),
    ).not.toThrow();
    expect(reached).toEqual(["second"]);
  });
});

describe("sending", () => {
  it("refuses while the channel is down, rather than queueing silently", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());

    expect(channel.ask("why not Zig?")).toBe(false);
    expect(pool.last().sent).toEqual([]);
  });

  it("sends once the room is open", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());
    pool.last().handlers.onOpen();

    expect(channel.ask("why not Zig?", "Ada")).toBe(true);
    expect(JSON.parse(pool.last().sent[0] ?? "null")).toEqual({
      type: "question",
      text: "why not Zig?",
      name: "Ada",
    });
  });

  it("refuses an over-long question without spending a round trip on it", () => {
    // The room checks again; this is only so the asker hears about it while
    // they are still looking at the box they typed in.
    const pool = sockets();
    const channel = channelOn(pool, timers());
    pool.last().handlers.onOpen();

    expect(channel.ask("a".repeat(500))).toBe(false);
    expect(pool.last().sent).toEqual([]);
  });

  it("refuses a reaction outside the vocabulary", () => {
    const pool = sockets();
    const channel = channelOn(pool, timers());
    pool.last().handlers.onOpen();

    expect(channel.react("shrug" as "clap")).toBe(false);
  });

  it("returns false rather than throwing when the socket dies mid-send", () => {
    const failing: SocketFactory = (_url, handlers) => {
      queueMicrotask(() => handlers.onOpen());
      return {
        send: () => {
          throw new Error("the socket is gone");
        },
        close: () => {},
      };
    };

    const channel = createAudienceChannel({
      endpoint: ENDPOINT,
      room: "a-talk",
      socket: (url, handlers) => {
        const socket = failing(url, handlers);
        handlers.onOpen();
        return socket;
      },
    });

    expect(channel.ask("why not Zig?")).toBe(false);
  });
});

describe("reconnecting", () => {
  it("schedules a retry when the socket drops", () => {
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    pool.last().handlers.onOpen();

    pool.last().handlers.onClose();

    expect(channel.status()).toBe("waiting");
    expect(clocks.queue[0]?.delayMs).toBe(100);
  });

  it("backs off further each time it fails", () => {
    const pool = sockets();
    const clocks = timers();
    channelOn(pool, clocks);
    pool.last().handlers.onClose();

    clocks.fire();
    pool.last().handlers.onClose();

    expect(clocks.queue[0]?.delayMs).toBe(200);
  });

  it("stops growing at the ceiling", () => {
    const pool = sockets();
    const clocks = timers();
    channelOn(pool, clocks);

    for (let attempt = 0; attempt < 8; attempt += 1) {
      pool.last().handlers.onClose();
      clocks.fire();
    }
    pool.last().handlers.onClose();

    expect(clocks.queue[0]?.delayMs).toBe(800);
  });

  it("comes back", () => {
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    pool.last().handlers.onOpen();
    pool.last().handlers.onClose();

    clocks.fire();
    pool.last().handlers.onOpen();

    expect(channel.status()).toBe("open");
    expect(pool.opened).toHaveLength(2);
  });

  it("starts the schedule over after a reconnect succeeds", () => {
    // Otherwise the second blip of the talk waits the length of the first.
    const pool = sockets();
    const clocks = timers();
    channelOn(pool, clocks);
    pool.last().handlers.onClose();
    clocks.fire();
    pool.last().handlers.onOpen();

    pool.last().handlers.onClose();

    expect(clocks.queue[0]?.delayMs).toBe(100);
  });

  it("treats a factory that throws as a transport that is not there yet", () => {
    // A laptop that woke up on a different network is the ordinary case.
    const clocks = timers();
    const channel = createAudienceChannel({
      endpoint: ENDPOINT,
      room: "a-talk",
      socket: () => {
        throw new Error("no network");
      },
      schedule: clocks.schedule,
      backoff,
    });

    expect(channel.status()).toBe("waiting");
    expect(clocks.queue).toHaveLength(1);
  });

  it("gives up when the room says it has ended", () => {
    // Retrying against a room that is gone is a loop for as long as the tab
    // stays open.
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    const endings: string[] = [];
    channel.onClosed((reason) => endings.push(reason));
    pool.last().handlers.onOpen();

    pool.last().handlers.onMessage(JSON.stringify({ type: "closed", reason: "expired" }));
    pool.last().handlers.onClose();

    expect(channel.status()).toBe("closed");
    expect(endings).toEqual(["expired"]);
    expect(clocks.queue).toHaveLength(0);
  });

  it("stops retrying once the deck closes it", () => {
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    pool.last().handlers.onOpen();

    channel.close();

    expect(channel.status()).toBe("closed");
    expect(pool.last().closed).toBe(true);
    expect(clocks.queue).toHaveLength(0);
  });

  it("does not reconnect after a deliberate close, even though closing drops the socket", () => {
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    pool.last().handlers.onOpen();

    channel.close();
    pool.last().handlers.onClose();

    expect(clocks.queue).toHaveLength(0);
    expect(channel.status()).toBe("closed");
  });

  it("reports each status change once", () => {
    const pool = sockets();
    const clocks = timers();
    const channel = channelOn(pool, clocks);
    const seen: string[] = [];
    channel.onStatus((status) => seen.push(status));

    pool.last().handlers.onOpen();
    pool.last().handlers.onClose();
    clocks.fire();
    pool.last().handlers.onOpen();

    expect(seen).toEqual(["open", "waiting", "connecting", "open"]);
  });
});
