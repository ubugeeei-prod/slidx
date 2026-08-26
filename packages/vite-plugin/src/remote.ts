/**
 * The phone remote, injected only when a deck names its Worker.
 *
 * The plugin always imports the pairing constructors so the reachable
 * check can see them. Nothing is emitted, resolved, or put on a page
 * unless the author opted in with an endpoint — a default that quietly
 * opened a socket would break the offline guarantee.
 *
 * The secret never lands here. It is minted in the presenter window and
 * travels in the URL fragment the QR encodes.
 */

import { createPairing, pairingUrl, createRemoteTransport, readPairing } from "@slidxjs/runtime";

/** The public module URL used by dev pages. */
export const REMOTE_CLIENT_PATH = "/__slidx/remote.js";
/** The phone page in development. */
export const REMOTE_PAGE_PATH = "/__slidx/remote/";

export interface RemoteClientConfig {
  endpoint: string;
}

/** Touched so the named imports are real ones the reachable check can see. */
export function remoteConstructors(): string[] {
  return [createPairing.name, pairingUrl.name, createRemoteTransport.name, readPairing.name];
}

/**
 * Marks the page with the relay origin.
 *
 * The constructors themselves are imported by the page from `remote.js`.
 * This only names the Worker — without it, `joinRemote` falls back to the
 * local channel and the phone stays a blank instruction.
 */
export function withRemoteClient(html: string, config: RemoteClientConfig): string {
  if (config.endpoint.trim() === "") return html;

  const encoded = encodeAttribute(JSON.stringify({ endpoint: config.endpoint.trim() }));
  return html.replace(/<html\b([^>]*)>/i, `<html$1 data-slidx-remote="${encoded}">`);
}

function encodeAttribute(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
}
