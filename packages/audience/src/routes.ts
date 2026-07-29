/**
 * The room's HTTP surface.
 *
 * Small on purpose: open a room, read it, moderate it, end it. Everything that
 * happens continuously happens on the socket instead, so this is the part a
 * speaker's presenter view drives and nothing else needs.
 *
 * Kept apart from the Durable Object that hosts it so the API can be exercised
 * with a plain `Request` and a room over a Map. What is left in the object is
 * the socket plumbing, which is the only part that genuinely needs the
 * platform.
 */

import type { Room } from "./room";
import { isRoomSlug, type ModerationMode } from "./protocol";

export interface RouteContext {
  room: Room;
  /**
   * Turns the request into a WebSocket. Supplied by the Durable Object.
   *
   * Receives the host key only once it has been checked, so the socket layer
   * never decides who the speaker is. Optional because everything else here is
   * testable without it, and a room without it should answer 501 rather than
   * pretend.
   */
  upgrade?: (request: Request, hostKey: string | null) => Response | Promise<Response>;
}

/**
 * Cross-origin access, granted broadly and deliberately.
 *
 * A deck is served from wherever it is published and the room is served from
 * a Worker, so these are always different origins. `*` is safe here in a way it
 * usually is not: the API has no cookies and no ambient authority, so a browser
 * that reaches it on some other page's behalf can do nothing the page could not
 * have done by asking directly. Authority comes only from the host key, which
 * the speaker's own view holds.
 */
const CORS_HEADERS: Record<string, string> = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, POST, DELETE, OPTIONS",
  "access-control-allow-headers": "authorization, content-type",
  "access-control-max-age": "86400",
};

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      // A room's state is live and personal to the moment. A cache between the
      // deck and the Worker showing a stale question queue is worse than none.
      "cache-control": "no-store",
      ...CORS_HEADERS,
    },
  });
}

function fail(status: number, error: string): Response {
  return json({ error }, status);
}

/**
 * The speaker's key, from a header or the query string.
 *
 * The header is the right place and the only one the HTTP routes use. The
 * query string exists because a browser cannot set headers on a WebSocket
 * handshake — there is no other way for the presenter view to authenticate its
 * own socket. It is a per-room token with a lifetime measured in hours, so its
 * appearance in a URL is a bounded exposure rather than an open one.
 */
function hostKey(request: Request, url: URL): string | null {
  const header = request.headers.get("authorization");
  if (header?.startsWith("Bearer ")) return header.slice("Bearer ".length);

  return url.searchParams.get("key");
}

async function readOpenBody(
  request: Request,
): Promise<{ hostKey: string; moderation?: ModerationMode; lifetimeMs?: number } | null> {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return null;
  }

  if (typeof body !== "object" || body === null) return null;

  const fields = body as Record<string, unknown>;
  if (typeof fields["hostKey"] !== "string") return null;

  const moderation = fields["moderation"];
  const lifetimeMs = fields["lifetimeMs"];

  return {
    hostKey: fields["hostKey"],
    // Anything other than the literal string is the default. A typo in
    // frontmatter must not be a way to end up unmoderated by accident.
    ...(moderation === "open" ? { moderation: "open" as const } : {}),
    ...(typeof lifetimeMs === "number" && Number.isFinite(lifetimeMs) ? { lifetimeMs } : {}),
  };
}

/** `/rooms/<slug>/rest` split into the slug and what follows it. */
export function splitRoomPath(pathname: string): { slug: string; rest: string } | null {
  const match = /^\/rooms\/([^/]+)(\/.*)?$/.exec(pathname);
  if (!match) return null;

  const slug = decodeURIComponent(match[1] ?? "");
  if (!isRoomSlug(slug)) return null;

  return { slug, rest: match[2] ?? "" };
}

export async function routeRoomRequest(request: Request, context: RouteContext): Promise<Response> {
  if (request.method === "OPTIONS")
    return new Response(null, { status: 204, headers: CORS_HEADERS });

  const url = new URL(request.url);
  const path = splitRoomPath(url.pathname);
  if (!path) return fail(404, "no such room");

  const { room } = context;
  const { rest } = path;

  if (rest === "/socket") {
    if (!context.upgrade) return fail(501, "sockets unavailable");

    // A room that is not open must not accept sockets: an unopened slug that
    // silently collects questions would show them to a speaker who never
    // agreed to run a Q&A at all.
    if (!(await room.snapshot())) return fail(404, "no such room");

    const key = hostKey(request, url);
    const speaker = key !== null && (await room.hostSnapshot(key)) !== null ? key : null;

    // A wrong key joins as an ordinary participant rather than being refused.
    // Somebody watching the talk on the presenter link should still be able to
    // ask a question when the key in it has gone stale.
    return context.upgrade(request, speaker);
  }

  if (rest === "") {
    if (request.method === "POST") {
      const body = await readOpenBody(request);
      if (!body) return fail(400, "expected a hostKey");

      const outcome = await room.open(body);
      if (!outcome.ok) {
        return outcome.reason === "taken"
          ? fail(409, "that room is already open")
          : fail(400, "the host key is too short to be a secret");
      }

      return json(outcome.snapshot);
    }

    if (request.method === "GET") {
      const snapshot = await room.snapshot();
      return snapshot ? json(snapshot) : fail(404, "no such room");
    }

    if (request.method === "DELETE") {
      const key = hostKey(request, url);
      const outcome = key === null ? null : await room.end(key);

      return outcome?.ok ? json({ ended: true }) : fail(403, "forbidden");
    }

    return fail(405, "method not allowed");
  }

  if (rest === "/pending" && request.method === "GET") {
    const key = hostKey(request, url);
    const snapshot = key === null ? null : await room.hostSnapshot(key);

    // 403 rather than 404 for a wrong key on a room that exists would tell a
    // guesser which slugs are live. Both answers are the same shape.
    return snapshot ? json(snapshot) : fail(403, "forbidden");
  }

  const moderation = /^\/questions\/([^/]+)\/(approve|dismiss)$/.exec(rest);
  if (moderation && request.method === "POST") {
    const key = hostKey(request, url);
    if (key === null) return fail(403, "forbidden");

    const questionId = decodeURIComponent(moderation[1] ?? "");
    const outcome =
      moderation[2] === "approve"
        ? await room.approve(questionId, key)
        : await room.dismiss(questionId, key);

    if (outcome.ok) return json({ [String(moderation[2])]: questionId });

    return outcome.reason === "forbidden" ? fail(403, "forbidden") : fail(404, "no such question");
  }

  return fail(404, "not found");
}
