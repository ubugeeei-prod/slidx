/**
 * The Cloudflare entry point: a Worker in front of one Durable Object per room.
 *
 * A room is a single consistent thing that many sockets are attached to, which
 * is the one problem Durable Objects solve and ordinary Workers cannot: two
 * requests to the same room reach the same object, in order, with the same
 * storage. The slug from the deck's frontmatter is the object's name, so a
 * deck's room is wherever its slug says it is, with nothing to provision.
 *
 * The platform types are written out here structurally rather than imported.
 * `@cloudflare/workers-types` is a global type package — pulling it into this
 * workspace would replace the DOM globals the deck-side half of this package is
 * compiled against, so it stays a devDependency for the deployment's own
 * tsconfig to use. Naming the handful of methods this file actually needs is
 * also what lets everything except the socket handshake be tested without
 * workerd.
 */

import { createParticipant, type Participant } from "./participant";
import type { RoomEndReason, RoomSnapshot, ServerMessage } from "./protocol";
import { createRoom, type Room, type RoomStorage } from "./room";
import { routeRoomRequest, splitRoomPath } from "./routes";
import { receiveFrame } from "./session";

/** A WebSocket as workerd hands it over. */
interface PlatformSocket {
  accept(): void;
  send(data: string): void;
  close(code?: number, reason?: string): void;
  addEventListener(type: "message", handler: (event: { data: unknown }) => void): void;
  addEventListener(type: "close" | "error", handler: () => void): void;
}

declare const WebSocketPair: { new (): { 0: PlatformSocket; 1: PlatformSocket } };

/** `Response` gains this field inside workerd; the DOM's does not have it. */
interface SocketResponseInit extends ResponseInit {
  webSocket: PlatformSocket;
}

interface AlarmStorage extends RoomStorage {
  setAlarm(scheduledTime: number): Promise<void>;
}

export interface DurableObjectStateLike {
  storage: AlarmStorage;
}

export interface DurableObjectNamespaceLike {
  idFromName(name: string): unknown;
  get(id: unknown): { fetch(request: Request): Promise<Response> };
}

export interface AudienceEnv {
  AUDIENCE_ROOM: DurableObjectNamespaceLike;
}

/**
 * Anything a room can talk back to.
 *
 * The fan-out is written against this rather than against a WebSocket so it can
 * be tested with an array. The parts of this file that genuinely need workerd
 * are the two lines that make a socket pair.
 */
export interface Sink {
  send(data: string): void;
  close(code?: number, reason?: string): void;
}

interface Connection {
  sink: Sink;
  participant: Participant;
  /** Present for the speaker's own socket, and the reason it sees the queue. */
  hostKey: string | null;
}

export interface RoomHub {
  attach(sink: Sink, hostKey: string | null, participant: Participant): Connection;
  detach(connection: Connection): void;
  size(): number;
  receive(connection: Connection, raw: unknown): Promise<void>;
  broadcast(): Promise<void>;
  /** Tells everyone the room is over and hangs up. */
  shutdown(reason: RoomEndReason): void;
}

/**
 * A room's connected sockets.
 *
 * Every send is wrapped: a socket that died between the last frame and this one
 * throws on write, and one dead connection must not stop the other two hundred
 * from being told what happened.
 */
export function createRoomHub(room: Room): RoomHub {
  const connections = new Set<Connection>();

  const post = (connection: Connection, message: ServerMessage) => {
    try {
      connection.sink.send(JSON.stringify(message));
    } catch {
      connections.delete(connection);
    }
  };

  const hub: RoomHub = {
    attach(sink, hostKey, participant) {
      const connection: Connection = { sink, participant, hostKey };
      connections.add(connection);
      return connection;
    },

    detach(connection) {
      connections.delete(connection);
    },

    size: () => connections.size,

    async receive(connection, raw) {
      // Binary frames are not part of this protocol. Refusing them is cheaper
      // and clearer than decoding something nobody meant to send.
      if (typeof raw !== "string") {
        post(connection, { type: "rejected", reason: "malformed" });
        return;
      }

      await receiveFrame(raw, {
        room,
        participant: connection.participant,
        reply: (message) => post(connection, message),
        broadcast: () => hub.broadcast(),
      });
    },

    async broadcast() {
      const shared = await room.snapshot();
      if (!shared) {
        hub.shutdown("expired");
        return;
      }

      for (const connection of connections) {
        const state = await viewFor(room, connection, shared);
        post(connection, { type: "state", state });
      }
    },

    shutdown(reason) {
      for (const connection of connections) {
        post(connection, { type: "closed", reason });
        try {
          // 1001 "going away": the room ended, so a client that reconnects
          // gets a 404 and stops, rather than retrying against nothing.
          connection.sink.close(1001, reason);
        } catch {
          // Already gone. Nothing to do and nothing to report.
        }
      }

      connections.clear();
    },
  };

  return hub;
}

