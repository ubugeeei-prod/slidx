/**
 * Which theme packages the project installed.
 *
 * A theme package is a token document — a palette, a type scale, spacing and
 * motion — and a `slidx.theme` key in its own `package.json` naming the file.
 * Reading it happens here because the pipeline runs inside WebAssembly, where
 * there is no filesystem and no module resolver, exactly as image headers are
 * read here and parsed there.
 *
 * The *rules* still live in Rust. What a document may say, which name wins, and
 * whether a theme is legible in the room are `slidx_theme::package`'s, so the
 * editor's live preview and the production build cannot disagree about a theme
 * the way two implementations always eventually do. This file finds bytes.
 *
 * # Installing is the declaration
 *
 * Only the project's *direct* dependencies are looked at. Nothing has to be
 * imported, registered, or named in `vite.config.ts` — `vp add -D
 * @slidx/theme-workshop` and `theme: workshop` is the whole path.
 *
 * Direct rather than the whole tree, and that is the load-bearing half. A
 * transitive dependency is not something the author chose, and a theme arriving
 * from one would be a package the deck never asked for deciding what the deck
 * looks like. It is also what keeps this cheap: a handful of manifest reads,
 * not a walk of `node_modules`.
 *
 * # Why the manifest is read rather than resolved
 *
 * `require.resolve` answers for modules, and a theme document is not one:
 * nothing imports it and no bundler sees it. A package with an `exports` map
 * that does not list `./package.json` — which is most of them — cannot be
 * resolved this way at all, so a resolver would fail on the packages most
 * likely to be doing everything else correctly.
 *
 * Walking `node_modules` upwards is what pnpm's symlinks, npm's hoisting and a
 * workspace root all have in common. Yarn PnP has no `node_modules` and is not
 * covered; a deck there gets `dialect/unknown-theme` and renders with the
 * default, which is a stated gap rather than a silent one.
 */

import { readFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

import type { ThemePackage } from "@slidx/wasm";

/** The manifest section slidx owns, holding `theme` and nothing else yet. */
const MANIFEST_KEY = "slidx";

/**
 * Every theme document the project's direct dependencies provide.
 *
 * Never throws. A missing manifest, an unreadable document, a dependency that
 * is not installed — all of them are a project that simply has no theme
 * package here, and a build that fails because a directory was mid-install is
 * a build nobody can act on. What a document turns out to *contain* is Rust's
 * to complain about, and it does.
 */
export async function readThemePackages(root: string): Promise<ThemePackage[]> {
  const found: ThemePackage[] = [];

  for (const name of await dependencyNames(root)) {
    const document = await readThemeDocument(root, name);
    if (document !== undefined) found.push({ source: name, document });
  }

  // By name, so two builds of one tree hand the documents over in the same
  // order — which is what makes the collision rule on the Rust side settle a
  // duplicate id the same way every time rather than by directory listing.
  //
  // The locale is pinned for the same reason: `localeCompare` without one
  // sorts by whatever the machine is set to, which would make "the same order
  // every time" true on one developer's laptop and not on CI.
  return found.sort((one, other) => one.source.localeCompare(other.source, "en"));
}

async function dependencyNames(root: string): Promise<string[]> {
  const manifest = await readManifest(join(root, "package.json"));
  if (manifest === undefined) return [];

  // `devDependencies` counts: a theme is a build-time input, and `-D` is what
  // the install line in every slidx package's README says.
  return Object.keys({
    ...asRecord(manifest["dependencies"]),
    ...asRecord(manifest["devDependencies"]),
  });
}

/** One dependency's theme document, if it ships one. */
async function readThemeDocument(root: string, name: string): Promise<string | undefined> {
  const installed = await installedPackage(root, name);
  if (installed === undefined) return undefined;

  const { directory, manifest } = installed;
  const declared = asRecord(manifest[MANIFEST_KEY])["theme"];
  if (typeof declared !== "string" || declared === "") return undefined;

  const path = resolve(directory, declared);

  // A package may only name a file inside itself. Everything under this is the
  // author's own disk, but a dependency that can point `slidx.theme` at
  // `../../.ssh/id_ed25519` is a dependency that can have its contents read
  // into a page — and a theme document's strings do reach one.
  if (!contains(directory, path)) return undefined;

  try {
    return await readFile(path, "utf8");
  } catch {
    return undefined;
  }
}

/**
 * Where a dependency is installed, walking `node_modules` up from the deck.
 *
 * Up rather than down: a monorepo hoists to its root and a pnpm store symlinks
 * only the direct dependencies into each package's own `node_modules`, and
 * looking upward is the one traversal both arrangements answer.
 */
async function installedPackage(
  root: string,
  name: string,
): Promise<{ directory: string; manifest: Record<string, unknown> } | undefined> {
  let directory = resolve(root);

  for (;;) {
    const candidate = join(directory, "node_modules", ...name.split("/"));
    const manifest = await readManifest(join(candidate, "package.json"));
    if (manifest !== undefined) return { directory: candidate, manifest };

    const parent = dirname(directory);
    if (parent === directory) return undefined;
    directory = parent;
  }
}

async function readManifest(path: string): Promise<Record<string, unknown> | undefined> {
  try {
    const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
    return typeof parsed === "object" && parsed !== null
      ? (parsed as Record<string, unknown>)
      : undefined;
  } catch {
    return undefined;
  }
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
}

/** True when `path` is something inside `directory`.
 *
 * `isAbsolute` is not redundant with the `..` check: on Windows, `relative`
 * between two drives returns the target unchanged, which begins with neither.
 */
function contains(directory: string, path: string): boolean {
  const step = relative(directory, path);

  return step !== "" && !step.startsWith("..") && !isAbsolute(step);
}
