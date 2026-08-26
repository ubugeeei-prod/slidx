/**
 * The pairing session's HTTP surface.
 *
 * One route: upgrade a socket. Everything that happens after that is a
 * frame on the socket, so there is no REST resource to GET and nothing
 * here that could leak a secret into a log. The session id is in the path
 * because it is not secret; the secret arrives in the first frame.
 *
 * Kept apart from the Durable Object so the path rules can be exercised
 * with a plain `Request`.
 */

import { isSessionId } from "./relay";

export interface SessionRouteContext {
  /**
   * Turns the request into a WebSocket. Supplied by the Durable Object.
   *
   * Optional because everything else here is testable without it, and a
   * session without it should answer 501 rather than pretend.
   */
  upgrade?: (request: Request) => Response | Promise<Response>;
}

const CORS_HEADERS: Record<string, string> = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, OPTIONS",
  "access-control-allow-headers": "content-type",
  "access-control-max-age": "86400",
};

function fail(status: number, error: string): Response {
  return new Response(JSON.stringify({ error }), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
      ...CORS_HEADERS,
    },
  });
}

/** `/sessions/<id>/rest` split into the id and what follows it. */
export function splitSessionPath(pathname: string): { id: string; rest: string } | null {
  const match = /^\/sessions\/([^/]+)(\/.*)?$/.exec(pathname);
  if (!match) return null;

  const id = decodeURIComponent(match[1] ?? "");
  if (!isSessionId(id)) return null;

  return { id, rest: match[2] ?? "" };
}

export async function routeSessionRequest(
  request: Request,
  context: SessionRouteContext,
): Promise<Response> {
  if (request.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: CORS_HEADERS });
  }

  const url = new URL(request.url);
  const path = splitSessionPath(url.pathname);
  if (!path) return fail(404, "no such session");

  if (path.rest !== "/socket") return fail(404, "not found");
  if (request.method !== "GET") return fail(405, "method not allowed");
  if (!context.upgrade) return fail(501, "sockets unavailable");

  return context.upgrade(request);
}
