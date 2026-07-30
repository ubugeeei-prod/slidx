/**
 * The editing session the dev server holds open.
 *
 * The visual editor runs in the browser and the deck lives on the author's
 * disk, so something has to sit between them. This is that something, and it is
 * deliberately thin: it reads the files, hands the operation to the pipeline,
 * writes back what came out, and says what the deck now looks like. It makes no
 * decision about Markdown at all.
 *
 * # Why it only exists in dev
 *
 * These routes write to the author's files. They are registered in
 * `configureServer` and nowhere else, so a built deck has no way to reach them
 * — the same structural guarantee that keeps JavaScript off an audience slide,
 * for a much sharper reason.
 *
 * # The one thing it remembers
 *
 * A slide file that loses its last slide is deleted. If the session forgot it,
 * undoing that deletion would have nowhere to put the slide back and it would
 * land in a neighbouring file — a session that ends where it began would still
 * leave a diff. So an emptied file keeps its place in the list until the dev
 * server stops.
 *
 * # Sharing changes who may ask, and nothing else
 *
 * When a share secret has been issued, every route here is answered only for a
 * request that is allowed to reach it: reading needs a secret, editing needs a
 * *different* secret, and loopback is the author and needs neither. The answers
 * themselves are identical either way — a co-presenter's editor and the
 * author's are the same page talking to the same routes, which is what keeps
 * this from becoming two servers with two behaviours.
 */

import type { IncomingMessage, ServerResponse } from "node:http";

import { createRoom, type Room } from "./collab/room";
import { readDeck, type DeckSource } from "./deck";
import { EDITOR_MODULE, EDITOR_PAGE, editorPage, readEditor } from "./editor";
import { joinDeck } from "./files";
import { createDeckHistory } from "./history";
import { createSharing, isLoopback, CREDENTIAL_HEADER, Grant, type Sharing } from "./share";
import {
  applyOperation,
  locate,
  revertOperation,
  writeDeck,
  type DeckFile,
  type Edit,
  type EditOp,
  type FileWrite,
} from "./edit";
import type { ResolvedOptions } from "./options";
import type { Measurement } from "./overflow";
import { build as buildDeck, lintMeasured } from "./pipeline";
import { readThemePackages } from "./themes";

/** Everything the editor posts to, under one prefix nothing else claims. */
export const EDITOR_ROUTE_PREFIX = "/__slidx/";

const DECK_ROUTE = `${EDITOR_ROUTE_PREFIX}deck`;
const EDIT_ROUTE = `${EDITOR_ROUTE_PREFIX}edit`;
const MEASURED_ROUTE = `${EDITOR_ROUTE_PREFIX}measured`;
const HISTORY_ROUTE = `${EDITOR_ROUTE_PREFIX}history`;
const CHANGE_ROUTE = `${EDITOR_ROUTE_PREFIX}history/change`;
const RESTORE_ROUTE = `${EDITOR_ROUTE_PREFIX}history/restore`;

/** What the editor posts: one operation, or one edit off its undo stack. */
interface EditRequest {
  op?: EditOp;
  edit?: Edit;
  /** The commit to put the deck back to, on the restore route. */
  rev?: string;
}

/** What the editor measured in its canvas, for the linter to read. */
interface MeasuredRequest {
  measured?: Measurement[];
}

/** Reads and writes one project's deck for as long as the dev server runs. */
export interface EditSession {
  /** True when the request was ours and has been answered. */
  handle(request: IncomingMessage, response: ServerResponse): Promise<boolean>;
  /**
   * The deck's files as a commit had them, for a slide URL asking to be shown
   * as of one. `null` when this repository has no such commit.
   */
  deckAt(rev: string): Promise<DeckSource | null>;
  /**
   * Re-reads the deck and tells every connected editor.
   *
   * Called by the watcher. This is the other half of the merge story: a file the
   * author saved in their own editor becomes a splice in the shared document
   * here, rather than waiting for the next operation to notice.
   */
  refresh(): Promise<void>;
  /** Ends every held connection. */
  close(): void;
}

export interface SessionOptions {
  /** Injected so a test can share a deck without setting process-wide state. */
  sharing?: Sharing;
}

