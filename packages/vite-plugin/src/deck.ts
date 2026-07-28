/**
 * Finding slide files and joining them into one deck source.
 *
 * One file per slide is the recommended layout: diffs stay small, reordering
 * is a rename, and two people can edit different slides without touching the
 * same file. A single file holding several slides still works, because small
 * decks and pasted drafts should not need a directory.
 *
 * Both shapes are joined into one string and handed to the parser, so the deck
 * format has exactly one implementation regardless of how it was stored.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

/** A slide file, as found on disk. */
export interface SlideFile {
  /** Absolute path, for watching and for error messages. */
  path: string;
  /** Path relative to the project root, for logs. */
  label: string;
  source: string;
}

/** Everything read from `srcDir`, in deck order. */
export interface DeckSource {
  files: SlideFile[];
  /** The joined source handed to the parser. */
  source: string;
}

/**
 * Reads a deck from a directory.
 *
 * Files sort by name, which is why the convention is `0001.md`: numeric
 * prefixes sort correctly as strings, and inserting a slide between two others
 * is a rename rather than a renumbering of everything after it.
 */
export async function readDeck(
  root: string,
  srcDir: string,
  extensions: string[],
  separator: string,
): Promise<DeckSource> {
  const directory = join(root, srcDir);
  const names = await listSlideFiles(directory, extensions);

  const files = await Promise.all(
    names.map(async (name) => {
      const path = join(directory, name);
      return { path, label: relative(root, path), source: await readFile(path, "utf8") };
    }),
  );

  return { files, source: joinSources(files, separator) };
}

async function listSlideFiles(directory: string, extensions: string[]): Promise<string[]> {
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    // A missing directory is the state every new project starts in. The plugin
    // reports it as guidance, not as a crash.
    return [];
  }

  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .filter((name) => extensions.some((extension) => name.toLowerCase().endsWith(extension)))
    .sort((a, b) => a.localeCompare(b, "en"));
}

/**
 * Joins slide files with the deck separator.
 *
 * The first file's frontmatter is the deck's, which is why it is left at the
 * very start: the parser reads deck metadata from the top of the source.
 */
function joinSources(files: SlideFile[], separator: string): string {
  return files.map((file) => file.source.trim()).join(`\n\n${separator}\n`);
}
