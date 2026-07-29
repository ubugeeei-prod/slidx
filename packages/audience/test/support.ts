/**
 * The fakes the audience suite runs on.
 *
 * A Durable Object cannot be constructed in a test and its clock cannot be
 * moved, so the room takes both as arguments. What is here is the other side of
 * that bargain: a Map that behaves like Cloudflare's storage, and a clock the
 * test moves by hand.
 *
 * Values are cloned on the way in and out, because the real storage serialises
 * them. A room that accidentally relied on holding the same object reference as
 * its storage would pass against a plain Map and fail in production.
 */

import { createParticipant, type Participant } from "../src/participant";
import { createRoom, type Room, type RoomStorage } from "../src/room";

/** Long enough to be a secret, so it is the key length that is not under test. */
export const HOST_KEY = "speaker-key-for-tests";

export interface Clock {
  now: () => number;
  advance: (ms: number) => void;
}

export function clock(start = 1_700_000_000_000): Clock {
  let current = start;

  return {
    now: () => current,
    advance: (ms) => {
      current += ms;
    },
  };
}

export interface MemoryStorage extends RoomStorage {
  keys(): string[];
}

export function memoryStorage(): MemoryStorage {
  const data = new Map<string, unknown>();

  return {
    async get<T>(key: string): Promise<T | undefined> {
      const value = data.get(key);
      return value === undefined ? undefined : (structuredClone(value) as T);
    },

    async put<T>(key: string, value: T): Promise<void> {
      data.set(key, structuredClone(value));
    },

    async delete(key: string): Promise<boolean> {
      return data.delete(key);
    },

    async list<T>(options?: { prefix?: string }): Promise<Map<string, T>> {
      const prefix = options?.prefix ?? "";
      const matches = new Map<string, T>();

      for (const [key, value] of [...data].sort(([left], [right]) => left.localeCompare(right))) {
        if (key.startsWith(prefix)) matches.set(key, structuredClone(value) as T);
      }

      return matches;
    },

    async deleteAll(): Promise<void> {
      data.clear();
    },

    keys: () => [...data.keys()],
  };
}

export interface Fixture {
  room: Room;
  storage: MemoryStorage;
  time: Clock;
  /** A fresh connection, with its own allowance and its own vote memory. */
  connection(): Participant;
  /** Sets the number of connections the room believes are attached. */
  setPresent(count: number): void;
}

export function roomFixture(options: { slug?: string; storage?: MemoryStorage } = {}): Fixture {
  const time = clock();
  const storage = options.storage ?? memoryStorage();
  let present = 0;

  const room = createRoom({
    slug: options.slug ?? "a-talk",
    storage,
    now: time.now,
    present: () => present,
  });

  return {
    room,
    storage,
    time,
    connection: () => createParticipant({ now: time.now }),
    setPresent: (count) => {
      present = count;
    },
  };
}

/** Opens a room with a key that is not itself the thing under test. */
export async function open(
  fixture: Fixture,
  overrides: { moderation?: "held" | "open"; lifetimeMs?: number; hostKey?: string } = {},
): Promise<void> {
  const outcome = await fixture.room.open({
    hostKey: overrides.hostKey ?? HOST_KEY,
    ...(overrides.moderation === undefined ? {} : { moderation: overrides.moderation }),
    ...(overrides.lifetimeMs === undefined ? {} : { lifetimeMs: overrides.lifetimeMs }),
  });

  if (!outcome.ok) throw new Error(`the fixture could not open a room: ${outcome.reason}`);
}

/** Submits a question and returns the id it was given, for tests that need one. */
export async function ask(
  fixture: Fixture,
  text: string,
  participant: Participant = fixture.connection(),
): Promise<string> {
  await fixture.room.submit({ type: "question", text }, participant);

  const host = await fixture.room.hostSnapshot(HOST_KEY);
  // Ids are a padded sequence, so the newest is the highest — which the
  // published list, sorted by votes, would not tell us.
  const newest = [...(host?.pending ?? []), ...(host?.questions ?? [])].sort((left, right) =>
    left.id.localeCompare(right.id),
  );

  const last = newest.at(-1);
  if (!last) throw new Error("the question was not accepted");
  return last.id;
}
