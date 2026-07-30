/**
 * The live room: one shared document, one roster, one stream.
 *
 * This is where the three halves of collaborative editing meet, and its whole
 * job is to keep them from becoming three sources of truth. The document
 * ([`collab.ts`](../collab)) decides bytes. The roster decides who is here. The
 * stream is the only way either of those reaches a browser. Nothing in here
 * parses Markdown, plans an edit, or writes a file — it is handed the bytes an
 * operation produced and hands back the bytes to write.
 *
 * # The reconciler is the whole point
 *
 * [`Room.reconciler`] is what the edit pipeline calls instead of writing an
 * operation's result straight to disk. It does two things and nothing else: it
 * folds what the files currently say into the shared document, and it applies
 * the splice between the operation's before and after. Both are byte-range
 * splices, which is what lets an edit from the canvas and a file the author
 * saved in their own editor merge rather than overwrite.
 *
 * With one editor connected and nothing changing on disk, the second of those
 * returns exactly the bytes it was given. That is the property that makes this
 * honest rather than a second writer, and it is asserted in `test/collab.test.ts`
 * against the files themselves.
 */

import type { IncomingMessage, ServerResponse } from "node:http";
import { randomUUID } from "node:crypto";

import { createSharedDeck, spliceBetween, type SharedDeck } from "../collab";
import { Grant } from "../share";
import { createRoster, type Roster, type Viewer } from "./presence";
import { createStream, type Stream } from "./stream";

/** Where a browser subscribes, and where it says which slide it is on. */
export const LIVE_ROUTE = "live";
export const HERE_ROUTE = "here";

/** The events a browser listens for. */
export const HELLO_EVENT = "hello";
export const STATE_EVENT = "state";
export const PRESENCE_EVENT = "presence";

/**
 * Where an operation's bytes are reconciled before they reach a file.
 *
 * Injected into the edit pipeline so that pipeline stays the same function with
 * collaboration off: no reconciler, no shared document, and the splice goes to
 * disk exactly as it always has.
 */
export interface Reconciler {
  /** Folds what the files say into the shared document. */
  adopt(joined: string): void;
  /**
   * Folds the files in and takes the copy the operation will be planned against.
   *
   * Taken *before* the plan rather than after, because planning is where the
   * wait is and the wait is where a file saved elsewhere gets in.
   */
  begin(joined: string): Pending;
}

/** One operation's change, waiting to be merged. */
export interface Pending {
  /** Applies the operation's splice, and hands back the bytes to write. */
  settle(before: string, after: string): string;
}

/**
 * What the request in front of us is allowed to do, and whose machine it is.
 *
 * The two are separate because loopback is answered `Write` and a co-presenter
 * holding an edit link is answered `Write` too. Only one of them is the person
 * whose files these are, and the roster says "you" about exactly one row.
 */
export interface Access {
  grant: Grant;
  local: boolean;
}

export interface Room {
  reconciler: Reconciler;
  /** True when the request was ours and has been answered. */
  handle(request: IncomingMessage, response: ServerResponse, access: Access): Promise<boolean>;
  /** Tells every connected browser what the deck now says. */
  announce(state: unknown): void;
  /** Everyone connected, author first. */
  viewers(): Viewer[];
  close(): void;
}

export interface RoomOptions {
  /** Everything a browser needs to draw the deck once. */
  deckState(): Promise<unknown>;
  stream?: Stream;
  roster?: Roster;
}

export function createRoom(options: RoomOptions): Room {
  const stream = options.stream ?? createStream();
  const roster = options.roster ?? createRoster();

  /**
   * Created on the first operation rather than at start-up.
   *
   * A dev server that nobody has edited in has nothing to reconcile, and
   * seeding a document from files that are about to be read again anyway would
   * be work for a session that may never happen.
   */
  let shared: SharedDeck | undefined;

  function tellEveryone(): void {
    stream.broadcast(PRESENCE_EVENT, { viewers: roster.viewers() });
  }

  const reconciler: Reconciler = {
    adopt(joined) {
      shared ??= createSharedDeck(joined);
      shared.adopt(joined);
    },

    begin(joined) {
      reconciler.adopt(joined);
      const fork = shared!.fork();

      return {
        // The splice is computed against the source the operation was planned
        // from, which is the fork's own text whenever nothing else has written
        // since. Where something has, the fork is what makes the offsets mean
        // anything at all.
        settle: (before, after) => fork.merge(spliceBetween(before, after)),
      };
    },
  };

  return {
    reconciler,
    viewers: () => roster.viewers(),

    announce(state) {
      stream.broadcast(STATE_EVENT, state);
    },

    async handle(request, response, access) {
      const path = (request.url ?? "").split("?")[0]!;
      const route = path.slice(path.lastIndexOf("/") + 1);

      if (route === LIVE_ROUTE && request.method === "GET") {
        const id = randomUUID();
        const canEdit = access.grant === Grant.Write;

        // Read before the response becomes a stream, and not for tidiness.
        // Joining commits `text/event-stream` headers, and a deck that cannot
        // be read after that leaves the caller above unable to answer at all:
        // its error reply would be a second set of headers on a response that
        // already sent one. Failing here is a plain 409 like any other.
        const state = await options.deckState();
        const listener = stream.join(id, response, () =>
          roster.seen(id, { local: access.local, canEdit }),
        );

        roster.seen(id, { local: access.local, canEdit });
        // The id is the browser's only way to say where it is later. Sent
        // rather than assigned by the browser, so a viewer cannot claim to be
        // somebody else's seat.
        listener.send(HELLO_EVENT, { id, canEdit });
        listener.send(STATE_EVENT, state);
        tellEveryone();

        response.on("close", () => {
          roster.gone(id);
          tellEveryone();
        });

        return true;
      }

      if (route === HERE_ROUTE && request.method === "POST") {
        const said = await body(request);
        const id = typeof said["id"] === "string" ? said["id"] : "";
        const slide = typeof said["slide"] === "number" ? said["slide"] : 0;

        roster.moved(id, { slide });
        tellEveryone();

        response.statusCode = 204;
        response.end();

        return true;
      }

      return false;
    },

    close() {
      stream.closeAll();
      shared?.destroy();
      shared = undefined;
    },
  };
}

/**
 * The most a position report may weigh.
 *
 * The only body that reaches here is a seat id and a slide number, so this is
 * two orders of magnitude of room. It exists because a share link is handed to
 * somebody else: a viewer who posts without stopping would otherwise grow the
 * dev server's memory by whatever they felt like sending, and that process is
 * the one holding everybody's deck.
 */
const MAX_BODY_BYTES = 4_096;

async function body(request: IncomingMessage): Promise<Record<string, unknown>> {
  const chunks: Buffer[] = [];
  let size = 0;

  for await (const chunk of request) {
    size += (chunk as Buffer).length;
    // Nonsense of any size is answered the same way, so an oversized body is
    // dropped rather than reported: nothing here is worth a second round trip.
    if (size > MAX_BODY_BYTES) return {};
    chunks.push(chunk as Buffer);
  }

  try {
    const parsed: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {};
  } catch {
    // A browser that posted nonsense is not a reason to drop a dev server.
    return {};
  }
}
