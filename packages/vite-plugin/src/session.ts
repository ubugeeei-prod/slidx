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
 */

import type { IncomingMessage, ServerResponse } from "node:http";

import { readDeck } from "./deck";
import { joinDeck } from "./files";
import {
  applyOperation,
  revertOperation,
  writeDeck,
  type DeckFile,
  type Edit,
  type EditOp,
  type FileWrite,
} from "./edit";
import type { ResolvedOptions } from "./options";
import { build as buildDeck } from "./pipeline";

/** Everything the editor posts to, under one prefix nothing else claims. */
export const EDITOR_ROUTE_PREFIX = "/__slidx/";

const DECK_ROUTE = `${EDITOR_ROUTE_PREFIX}deck`;
const EDIT_ROUTE = `${EDITOR_ROUTE_PREFIX}edit`;

/** What the editor posts: one operation, or one edit off its undo stack. */
interface EditRequest {
  op?: EditOp;
  edit?: Edit;
}

/** Reads and writes one project's deck for as long as the dev server runs. */
export interface EditSession {
  /** True when the request was ours and has been answered. */
  handle(request: IncomingMessage, response: ServerResponse): Promise<boolean>;
}

export function createEditSession(root: string, options: ResolvedOptions): EditSession {
  const emptied = new Map<string, DeckFile>();

  async function files(): Promise<DeckFile[]> {
    const { files: found } = await readDeck(
      root,
      options.srcDir,
      options.extensions,
      options.separator,
    );

    return placeEmptied(found, emptied);
  }

  async function state(source: string) {
    const deck = await buildDeck(source, {
      theme: options.theme,
      separator: options.separator,
      parseOnly: true,
    });

    return { source, deck };
  }

  return {
    async handle(request, response) {
      const path = (request.url ?? "").split("?")[0]!;
      if (!path.startsWith(EDITOR_ROUTE_PREFIX)) return false;

      try {
        if (path === DECK_ROUTE && request.method === "GET") {
          send(response, 200, await state(joinDeck(await files(), options.separator).source));
          return true;
        }

        if (path === EDIT_ROUTE && request.method === "POST") {
          send(response, 200, await edit(await read(request)));
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
    const current = await files();
    const result = payload.edit
      ? await revertOperation(current, options.separator, payload.edit)
      : await applyOperation(current, options.separator, payload.op ?? {});

    if (result.error) return { error: result.error, ...(await state(result.source)) };

    await writeDeck(result.writes);
    remember(current, result.writes, emptied);

    return {
      undo: result.undo,
      written: result.writes.map((write) => write.label),
      ...(await state(result.source)),
    };
  }
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

async function read(request: IncomingMessage): Promise<EditRequest> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(chunk as Buffer);

  const body = Buffer.concat(chunks).toString("utf8");
  return body ? (JSON.parse(body) as EditRequest) : {};
}

function send(response: ServerResponse, status: number, payload: unknown): void {
  response.statusCode = status;
  response.setHeader("content-type", "application/json; charset=utf-8");
  // The deck changes under the editor constantly; a cached answer is a wrong
  // one within a keystroke.
  response.setHeader("cache-control", "no-store");
  response.end(JSON.stringify(payload));
}