export function createEditSession(
  root: string,
  options: ResolvedOptions,
  session: SessionOptions = {},
): EditSession {
  const emptied = new Map<string, DeckFile>();
  const history = createDeckHistory(root, options);
  const sharing = session.sharing ?? createSharing();
  const room = createRoom({ deckState: () => current() });

  async function files(): Promise<DeckFile[]> {
    const { files: found } = await readDeck(
      root,
      options.srcDir,
      options.extensions,
      options.separator,
    );

    return placeEmptied(found, emptied);
  }

  /**
   * Everything the editor needs to draw itself once.
   *
   * The parsed model, the diagnostics that go with it, and where each slide's
   * bytes are — the last of these is what turns a selection in the canvas into
   * the byte range an operation names.
   */
  async function state(source: string) {
    // Read each time rather than once at startup, because `vp add` during a dev
    // session is exactly when an author installs a theme — and an editor that
    // kept reporting `dialect/unknown-theme` until the server restarted would
    // be reporting on a project that no longer exists.
    const themePackages = await readThemePackages(root);

    const [deck, located] = await Promise.all([
      buildDeck(source, {
        theme: options.theme,
        themePackages,
        separator: options.separator,
        parseOnly: true,
      }),
      locate(source, options.separator),
    ]);

    return { source, spans: located.slides, deck };
  }

  /** The deck as the files say it reads, right now. */
  async function current() {
    return state(joinDeck(await files(), options.separator).source);
  }

  return {
    deckAt: (rev) => history.deckAt(rev),
    async refresh() {
      const deck = await current();
      // Through the reconciler rather than straight into the room, because a
      // file that changed on disk is a splice like any other and has to enter
      // the shared document by the one door.
      room.reconciler.adopt(deck.source);
      room.announce(deck);
    },

    close: () => room.close(),

    async handle(request, response) {
      const url = request.url ?? "";
      const path = url.split("?")[0]!;
      if (!path.startsWith(EDITOR_ROUTE_PREFIX)) return false;

      const local = isLoopback(request.socket.remoteAddress);
      const grant = sharing.grant(credential(request), request.socket.remoteAddress);

      // A shared deck answers only what the presented secret allows. Editing is
      // a second secret rather than a flag on the first, so a viewer cannot
      // reach this branch with the link they were given.
      if (grant === Grant.None || (grant === Grant.Read && path === EDIT_ROUTE)) {
        send(response, 403, { message: refused(grant) });
        return true;
      }

      try {
        if (await room.handle(request, response, { grant, local })) return true;

        if (path === EDITOR_PAGE && request.method === "GET") {
          const deck = await state(joinDeck(await files(), options.separator).source);
          // The generated deck type says `null` because that is what serde
          // writes across the boundary; everything on this side of it says
          // `undefined`. One conversion, here, rather than two vocabularies
          // for absence leaking through the plugin.
          page(response, editorPage(options.base, deck.deck.title ?? undefined));
          return true;
        }

        if (path === EDITOR_MODULE && request.method === "GET") {
          script(response, await readEditor());
          return true;
        }

        if (path === DECK_ROUTE && request.method === "GET") {
          send(response, 200, await state(joinDeck(await files(), options.separator).source));
          return true;
        }

        if (path === EDIT_ROUTE && request.method === "POST") {
          send(response, 200, await edit(await read<EditRequest>(request)));
          return true;
        }

        // The one route that changes nothing. Whether content fits its box
        // depends on where lines break, so the editor measures its canvas and
        // the pipeline says what the numbers mean — the same rule the build
        // runs, which is what lets the editor warn before a block has landed
        // rather than after it has shipped.
        if (path === MEASURED_ROUTE && request.method === "POST") {
          const { measured = [] } = await read<MeasuredRequest>(request);
          const source = joinDeck(await files(), options.separator).source;

          send(response, 200, await lintMeasured(source, measured, options));
          return true;
        }

        if (path === HISTORY_ROUTE && request.method === "GET") {
          send(response, 200, await history.commits());
          return true;
        }

        if (path === RESTORE_ROUTE && request.method === "POST") {
          // A write, so it is a POST — and it goes through git rather than
          // through a file write from the browser, which is the same rule the
          // editing routes keep by handing every change to `slidx_edit`.
          const { rev } = await read<EditRequest>(request);

          send(response, 200, await history.restore(rev ?? ""));
          return true;
        }

        if (path === CHANGE_ROUTE && request.method === "GET") {
          const change = await history.changeAt(query(url, "rev"));

          // A revision this repository does not have is an answer rather than
          // a failure: the panel builds its request from a log it read a
          // moment ago, and a rebase since then is ordinary traffic.
          send(response, change === null ? 404 : 200, change ?? { message: "No such revision." });
          return true;
        }

        send(response, 404, { message: `No editor route at ${path}.` });
      } catch (error) {
        // A deck whose files cannot be written to is the one failure worth
        // stopping for: writing half of it would be worse than writing none.
        send(response, 409, { message: error instanceof Error ? error.message : String(error) });
      }

      return true;
    },
  };

  async function edit(payload: EditRequest) {
    const before = await files();
    // The reconciler is only threaded in when somebody is sharing. With nobody
    // else connected the pipeline is the same function it has always been, and
    // the bytes it writes are the ones the splice named.
    const reconciler = sharing.on ? room.reconciler : undefined;
    const result = payload.edit
      ? await revertOperation(before, options.separator, payload.edit, reconciler)
      : await applyOperation(before, options.separator, payload.op ?? {}, reconciler);

    if (result.error) return { error: result.error, ...(await state(result.source)) };

    await writeDeck(result.writes);
    remember(before, result.writes, emptied);

    const answer = {
      undo: result.undo,
      written: result.writes.map((write) => write.label),
      ...(await state(result.source)),
    };

    // Everyone else sees the deck the operation produced without asking for it.
    // The author's own editor already has this in the reply it is reading.
    room.announce(answer);

    return answer;
  }
}

