/**
 * Driving the deck from a phone.
 *
 * Mirroring already solved the hard half: two windows agreeing on a position,
 * with no leader and ordering settled by a counter. A remote is a third window
 * that happens to be on someone's phone, so it is a `MirrorTransport` rather
 * than a second implementation of the same idea.
 *
 * What is genuinely new is trust, and that is what these tests are about.
 *
 * - **The secret must never reach a log.** It travels in the URL fragment,
 *   which is not sent with the request, and it is presented to the relay once
 *   rather than attached to every frame.
 * - **A remote must not be able to do more than move the deck.** The message
 *   union *is* the capability boundary, so anything that is not a position or
 *   a request for one is dropped rather than interpreted.
 * - **Somebody else's session must not reach this deck.** Two talks in two
 *   rooms will use the same relay at the same time.
 * - **A bad frame is not a crash.** This runs on stage.
 */

import { describe, expect, it } from "vite-plus/test";

import { createMirror } from "../src/mirror";
import {
  createPairing,
  createRemoteTransport,
  pairingUrl,
  readPairing,
  type RemoteSocket,
} from "../src/remote";

/** A socket that records what was sent and can be fed what arrives. */
function fakeSocket() {
  const sent: string[] = [];
  const handlers = new Set<(data: string) => void>();
  let closed = false;

  const socket: RemoteSocket = {
    get open() {
      return !closed;
    },
    send: (data) => sent.push(data),
    listen(handler) {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
    close: () => {
      closed = true;
    },
  };

  return {
    socket,
    sent,
    deliver: (data: string) => {
      for (const handler of handlers) handler(data);
    },
    isClosed: () => closed,
  };
}

/** Bytes a test can predict, so a pairing is reproducible. */
function fixedRandom(fill: number) {
  return (bytes: Uint8Array) => {
    bytes.fill(fill);
    return bytes;
  };
}

const PAIRING = createPairing({ random: fixedRandom(0xab) });

describe("pairing a phone", () => {
  it("puts the secret in the fragment, which is never sent with a request", () => {
    // The whole point. A query parameter lands in the relay's access log, in
    // the referrer of anything the page links to, and in any proxy between —
    // and a secret in a log is a secret a stranger can drive your talk with.
    const url = pairingUrl("https://slidx.dev/remote", PAIRING);

    expect(url).toContain("#");
    expect(url.split("#")[1]).toContain(PAIRING.secret);
    expect(url.split("#")[0]).not.toContain(PAIRING.secret);
  });

  it("reads back what it wrote", () => {
    expect(readPairing(pairingUrl("https://slidx.dev/remote", PAIRING))).toEqual(PAIRING);
  });

  it("refuses a URL carrying the secret anywhere but the fragment", () => {
    // Never accepted from the query, so a link that leaked into a log cannot
    // be replayed by pasting it back in.
    const url = `https://slidx.dev/remote?s=${PAIRING.session}.${PAIRING.secret}`;

    expect(readPairing(url)).toBeNull();
  });

  it("reads nothing out of a URL with no pairing at all", () => {
    expect(readPairing("https://slidx.dev/remote")).toBeNull();
    expect(readPairing("https://slidx.dev/remote#")).toBeNull();
    expect(readPairing("not a url")).toBeNull();
  });

  it("draws a different session and secret every time", () => {
    const first = createPairing();
    const second = createPairing();

    expect(first.session).not.toBe(second.session);
    expect(first.secret).not.toBe(second.secret);
  });

  it("makes the secret long enough to be worth having", () => {
    // A four-digit code is guessable by anyone who can see the room's URL.
    expect(createPairing().secret.length).toBeGreaterThanOrEqual(32);
  });
});

describe("joining the relay", () => {
  it("presents the secret once, before anything else", () => {
    const { socket, sent } = fakeSocket();
    createRemoteTransport({ pairing: PAIRING, socket });

    expect(sent).toHaveLength(1);
    expect(JSON.parse(sent[0] ?? "{}")).toEqual({
      type: "join",
      session: PAIRING.session,
      secret: PAIRING.secret,
    });
  });

  it("never repeats the secret on a relayed frame", () => {
    // A relay that logs traffic logs positions. It must not thereby log the
    // credential that lets someone send them.
    const { socket, sent } = fakeSocket();
    const transport = createRemoteTransport({ pairing: PAIRING, socket });

    transport.post({ type: "position", position: { slide: 2, step: 0 }, sequence: 1 });

    expect(sent.slice(1).join("")).not.toContain(PAIRING.secret);
  });

  it("scopes what it sends to its own session", () => {
    const { socket, sent } = fakeSocket();
    const transport = createRemoteTransport({ pairing: PAIRING, socket });

    transport.post({ type: "request", sequence: 1 });

    expect(JSON.parse(sent[1] ?? "{}").session).toBe(PAIRING.session);
  });
});

describe("what it refuses to act on", () => {
  function received(frame: unknown): unknown[] {
    const { socket, deliver } = fakeSocket();
    const transport = createRemoteTransport({ pairing: PAIRING, socket });
    const seen: unknown[] = [];

    transport.listen((message) => seen.push(message));
    deliver(typeof frame === "string" ? frame : JSON.stringify(frame));

    return seen;
  }

  it("ignores a frame for somebody else's session", () => {
    // Two talks, two rooms, one relay, at the same minute.
    expect(
      received({
        type: "relay",
        session: "someone-else",
        message: { type: "position", position: { slide: 9, step: 0 }, sequence: 1 },
      }),
    ).toEqual([]);
  });

  it("passes through a position for its own session", () => {
    const message = { type: "position", position: { slide: 3, step: 1 }, sequence: 4 };

    expect(received({ type: "relay", session: PAIRING.session, message })).toEqual([message]);
  });

  it("drops anything that is not a position or a request for one", () => {
    // The message union is the capability boundary: a remote can move the
    // deck and cannot do anything else, because there is nothing else it can
    // say. A relay that started forwarding richer frames would find no
    // listener for them here.
    for (const message of [
      { type: "eval", source: "alert(1)", sequence: 1 },
      { type: "position", sequence: 1 },
      { type: "position", position: { slide: 1, step: 0 } },
      { type: "position", position: { slide: "one", step: 0 }, sequence: 1 },
    ]) {
      expect(received({ type: "relay", session: PAIRING.session, message }), message.type).toEqual(
        [],
      );
    }
  });

  it("survives a frame that is not JSON at all", () => {
    expect(() => received("}{")).not.toThrow();
    expect(received("}{")).toEqual([]);
  });

  it("survives a frame that is JSON but not a frame", () => {
    expect(received(42)).toEqual([]);
    expect(received(null)).toEqual([]);
  });
});

describe("as a mirror transport", () => {
  it("drives a mirror without the mirror knowing it is a network", () => {
    // The point of reusing the transport seam: ordering, echo suppression and
    // the no-leader rule are already solved, and a remote inherits them.
    const { socket, deliver } = fakeSocket();
    const mirror = createMirror({ transport: createRemoteTransport({ pairing: PAIRING, socket }) });

    const seen: unknown[] = [];
    mirror.subscribe((position) => seen.push(position));

    deliver(
      JSON.stringify({
        type: "relay",
        session: PAIRING.session,
        message: { type: "position", position: { slide: 5, step: 2 }, sequence: 7 },
      }),
    );

    expect(seen).toEqual([{ slide: 5, step: 2 }]);
    expect(mirror.position()).toEqual({ slide: 5, step: 2 });
  });

  it("reports itself available, so the deck can say the remote is paired", () => {
    const { socket } = fakeSocket();
    const mirror = createMirror({ transport: createRemoteTransport({ pairing: PAIRING, socket }) });

    expect(mirror.available).toBe(true);
  });

  it("closes the socket when the deck closes the mirror", () => {
    const { socket, isClosed } = fakeSocket();
    const mirror = createMirror({ transport: createRemoteTransport({ pairing: PAIRING, socket }) });

    mirror.close();

    expect(isClosed()).toBe(true);
  });

  it("says nothing down a socket that has closed", () => {
    // The network went at the worst moment. The deck keeps presenting from
    // the keyboard, which is the only reason mirroring is an enhancement.
    const { socket, sent } = fakeSocket();
    const transport = createRemoteTransport({ pairing: PAIRING, socket });

    socket.close();

    expect(() => transport.post({ type: "request", sequence: 1 })).not.toThrow();
    expect(sent).toHaveLength(1);
  });
});
