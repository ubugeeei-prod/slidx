/**
 * Who is allowed to do what, when a deck is shared beyond this machine.
 *
 * The dev server writes to the author's files. Putting it on a network is
 * therefore a security decision and not a convenience, and this module is the
 * whole of that decision: nothing else in the plugin decides whether a request
 * may edit.
 *
 * # Four rules, and none of them is a default anyone can drift past
 *
 * **Nothing is shared unless somebody asked.** With no share secret in the
 * environment the routes behave exactly as they always have, on loopback, and
 * this module answers `Grant.Write` to every request — because the only thing
 * that can reach loopback is the author.
 *
 * **A link is a capability, and the secret travels in the fragment.** A
 * fragment is not sent with the request, so it reaches no access log, no
 * referrer header and no proxy record. The shape is
 * [`readPairing`](@slidxjs/runtime)'s, and the reader *is* `readPairing` — one
 * answer in this repository rather than two, and a URL that arrived with its
 * secret in the query is refused rather than honoured.
 *
 * **Read-only is the default; editing is a second secret.** Not a flag on the
 * same token — a different sixteen bytes, which a viewer was never given.
 * Somebody sharing their screen at a conference has not handed out the right to
 * rewrite their talk, and that is true by construction rather than by policy.
 *
 * **Loopback is always the author.** A request from `127.0.0.1` is this machine,
 * so the bookmark the author already had keeps working when sharing is switched
 * on. Everything else has to present a secret.
 */

import { timingSafeEqual } from "node:crypto";
import type { IncomingMessage, ServerResponse } from "node:http";

import { readPairing, type Pairing } from "@slidxjs/runtime";

/** What a request is allowed to do. */
export enum Grant {
  /** Not allowed to see the deck at all. */
  None = "none",
  /** May read the deck and say where in it they are. */
  Read = "read",
  /** May also change it. */
  Write = "write",
}

/**
 * Where the secrets come from.
 *
 * `slidx dev --crdt` mints them and passes them in the environment, so they
 * never touch the project directory and are not in the argument list of a
 * process anybody can list. The plugin can be used without slidx, in which case
 * these are unset and nothing is shared.
 */
export const SHARE_VARIABLE = "SLIDX_SHARE";
export const SHARE_EDIT_VARIABLE = "SLIDX_SHARE_EDIT";
/** Public LAN origin the CLI printed, used only to rebuild its links locally. */
export const SHARE_ORIGIN_VARIABLE = "SLIDX_SHARE_ORIGIN";

/** The header the editor presents its share credential in. */
export const CREDENTIAL_HEADER = "x-slidx-share";

/**
 * Browser-held reading access for the deck pages an authenticated editor embeds.
 *
 * It is a session cookie: closing the browser drops it, and stopping the dev
 * server invalidates the capability it contains. A cookie is always capped at
 * reading by [`grantForRequest`]; writes continue to require the explicit
 * header JavaScript derives from the URL fragment.
 */
export const CREDENTIAL_COOKIE = "slidx_share";

export interface Sharing {
  /** True when a secret was issued, which is the only time anything is shared. */
  readonly on: boolean;
  /** Links the local author may copy again. Never returned to a remote peer. */
  readonly links?: SharingLinks;
  /** What a request presenting this credential from this address may do. */
  grant(credential: string | undefined, address: string | undefined): Grant;
}

/** The capabilities the CLI printed when it started this shared session. */
export interface SharingLinks {
  readonly read?: string;
  readonly edit?: string;
}

/**
 * Reads the environment once.
 *
 * A malformed secret is treated as no secret at all rather than as a secret
 * nobody can match: half-configured sharing that silently refused every
 * request would look exactly like a network problem at a conference.
 */
export function createSharing(environment: NodeJS.ProcessEnv = process.env): Sharing {
  const read = pairing(environment[SHARE_VARIABLE]);
  const write = pairing(environment[SHARE_EDIT_VARIABLE]);
  const on = read !== null || write !== null;
  const links = sharingLinks(environment, read !== null, write !== null);

  return {
    on,
    ...(links ? { links } : {}),

    grant(credential, address) {
      if (!on || isLoopback(address)) return Grant.Write;

      const presented = pairing(credential);
      if (presented === null) return Grant.None;

      if (matches(presented, write)) return Grant.Write;
      if (matches(presented, read)) return Grant.Read;

      return Grant.None;
    },
  };
}

/**
 * Rebuilds the fragment links only when the CLI supplied their public origin.
 *
 * The server cannot infer this from a local editor request: its `Host` is
 * usually `localhost`, while the useful link names the machine on the LAN.
 * Rejecting paths, credentials and non-HTTP schemes keeps an environment typo
 * from turning the share sheet into a link to somewhere else.
 */