/** Why a request was refused, in words a co-presenter can act on. */
function refused(grant: Grant): string {
  return grant === Grant.Read
    ? "This link can read the deck but not change it. Editing needs a link from `slidx dev --crdt --allow-edit`."
    : "This deck is shared by link, and this request carried no valid one.";
}

/**
 * The share credential a request presented.
 *
 * A header rather than a query parameter, for the same reason the secret lives
 * in the fragment: a query string reaches an access log, and this is the value
 * that log would then be enough to replay.
 */
function credential(request: IncomingMessage): string | undefined {
  const presented = request.headers[CREDENTIAL_HEADER];

  return Array.isArray(presented) ? presented[0] : presented;
}

/**
 * The files on disk, with the ones this session emptied back in their places.
 *
 * Sorted the way the reader sorts them, so a restored file lands between the
 * same two neighbours it was between when it was deleted.
 */
function placeEmptied(found: DeckFile[], emptied: Map<string, DeckFile>): DeckFile[] {
  if (emptied.size === 0) return found;

  const missing = [...emptied.values()].filter(
    (gone) => !found.some((file) => file.path === gone.path),
  );

  return [...found, ...missing].sort((a, b) => a.label.localeCompare(b.label, "en"));
}

function remember(
  before: DeckFile[],
  writes: readonly FileWrite[],
  emptied: Map<string, DeckFile>,
): void {
  for (const write of writes) {
    if (write.source === null) {
      emptied.set(write.path, { path: write.path, label: write.label, source: "" });
    } else {
      emptied.delete(write.path);
    }
  }

  // A file the author restored by hand is no longer this session's to hold.
  for (const file of before) {
    if (file.source.length > 0) emptied.delete(file.path);
  }
}

/**
 * One query parameter, or an empty string.
 *
 * Parsed against a base that is thrown away — these URLs arrive without an
 * origin, and `URL` is the only decoder in the runtime that agrees with the
 * browser about what `%2F` and `+` mean.
 */
function query(url: string, name: string): string {
  return new URL(url, "http://deck.invalid").searchParams.get(name) ?? "";
}

async function read<T>(request: IncomingMessage): Promise<T> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(chunk as Buffer);

  const body = Buffer.concat(chunks).toString("utf8");
  return (body ? JSON.parse(body) : {}) as T;
}

function page(response: ServerResponse, html: string): void {
  response.statusCode = 200;
  response.setHeader("content-type", "text/html; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.end(html);
}

function script(response: ServerResponse, source: string): void {
  response.statusCode = 200;
  response.setHeader("content-type", "text/javascript; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.end(source);
}

function send(response: ServerResponse, status: number, payload: unknown): void {
  response.statusCode = status;
  response.setHeader("content-type", "application/json; charset=utf-8");
  // The deck changes under the editor constantly; a cached answer is a wrong
  // one within a keystroke.
  response.setHeader("cache-control", "no-store");
  response.end(JSON.stringify(payload));
}
