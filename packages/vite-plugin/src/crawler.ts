/**
 * The two files a crawler asks the *site* for, rather than the deck.
 *
 * Both are composed in Rust beside the tags they have to agree with — see
 * `slidx_render::seo` — so what is left here is where each one goes, which is
 * the half that depends on how the build is laid out.
 *
 * They go to different places for a reason. A sitemap may only list URLs at or
 * below its own directory, so it sits with the slides and a deck can be one
 * part of somebody's site. `robots.txt` is read at `/robots.txt` and nowhere
 * else, which makes it the one file a deck writes outside its own base — and
 * therefore the one that can already belong to someone.
 */

import { ROBOTS_FILE_NAME, sitemapFileName, type ResolvedOptions } from "./options";

/** Just enough of the Rollup context to write a file, so tests need no plugin. */
export interface Emitter {
  emitFile: (file: { type: "asset"; fileName: string; source: string }) => unknown;
}

/** What the pipeline composed, when it composed anything. */
export interface CrawlerFiles {
  sitemap?: string | undefined;
  robots?: string | undefined;
}

/**
 * Writes the sitemap and, unless the project has its own, the robots file.
 *
 * A `robots.txt` the project already ships is left alone. It is the whole
 * site's file and this plugin owns a directory of it, so replacing it could
 * open a site up as easily as close it down. What still holds in that case is
 * the `robots` meta on each page, which is why a draft deck says it both ways
 * rather than relying on either.
 */
export async function emitCrawlerFiles(
  emitter: Emitter,
  built: CrawlerFiles,
  options: ResolvedOptions,
  publicDir: string | false,
): Promise<void> {
  // Absent for a deck nobody has given an address: a `<loc>` is defined as a
  // full URL, so a relative sitemap is an invalid file rather than a lenient
  // one.
  if (built.sitemap) {
    emitter.emitFile({
      type: "asset",
      fileName: sitemapFileName(options),
      source: built.sitemap,
    });
  }

  if (built.robots && !(await hasOwnRobots(publicDir))) {
    emitter.emitFile({ type: "asset", fileName: ROBOTS_FILE_NAME, source: built.robots });
  }
}

/** Whether the project already ships a `robots.txt` of its own. */
async function hasOwnRobots(publicDir: string | false): Promise<boolean> {
  if (publicDir === false) return false;

  const { access } = await import("node:fs/promises");
  const { join } = await import("node:path");

  return access(join(publicDir, ROBOTS_FILE_NAME)).then(
    () => true,
    () => false,
  );
}