function sharingLinks(
  environment: NodeJS.ProcessEnv,
  hasRead: boolean,
  hasEdit: boolean,
): SharingLinks | undefined {
  const origin = shareOrigin(environment[SHARE_ORIGIN_VARIABLE]);
  if (origin === undefined) return undefined;

  const read = hasRead ? environment[SHARE_VARIABLE] : undefined;
  const edit = hasEdit ? environment[SHARE_EDIT_VARIABLE] : undefined;

  return {
    ...(read ? { read: `${origin}/__slidx/#s=${read}` } : {}),
    ...(edit ? { edit: `${origin}/__slidx/#s=${edit}` } : {}),
  };
}

function shareOrigin(value: string | undefined): string | undefined {
  if (!value) return undefined;

  try {
    const parsed = new URL(value);
    if (
      (parsed.protocol !== "http:" && parsed.protocol !== "https:") ||
      parsed.username ||
      parsed.password ||
      (parsed.pathname !== "/" && parsed.pathname !== "") ||
      parsed.search ||
      parsed.hash
    )
      return undefined;

    return parsed.origin;
  } catch {
    return undefined;
  }
}

/**
 * Access presented by one HTTP request.
 *
 * An explicit header retains its full grant. A browser cookie is deliberately
 * capped at reading so another page on the same site cannot turn an ambient
 * credential into a file-writing request.
 */
export function grantForRequest(sharing: Sharing, request: IncomingMessage): Grant {
  const header = headerCredential(request);
  if (header !== undefined) return sharing.grant(header, request.socket.remoteAddress);

  const cookie = cookieCredential(request);
  const grant = sharing.grant(cookie, request.socket.remoteAddress);
  return cookie !== undefined && grant === Grant.Write ? Grant.Read : grant;
}

/** The explicit capability the editor presents after reading its fragment. */
export function headerCredential(request: IncomingMessage): string | undefined {
  const presented = request.headers[CREDENTIAL_HEADER];
  return Array.isArray(presented) ? presented[0] : presented;
}

/**
 * Lets subsequent same-origin slide and asset requests carry reading access.
 *
 * Only called after the header was accepted. `HttpOnly` keeps the credential
 * out of deck scripts, and `SameSite=Strict` keeps it off cross-site requests.
 */
export function rememberReadAccess(request: IncomingMessage, response: ServerResponse): void {
  const presented = headerCredential(request);
  if (presented === undefined) return;

  response.setHeader(
    "set-cookie",
    `${CREDENTIAL_COOKIE}=${encodeURIComponent(presented)}; Path=/; HttpOnly; SameSite=Strict`,
  );
}

function cookieCredential(request: IncomingMessage): string | undefined {
  const cookies = request.headers.cookie;
  if (!cookies) return undefined;

  const value = cookies
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${CREDENTIAL_COOKIE}=`))
    ?.slice(CREDENTIAL_COOKIE.length + 1);
  if (value === undefined) return undefined;

  try {
    return decodeURIComponent(value);
  } catch {
    return undefined;
  }
}

/**
 * One credential, read the way a phone remote's pairing is read.
 *
 * The value is the fragment's own text, so it is handed back to `readPairing`
 * with the `#` it was found after. That keeps the refusal of a secret in a
 * query string in one place — this module never learns the rule.
 */
function pairing(value: string | undefined): Pairing | null {
  if (value === undefined || value.length === 0) return null;

  return readPairing(`#s=${value}`);
}

/**
 * Whether two pairings are the same, without leaking how far they matched.
 *
 * A capability guarded by nothing but its own length has to be compared in
 * constant time: a byte-at-a-time comparison tells an attacker on the same
 * network which prefix was right, and sixteen bytes guessed one byte at a time
 * is not sixteen bytes.
 */
function matches(presented: Pairing, issued: Pairing | null): boolean {
  if (issued === null) return false;

  return same(presented.session, issued.session) && same(presented.secret, issued.secret);
}

function same(a: string, b: string): boolean {
  const left = Buffer.from(a, "utf8");
  const right = Buffer.from(b, "utf8");

  // `timingSafeEqual` throws on a length mismatch, which is itself a leak of
  // one bit. A length that differs cannot be the issued secret anyway, because
  // slidx issues one length.
  return left.length === right.length && timingSafeEqual(left, right);
}

/**
 * True for the machine the dev server is running on.
 *
 * Node reports an IPv4 peer over a dual-stack socket as `::ffff:127.0.0.1`, so
 * the mapped form has to be recognised too — missing it would lock the author
 * out of their own editor the moment they shared it.
 */
export function isLoopback(address: string | undefined): boolean {
  if (address === undefined) return false;

  const plain = address.replace(/^::ffff:/, "");

  return plain === "127.0.0.1" || plain === "::1" || plain.startsWith("127.");
}
