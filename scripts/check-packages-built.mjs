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
 * The list is derived, and the rule is about **publishing** rather than about
 * this repository. It was "a package TypeScript outside its own directory
 * imports", which is the invariant that keeps a check honest and the wrong one
 * for a registry: `@slidx/audience`, `@slidx/islands`, `@slidx/rehearsal` and
 * `@slidx/publish` are imported by nobody here, were built by nothing, and a
 * publish dry run found all four would ship a tarball holding one
 * `package.json` — a permanently broken version, fixable only by publishing
 * another.
 *
 * So a package counts when it is publishable and says its contents live in
 * `dist/`. Being imported here is no longer part of it: the people that rule
 * protects are the ones who install these, not us.
 */

import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";

function tracked(...paths) {
  const output = execFileSync("git", ["ls-files", "-z", ...paths], { encoding: "utf8" });
  return output.split("\0").filter(Boolean);
}

/**
 * Every package that would be published, and that says its contents are in
 * `dist/` — through `files`, which decides what goes in the tarball, or through
 * `exports`, which decides what resolves once it is installed.
 */
const packages = tracked("packages")
  .filter((file) => /^packages\/[^/]+\/package\.json$/.test(file))
  .map((file) => ({ manifest: file, ...JSON.parse(readFileSync(file, "utf8")) }))
  .filter(({ private: hidden }) => hidden !== true)
  .filter(({ files, exports }) =>
    [JSON.stringify(files ?? []), JSON.stringify(exports ?? {})].some((declared) =>
      declared.includes("dist"),
    ),
  );

const missing = [];

for (const { manifest, name } of packages) {
  const directory = dirname(manifest);

  if (!existsSync(join(directory, "dist"))) {
    missing.push(
      `${name} would be published from ${directory} and has no dist/ after a build — ` +
        "add it to build:packages in vite.config.ts, or it ships as one package.json",
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
