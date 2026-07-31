/**
 * Which directories become a tarball, and therefore need the licence in them.
 *
 * The MIT licence's one condition is that its notice travels with the copies —
 * "shall be included in all copies or substantial portions of the Software".
 * `license = "MIT"` in a manifest is metadata about that, not the notice: it
 * puts a word on a registry page and puts nothing in the archive somebody
 * downloads.
 *
 * Neither packager will find the file on its own. Cargo includes `LICENSE*`
 * only from the package directory, and npm includes it only from the package
 * directory — and this is a workspace, so the one at the root is in neither.
 * A publish dry run across every artefact found exactly that: 27 of them
 * carried the word and none of them carried the text.
 *
 * So a copy lives beside each manifest, and this decides where "each" is by
 * asking the same two questions the packagers ask rather than by holding a
 * list. A crate added to `crates/` or a package added to `packages/` is covered
 * the day it exists. The one directory whose copy is written by its build
 * rather than committed is recognised the same derived way, by asking git which
 * of the paths it has been told to ignore.
 *
 * # Why byte-identical rather than merely present
 *
 * Because the failure this is guarding against is not a missing file. It is
 * twenty-eight copies of one paragraph drifting — a year renumbered in one of
 * them, a name changed in another — and copies of a legal notice that disagree
 * are worse than one that is only in the repository root.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname } from "node:path";

/** The name both packagers look for, and the only one this writes. */
export const LICENCE_FILE = "LICENSE";

function tracked(...paths) {
  return execFileSync("git", ["ls-files", "-z", ...paths], { encoding: "utf8" })
    .split("\0")
    .filter(Boolean);
}

/**
 * Every crate cargo would publish.
 *
 * `publish = false` is the only way a crate opts out, and `slidx_docs` uses it:
 * it builds this project's own documentation site and has no reader outside
 * this repository.
 */
export function publishedCrates(read = readFileSync) {
  return tracked("crates")
    .filter((file) => /^crates\/[^/]+\/Cargo\.toml$/.test(file))
    .filter((file) => !/^\s*publish\s*=\s*false/m.test(read(file, "utf8")))
    .map(dirname);
}

/**
 * Every package npm would publish.
 *
 * The same rule `check-version.mjs` and `check-packages-built.mjs` use, and
 * for the same reason: `private: true` is what a manifest says when it is not
 * for anybody else. `packages/vscode` says it, because a marketplace extension
 * is not an npm package.
 */
export function publishedPackages(read = readFileSync) {
  return tracked("packages")
    .filter((file) => /^packages\/[^/]+\/package\.json$/.test(file))
    .filter((file) => JSON.parse(read(file, "utf8")).private !== true)
    .map(dirname);
}

/**
 * Which of those directories has to carry a committed copy.
 *
 * `packages/wasm` is generated: `scripts/build-wasm.mjs` emits the dist, writes
 * the README, and copies this repository's `LICENSE` in beside them, so the
 * notice is in the tarball without ever being in the tree. `.gitignore` already
 * says which of those paths are written rather than kept, so asking git keeps
 * that in one place instead of starting a second list here.
 *
 * `isIgnored` is injected for the same reason `read` is below: so the rule can
 * be exercised without a repository that happens to ignore the right paths.
 */
export function needsCommittedLicence(directories, isIgnored = ignored) {
  return needsCommitted(directories, LICENCE_FILE, isIgnored);
}

/**
 * The same question about any file a tarball is judged on.
 *
 * The licence is one of two: `check-pages.mjs` asks it about `README.md`, which
 * `packages/wasm` also has written into it by its build. One rule, so the two
 * cannot come to different conclusions about the same directory.
 */
export function needsCommitted(directories, file, isIgnored = ignored) {
  const generated = isIgnored(directories.map((directory) => `${directory}/${file}`));

  return directories.filter((directory) => !generated.has(`${directory}/${file}`));
}

/** The subset of `paths` git has been told to ignore. */
function ignored(paths) {
  if (paths.length === 0) return new Set();

  try {
    const matched = execFileSync("git", ["check-ignore", "-z", "--stdin"], {
      input: paths.join("\0"),
      encoding: "utf8",
    });

    return new Set(matched.split("\0").filter(Boolean));
  } catch (error) {
    // git exits 1 when it ignores none of them, which is not an error here.
    if (error.status === 1) return new Set();
    throw error;
  }
}

/**
 * The directories that ship without the notice, or with a different one.
 *
 * `read` is injected so the classification can be tested against a tree that
 * does not exist, which is the only way to test the case this exists to catch:
 * a copy that has drifted.
 */
export function unlicensed(directories, licence, read = readFileSync) {
  return directories.flatMap((directory) => {
    let found;

    try {
      found = read(`${directory}/${LICENCE_FILE}`, "utf8");
    } catch {
      return [{ directory, problem: "missing" }];
    }

    return found === licence ? [] : [{ directory, problem: "differs" }];
  });
}
