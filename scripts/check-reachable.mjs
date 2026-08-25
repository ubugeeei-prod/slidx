/**
 * Reports a module no page can run.
 *
 * The scan; `reachable.mjs` holds the rules, and holds them apart so they can be
 * exercised without walking a workspace — the split `write-only.mjs` already
 * makes, for the same reason.
 *
 * A failure rather than a warning. `check-dead-config.mjs` warns because a
 * write-only field is a real shape, so its output is a prompt to look. This one
 * has no equivalent: a module exists so that something can call it, and nothing
 * being able to is the whole defect. The two honest exceptions — somebody else's
 * code is the consumer, and a type is not importable at runtime — are handled in
 * `reachable.mjs` rather than by being lenient here.
 *
 * What the already-unreachable modules do *not* get is a grace period. They are
 * in `UNREACHABLE` with the issue that closes each, which fails on anything new
 * and fails again when a recorded one becomes reachable — so the list can only
 * shrink, and it shrinks in the same commit that earns it.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";

import { implementation } from "./rust-source.mjs";
import {
  barrelExports,
  EMITTED_BUNDLES,
  EMITTED_EXACTLY,
  importsIn,
  PUBLIC_API,
  UNREACHABLE,
  walk,
  wranglerMain,
} from "./reachable.mjs";

/**
 * The root that is a package rather than a language.
 *
 * A deck's own `vite.config.ts` imports the plugin, so every name on its barrel
 * is requested by definition and none of them can be reported. That makes it a
 * `PUBLIC_API` entry as well as a root, and it is spelled out here rather than
 * in both lists because the two facts have one cause.
 */
const PLUGIN = "@slidxjs/vite-plugin";

const TEST_FILE = /(^|\/)tests?\/|[._](test|spec)\./;

function tracked(root, pattern) {
  return execFileSync("git", ["ls-files", "-z", root], { encoding: "utf8" })
    .split("\0")
    .filter((file) => pattern.test(file) && !TEST_FILE.test(file));
}

/**
 * A tracked file's text, or nothing when it is not there.
 *
 * `git ls-files` answers about the index, and the index is not the disk: a file
 * deleted and not yet staged is listed and unreadable. That is the ordinary
 * state halfway through removing a module, which is one of the two things this
 * check exists to prompt — so it reports on what remains rather than exiting
 * with a stack trace at the moment somebody is taking its advice.
 */
function read(file) {
  try {
    return readFileSync(file, "utf8");
  } catch {
    return undefined;
  }
}

/**
 * Every entry a specifier can name, mapped to the source behind it.
 *
 * Both a package and its subpaths: `@slidxjs/runtime` and
 * `@slidxjs/runtime/emitted` are two files a page can be handed, and only one
 * of them is what a room downloads.
 */
const barrels = new Map();
/** Names that are a package rather than one of its entries, for the summary. */
const packages = new Set();
for (const manifest of tracked("packages", /^packages\/[^/]+\/package\.json$/)) {
  const directory = dirname(manifest);
  const source = read(manifest);
  if (source === undefined) continue;

  const { name, exports = {} } = JSON.parse(source);
  if (name === undefined) continue;

  barrels.set(name, `${directory}/src/index.ts`);
  packages.add(name);

  // A package's own export map says which entries exist. Deriving them from it
  // rather than listing them here means an entry added to a manifest is one
  // this check already knows about.
  for (const [subpath, target] of Object.entries(exports)) {
    const file = typeof target === "string" ? target : (target?.default ?? "");
    if (subpath === "." || !file.endsWith(".mjs")) continue;

    barrels.set(`${name}${subpath.slice(1)}`, `${directory}/src/${basename(file, ".mjs")}.ts`);
  }
}

const modules = new Map();
for (const file of tracked("packages", /\.(ts|mts)$/)) {
  if (file.endsWith(".d.ts")) continue;

  const source = read(file);
  if (source === undefined) continue;

  modules.set(file, { source, barrel: [...barrels.values()].includes(file) });
}

/** A specifier, from the file that wrote it, as a path this walk knows. */
function resolve(from, specifier) {
  const named = barrels.get(EMITTED_BUNDLES[specifier] ?? specifier);
  if (named !== undefined) return modules.has(named) ? named : undefined;

  if (!specifier.startsWith(".")) return undefined;

  const base = join(dirname(from), specifier);
  for (const candidate of [`${base}.ts`, `${base}/index.ts`, base]) {
    if (modules.has(candidate)) return candidate;
  }

  return undefined;
}

/** What the roots ask for: the crates' emitted imports, the plugin's barrel, and wrangler `main`. */
const entries = [];

/** Every name a page asks of each emitted specifier, for the equality rule. */
const asked = new Map();

