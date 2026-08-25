/**
 * Cloudflare Pages, as a file on disk and a command the author runs.
 *
 * A built deck is a directory of HTML. Cloudflare Pages will host that
 * directory; slidx will not. What this target writes is the `wrangler.toml`
 * Pages reads, and what it prints is `wrangler pages deploy` — the author is
 * logged into *their* Cloudflare account, and slidx still has no HTTP client
 * and no token store.
 */

import { ask, source, type CloudflarePages, type Composed, type SourceInput } from "../boundary";

export function composeCloudflare(input: SourceInput): Composed<CloudflarePages> {
  return ask<Composed<CloudflarePages>>({ op: "composeCloudflare", ...source(input) });
}

/** One line for a printed plan. */
export function describeCloudflare(pages: CloudflarePages): string {
  return ask<string>({ op: "describeCloudflare", pages });
}

export type { CloudflarePages } from "../boundary";
