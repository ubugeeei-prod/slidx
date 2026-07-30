/**
 * Packages that something imports, and whether anything builds them.
 *
 * Every package here is consumed through its `exports`, which point at `dist/`.
 * A package the task graph never builds fails in two ways and only one of them
 * announces itself:
 *
 * - On a clean checkout there is no `dist/`, the import does not resolve, and
 *   whatever needed it says so.
 * - On a machine that built it once, a **stale** `dist/` answers instead. A
 *   check runs, passes, and reports on code from whenever that build happened.
 *
 * `@slidx/vite-plugin` sat in the second state. Nothing in `build:packages`
 * built it, two agents lost time to it, and a capability check on a shared dev
 * server was verified against a plugin build old enough not to contain the
 * capability — it looked unenforced when it was not.
 *
 * So this runs *after* a build and asks the filesystem, rather than reading the
 * task graph and believing it. A static check cannot answer this anyway:
 * `build:wasm` runs `node scripts/build-wasm.mjs` and never names the package
 * it produces.
 *
 * The list is derived. A package counts when TypeScript outside its own
 * directory imports it by name — which is what makes its `dist/` load-bearing
 * for this repository rather than only for whoever installs it later.
 * `@slidx/audience`, `@slidx/islands` and `@slidx/rehearsal` are published and
 * imported by nobody here, and are correctly absent.
 */

import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";

function tracked(...paths) {
  const output = execFileSync("git", ["ls-files", "-z", ...paths], { encoding: "utf8" });
  return output.split("\0").filter(Boolean);
}

/** Every workspace package that is consumed through a built `dist/`. */
const packages = tracked("packages")
  .filter((file) => /^packages\/[^/]+\/package\.json$/.test(file))
  .map((file) => ({ manifest: file, ...JSON.parse(readFileSync(file, "utf8")) }))
  .filter(({ exports }) => JSON.stringify(exports ?? {}).includes("./dist/"));

const sources = tracked("packages", "crates", "examples", "scripts").filter((file) =>
  /\.(ts|mts|mjs|js)$/.test(file),
);

/** True when TypeScript outside `directory` imports `name`. */
function importedElsewhere(name, directory) {
  const specifier = new RegExp(`from ["']${name}(/[^"']*)?["']`);

  return sources.some(
    (file) => !file.startsWith(`${directory}/`) && specifier.test(readFileSync(file, "utf8")),
  );
}

const missing = [];

for (const { manifest, name } of packages) {
  const directory = dirname(manifest);
  if (!importedElsewhere(name, directory)) continue;

  if (!existsSync(join(directory, "dist"))) {
    missing.push(
      `${name} is imported outside ${directory} and has no dist/ after a build — ` +
        "add it to build:packages in vite.config.ts",
    );
  }
}

for (const failure of missing) {
  process.stderr.write(`error: ${failure}\n`);
}

if (missing.length > 0) {
  process.exit(1);
}

process.stdout.write(`packages built: ${packages.length} checked, ${missing.length} missing\n`);
