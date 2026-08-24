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
import { dirname, join } from "node:path";

import { implementation } from "./rust-source.mjs";
import {
  barrelExports,
  EMITTED_BUNDLES,
  importsIn,
  PUBLIC_API,
  UNREACHABLE,
  walk,
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

/** Every package, by name, with where its barrel is. */
const barrels = new Map();
for (const manifest of tracked("packages", /^packages\/[^/]+\/package\.json$/)) {
  const directory = dirname(manifest);
  const { name } = JSON.parse(readFileSync(manifest, "utf8"));
  if (name !== undefined) barrels.set(name, `${directory}/src/index.ts`);
}

const modules = new Map();
for (const file of tracked("packages", /\.(ts|mts)$/)) {
  if (file.endsWith(".d.ts")) continue;

  modules.set(file, {
    source: readFileSync(file, "utf8"),
    barrel: [...barrels.values()].includes(file),
  });
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

/** What the roots ask for: the crates' emitted imports, and the plugin's barrel. */
const entries = [];

for (const file of tracked("crates", /\.rs$/)) {
  const source = implementation(readFileSync(file, "utf8")).text;

  for (const { specifier, names } of importsIn(source)) {
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

const total = stalePackages.length + appeared.length + closed.length;
const carried = unreachable.length - appeared.length;

if (total === 0) {
  const held = carried === 0 ? "" : `, ${carried} carried in UNREACHABLE against an open issue`;
  console.log(
    `reachable: ${modules.size} modules across ${barrels.size} packages, every one on a path from a page${held}.`,
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
