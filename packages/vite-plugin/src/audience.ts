/**
 * The audience channel client, injected only when a deck names its Worker.
 *
 * The plugin always imports `@slidxjs/audience` so the reachable check can see
 * the client graph. Nothing is emitted, resolved, or put on a page unless the
 * author opted in with an endpoint and a room — a default that quietly opened
 * a socket would break the offline guarantee the rest of the deck is built on.
 *
 * Presenter pages may carry a host key. Audience pages never do.
 */

import { createRequire } from "node:module";
import { posix } from "node:path";

import { createAudienceChannel, isRoomSlug } from "@slidxjs/audience";

/** The public module URL used by dev pages. */
export const AUDIENCE_CLIENT_PATH = "/__slidx/audience.js";
/** Rollup's input id when a deck opted into the channel. */
export const AUDIENCE_CLIENT_ID = "virtual:slidx-audience";
/** The id after this plugin has claimed it. */
export const RESOLVED_AUDIENCE_CLIENT_ID = `\0${AUDIENCE_CLIENT_ID}`;

export interface AudienceClientConfig {
  endpoint: string;
  room: string;
  hostKey?: string;
}

/** A Vite entry that opens the channel from the page's own data attribute. */
export function audienceClientModule(dev = false): string {
  const require = createRequire(import.meta.url);
  const audience = modulePath(require.resolve("@slidxjs/audience"), dev);
  // Touched so the named import is a real one the reachable check can see.
  void createAudienceChannel.name;

  return [
    `import { createAudienceChannel } from ${JSON.stringify(audience)};`,
    "",
    'const raw = document.documentElement.getAttribute("data-slidx-audience");',
    "if (raw) createAudienceChannel(JSON.parse(raw));",
    "",
  ].join("\n");
}

/**
 * Marks the page with the channel config and loads the client.
 *
 * Returns the html unchanged when the room slug is not one the Worker will
 * accept — inventing a fallback name would open a different room from the one
 * the author named.
 */
export function withAudienceClient(
  html: string,
  source: string,
  config: AudienceClientConfig,
): string {
  if (!isRoomSlug(config.room) || config.endpoint.trim() === "") return html;

  const payload: Record<string, string> = {
    endpoint: config.endpoint,
    room: config.room,
  };
  if (config.hostKey) payload.hostKey = config.hostKey;

  const encoded = encodeAttribute(JSON.stringify(payload));
  const scriptSrc = encodeAttribute(source);
  const marked = html.replace(/<html\b([^>]*)>/i, `<html$1 data-slidx-audience="${encoded}">`);
  const script = `<script type="module" src="${scriptSrc}"></script>\n`;

  return marked.replace("</body>", `${script}</body>`);
}

/** The client chunk as seen from one emitted HTML file. */
export function audienceClientSource(pageFile: string, clientFile: string): string {
  const relative = posix.relative(posix.dirname(pageFile), clientFile);
  return relative.startsWith(".") ? relative : `./${relative}`;
}

function encodeAttribute(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
}

/** Vite and Rollup use forward-slash module ids on every platform. */
function modulePath(path: string, dev: boolean): string {
  const normalised = path.replaceAll("\\", "/");
  return dev ? `/@fs/${normalised}` : normalised;
}
