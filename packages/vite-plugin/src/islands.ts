/**
 * The one client entry a deck with framework islands opts into.
 *
 * A setup module exports an `IslandRegistry`; this module turns it into the
 * browser entry Vite bundles. Nothing is generated, resolved, or injected when
 * the option is absent, and even an opted-in deck puts the entry only on pages
 * whose rendered Markdown actually contains an island.
 */

import { createRequire } from "node:module";
import { posix, resolve } from "node:path";

/** The public module URL used by dev pages. */
export const ISLAND_CLIENT_PATH = "/__slidx/islands.js";
/** Rollup's input id when a deck opted into islands. */
export const ISLAND_CLIENT_ID = "virtual:slidx-islands";
/** The id after this plugin has claimed it. */
export const RESOLVED_ISLAND_CLIENT_ID = `\0${ISLAND_CLIENT_ID}`;

const ISLAND_ATTRIBUTE = "data-slidx-island";

/** A Vite entry that loads the deck's registry and hydrates only marked elements. */
export function islandClientModule(root: string, setup: string, dev = false): string {
  const require = createRequire(import.meta.url);
  const runtime = modulePath(require.resolve("@slidx/islands"), dev);
  const registry = modulePath(resolve(root, setup), dev);

  return [
    `import registry from ${JSON.stringify(registry)};`,
    `import { hydrateIslands } from ${JSON.stringify(runtime)};`,
    "",
    "hydrateIslands(document, { registry });",
    "",
  ].join("\n");
}

/** Adds the client to one rendered page if, and only if, it contains an island. */
export function withIslandClient(html: string, source: string): string {
  if (!hasIsland(html)) return html;

  const escaped = source.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
  const script = `<script type="module" src="${escaped}"></script>\n`;

  return html.replace("</body>", `${script}</body>`);
}

/** The client chunk as seen from one emitted HTML file. */
export function islandClientSource(pageFile: string, clientFile: string): string {
  const relative = posix.relative(posix.dirname(pageFile), clientFile);
  return relative.startsWith(".") ? relative : `./${relative}`;
}

function hasIsland(html: string): boolean {
  return html.includes(ISLAND_ATTRIBUTE);
}

/** Vite and Rollup use forward-slash module ids on every platform. */
function modulePath(path: string, dev: boolean): string {
  const normalised = path.replaceAll("\\", "/");
  return dev ? `/@fs/${normalised}` : normalised;
}