async function viewFor(
  room: Room,
  connection: Connection,
  shared: RoomSnapshot,
): Promise<RoomSnapshot> {
  if (connection.hostKey === null) return shared;

  // Re-checked on every broadcast rather than trusted from the handshake, so a
  // speaker who ends and reopens a room under a new key does not keep feeding
  // the queue to a socket authenticated against the old one.
  return (await room.hostSnapshot(connection.hostKey)) ?? shared;
}

/**
 * One room, alive for as long as its slug is being used.
 *
 * Registered in `wrangler.toml` as a Durable Object class. Everything it knows
 * how to do lives in `room.ts`; what is here is the platform: sockets, the
 * alarm that ends the room on time, and the storage handed over at construction.
 */
export class AudienceRoom {
  readonly #storage: AlarmStorage;
  #room: Room | null = null;
  #hub: RoomHub | null = null;

  constructor(state: DurableObjectStateLike) {
    this.#storage = state.storage;
  }

  async fetch(request: Request): Promise<Response> {
    const path = splitRoomPath(new URL(request.url).pathname);
    if (!path) return new Response("no such room", { status: 404 });

    const room = this.#roomFor(path.slug);
    const response = await routeRoomRequest(request, {
      room,
      upgrade: (upgradeRequest, hostKey) => this.#upgrade(upgradeRequest, hostKey),
    });

    // Re-armed after every request. An alarm is the platform's way of waking a
    // room up to end itself, and setting it to a time already known is free;
    // missing it entirely would leave a room's storage sitting there until
    // somebody happened to ask about it.
    const endsAt = await room.endsAt();
    if (endsAt !== null) await this.#storage.setAlarm(endsAt);

    return response;
  }

  /** Fired when the room's time is up. */
  async alarm(): Promise<void> {
    const room = this.#room;
    if (!room) return;

    const ended = await room.sweep();
    if (ended) this.#hub?.shutdown(ended);
  }

  #roomFor(slug: string): Room {
    if (!this.#room) {
      const hub = () => this.#hub?.size() ?? 0;
      this.#room = createRoom({ slug, storage: this.#storage, present: hub });
      this.#hub = createRoomHub(this.#room);
    }

    return this.#room;
  }

  #upgrade(request: Request, hostKey: string | null): Response {
    if (request.headers.get("upgrade") !== "websocket") {
      return new Response("expected a websocket", { status: 426 });
    }

    const hub = this.#hub;
    if (!hub) return new Response("no such room", { status: 404 });

    const pair = new WebSocketPair();
    const client = pair[0];
    const server = pair[1];

    server.accept();

    const connection = hub.attach(
      { send: (data) => server.send(data), close: (code, reason) => server.close(code, reason) },
      hostKey,
      createParticipant(),
    );

    server.addEventListener("message", (event) => {
      void hub.receive(connection, event.data);
    });
    server.addEventListener("close", () => hub.detach(connection));
    server.addEventListener("error", () => hub.detach(connection));

    // The joiner needs the room as it is now; waiting for somebody else to act
    // would leave a participant looking at an empty screen in a busy room.
    void hub.broadcast();

    const init: SocketResponseInit = { status: 101, webSocket: client };
    return new Response(null, init);
  }
}

/**
 * The Worker in front of the objects.
 *
 * It does one thing: turn a slug into the object that owns it. The slug is
 * validated before it becomes a Durable Object name, because an unvalidated one
 * is an unbounded namespace anybody can allocate in.
 */
export async function handleFetch(request: Request, env: AudienceEnv): Promise<Response> {
  const path = splitRoomPath(new URL(request.url).pathname);
  if (!path) return new Response("not found", { status: 404 });

  const stub = env.AUDIENCE_ROOM.get(env.AUDIENCE_ROOM.idFromName(path.slug));
  return stub.fetch(request);
}

export const audienceWorker = { fetch: handleFetch };

export default audienceWorker;