for (const file of tracked("crates", /\.rs$/)) {
  const rust = read(file);
  if (rust === undefined) continue;

  for (const { specifier, names } of importsIn(implementation(rust).text)) {
    const wanted = asked.get(specifier) ?? new Set();
    for (const name of names) wanted.add(name);
    asked.set(specifier, wanted);

    const path = resolve(file, specifier);
    if (path !== undefined) entries.push({ path, names });
  }
}

const pluginBarrel = barrels.get(PLUGIN);
if (pluginBarrel !== undefined && modules.has(pluginBarrel)) {
  entries.push({
    path: pluginBarrel,
    names: [...barrelExports(modules.get(pluginBarrel).source).values],
  });
}

for (const file of tracked("packages", /\/wrangler\.toml$/)) {
  const source = read(file);
  if (source === undefined) continue;

  const main = wranglerMain(source);
  if (main === undefined) continue;

  const path = join(dirname(file), main);
  if (!modules.has(path)) continue;

  entries.push({
    path,
    names: [...barrelExports(modules.get(path).source).values],
  });
}

const { reached } = walk({ modules, entries, resolve });

/** Which package a path belongs to, so an exempt one can be left out whole. */
function packageOf(path) {
  for (const [name, barrel] of barrels) {
    if (path.startsWith(`${dirname(dirname(barrel))}/`)) return name;
  }

  return undefined;
}

const exempt = new Set([...Object.keys(PUBLIC_API), PLUGIN]);

/**
 * A module that exports only types is not a feature nobody can reach.
 *
 * `export type` erases, so such a file ships no bytes and runs no code — the
 * deck model crossing from Rust, and the editor's operation shapes, are both
 * imported exclusively as types by everything that uses them. Reporting one
 * would be reporting that a type is not called.
 */
function shipsCode(path) {
  return barrelExports(modules.get(path).source).values.size > 0;
}

const unreachable = [...modules.keys()]
  .filter((path) => !reached.has(path) && shipsCode(path))
  .filter((path) => {
    const name = packageOf(path);
    return name !== undefined && !exempt.has(name);
  })
  .sort();

const appeared = unreachable.filter((path) => UNREACHABLE[path] === undefined);

/** A recorded path that a page can now reach, so the ratchet tightens itself. */
const closed = Object.keys(UNREACHABLE)
  .filter((path) => !unreachable.includes(path))
  .sort();

/**
 * A bundle emitted whole, against the names the pages that receive it import.
 *
 * Equality in both directions: a missing name is a deck that breaks on load,
 * and an extra one is bytes a room downloads and cannot run.
 */
const emissionErrors = [];
for (const [bundle, specifier] of Object.entries(EMITTED_EXACTLY)) {
  const barrel = barrels.get(bundle);

  if (barrel === undefined || !modules.has(barrel)) {
    emissionErrors.push(`${bundle} is emitted whole and this workspace has no entry for it.`);
    continue;
  }

  const exported = barrelExports(modules.get(barrel).source).values;
  const wanted = asked.get(specifier) ?? new Set();

  for (const name of [...wanted].filter((one) => !exported.has(one)).sort()) {
    emissionErrors.push(
      `a page imports ${name} and ${barrel} does not export it. That deck breaks on load.`,
    );
  }

  for (const name of [...exported].filter((one) => !wanted.has(one)).sort()) {
    emissionErrors.push(
      `${barrel} exports ${name} and no page imports it. The file is emitted whole, so a room ` +
        `downloads it and cannot run it.`,
    );
  }
}

for (const problem of emissionErrors) console.error(`reachable: ${problem}`);

const stalePackages = Object.keys(PUBLIC_API)
  .filter((name) => !barrels.has(name))
  .sort();

for (const name of stalePackages) {
  console.error(`reachable: PUBLIC_API exempts ${name}, which is not a package in this workspace.`);
}

for (const path of appeared) {
  console.error(`reachable: nothing imports ${path} on any path from a page.`);
}

for (const path of closed) {
  const issue = UNREACHABLE[path];
  console.error(
    `reachable: ${path} is reachable now. Remove its UNREACHABLE entry — #${issue} is done.`,
  );
}

const total = stalePackages.length + appeared.length + closed.length + emissionErrors.length;
const carried = unreachable.length - appeared.length;

if (total === 0) {
  const held = carried === 0 ? "" : `, ${carried} carried in UNREACHABLE against an open issue`;
  console.log(
    `reachable: ${modules.size} modules across ${packages.size} packages, every one on a path from a page${held}.`,
  );
  process.exit(0);
}

console.error("");

if (appeared.length > 0) {
  console.error("reachable: a module exists so that something can call it, and when nothing can,");
  console.error("  the feature behind it is not shipped, however many tests it has. Wire it to a");
  console.error("  page, delete it, or record it in PUBLIC_API with the reason its consumer is");
  console.error(
    "  somebody else's code. UNREACHABLE is for what is already known and being fixed;",
  );
  console.error("  adding a line there needs an issue that says when it comes out again.");
}

process.exit(1);
