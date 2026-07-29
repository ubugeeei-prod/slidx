/**
 * A frame arriving, and everyone hearing about it.
 *
 * This is the specification for the step between "bytes on a socket" and "the
 * room changed", plus the fan-out that follows. Everything a WebSocket
 * contributes is a string in and a string out, so none of this needs a network
 * — which is the point of keeping the hub separate from the object that holds
 * the real sockets.
 *
 * The failure modes it guards:
 *
 * - A question that vanishes without an answer, which gets asked again, and
 *   again, until the queue holds four copies of it.
 * - The pending queue arriving on an ordinary participant's socket.
 * - One socket that died between frames aborting the fan-out to the other two
 *   hundred.
 * - A room that ended still holding its connections open.
 */

import { describe, expect, it } from "vitest";

import type { ServerMessage } from "../src/protocol";
import { createRoomHub, type Sink } from "../src/worker";
import { ask, HOST_KEY, open, roomFixture, type Fixture } from "./support";

interface Recorder extends Sink {
  received(): ServerMessage[];
  closedWith(): string | null;
}

function recorder(options: { broken?: boolean } = {}): Recorder {
  const frames: ServerMessage[] = [];
  let closed: string | null = null;

  return {
    send(data) {
      if (options.broken) throw new Error("this socket is gone");
      frames.push(JSON.parse(data) as ServerMessage);
    },
    close(_code, reason) {
      closed = reason ?? "";
    },
    received: () => frames,
    closedWith: () => closed,
  };
}

function join(fixture: Fixture, hub: ReturnType<typeof createRoomHub>, hostKey: string | null) {
  const sink = recorder();
  return { sink, connection: hub.attach(sink, hostKey, fixture.connection()) };
}

const frame = (message: unknown) => JSON.stringify(message);

describe("receiving a frame", () => {
  it("tells the asker their question is waiting for the speaker", async () => {
    // Silence is what makes somebody ask the same thing four times.
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { sink, connection } = join(fixture, hub, null);

    await hub.receive(connection, frame({ type: "question", text: "why not Zig?" }));

    expect(sink.received()[0]).toEqual({ type: "accepted", held: true });
  });

  it("tells the asker when the room is unmoderated and it went straight up", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const hub = createRoomHub(fixture.room);
    const { sink, connection } = join(fixture, hub, null);

    await hub.receive(connection, frame({ type: "question", text: "why not Zig?" }));

    expect(sink.received()[0]).toEqual({ type: "accepted", held: false });
  });

  it("answers an upvote with the room rather than an acknowledgement", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const id = await ask(fixture, "asked");
    const hub = createRoomHub(fixture.room);
    const { sink, connection } = join(fixture, hub, null);

    await hub.receive(connection, frame({ type: "upvote", questionId: id }));

    expect(sink.received().map((message) => message.type)).toEqual(["state"]);
  });

  it("rejects a bad frame to its sender and to nobody else", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const sender = join(fixture, hub, null);
    const bystander = join(fixture, hub, null);

    await hub.receive(sender.connection, "not json at all");

    expect(sender.sink.received()).toEqual([{ type: "rejected", reason: "malformed" }]);
    expect(bystander.sink.received()).toEqual([]);
  });

  it("refuses a binary frame rather than guessing at it", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { sink, connection } = join(fixture, hub, null);

    await hub.receive(connection, new ArrayBuffer(8));

    expect(sink.received()).toEqual([{ type: "rejected", reason: "malformed" }]);
  });

  it("passes the room's refusal through, so the sender learns why", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { sink, connection } = join(fixture, hub, null);

    await hub.receive(connection, frame({ type: "question", text: "a".repeat(400) }));

    expect(sink.received()).toEqual([{ type: "rejected", reason: "too-long" }]);
  });
});

describe("the fan-out", () => {
  it("reaches every connection", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const hub = createRoomHub(fixture.room);
    const first = join(fixture, hub, null);
    const second = join(fixture, hub, null);

    await hub.receive(first.connection, frame({ type: "question", text: "why not Zig?" }));

    expect(second.sink.received().at(-1)).toMatchObject({ type: "state" });
  });

  it("gives the speaker the queue and gives nobody else it", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const speaker = join(fixture, hub, HOST_KEY);
    const participant = join(fixture, hub, null);

    await hub.receive(participant.connection, frame({ type: "question", text: "why not Zig?" }));
    await hub.broadcast();

    const toSpeaker = speaker.sink.received().at(-1);
    const toParticipant = participant.sink.received().at(-1);

    expect(toSpeaker).toMatchObject({ state: { pending: [{ text: "why not Zig?" }] } });
    expect(toParticipant).toMatchObject({ type: "state" });
    expect(toParticipant).not.toHaveProperty("state.pending");
  });

  it("is not stopped by one socket that died between frames", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const dead = recorder({ broken: true });
    hub.attach(dead, null, fixture.connection());
    const alive = join(fixture, hub, null);

    await hub.broadcast();

    expect(alive.sink.received()).toHaveLength(1);
  });

  it("drops a connection that cannot be written to", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    hub.attach(recorder({ broken: true }), null, fixture.connection());

    await hub.broadcast();

    expect(hub.size()).toBe(0);
  });

  it("forgets a connection that hung up", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { connection } = join(fixture, hub, null);

    hub.detach(connection);

    expect(hub.size()).toBe(0);
  });
});

describe("the room ending", () => {
  it("tells everyone why and hangs up", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { sink } = join(fixture, hub, null);

    hub.shutdown("ended-by-speaker");

    expect(sink.received().at(-1)).toEqual({ type: "closed", reason: "ended-by-speaker" });
    expect(sink.closedWith()).toBe("ended-by-speaker");
    expect(hub.size()).toBe(0);
  });

  it("shuts the connections down when a broadcast finds the room expired", async () => {
    // The alarm can be missed. A broadcast against a room that is already gone
    // is the other place that gets noticed.
    const fixture = roomFixture();
    await open(fixture);
    const hub = createRoomHub(fixture.room);
    const { sink } = join(fixture, hub, null);

    fixture.time.advance(24 * 60 * 60_000);
    await hub.broadcast();

    expect(sink.received().at(-1)).toEqual({ type: "closed", reason: "expired" });
  });
});
