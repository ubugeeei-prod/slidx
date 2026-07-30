#!/usr/bin/env node
/**
 * Runs the prebuilt `slidx` that npm installed for this machine.
 *
 * # Why there is no postinstall script
 *
 * The obvious way to ship a binary on npm is a `postinstall` that downloads
 * one. It is also the wrong way, and not marginally:
 *
 * - **It breaks offline installs.** A populated npm cache is supposed to be
 *   enough. A postinstall that reaches the network makes `npm ci` on a plane,
 *   or in an air-gapped build, fail on a package that is already there.
 * - **It breaks caching.** Every CI run downloads the binary again, because
 *   the thing the cache holds is the tarball and the binary is not in it.
 * - **It breaks behind a corporate proxy.** npm's proxy settings are npm's;
 *   a script doing its own HTTP does not inherit them, and the failure is a
 *   timeout at install time with no useful message.
 * - **It is a supply-chain smell.** A package that fetches an executable at
 *   install time and runs it cannot be audited from its published contents.
 *   Whatever the intent, that is indistinguishable from the shape of an
 *   attack, and people are right to treat it as one.
 *
 * So the binary is *in* a package instead. One package per platform, each
 * declaring `os` and `cpu`, all of them `optionalDependencies` of this one.
 * npm installs exactly the one that matches and silently skips the rest, which
 * is the behaviour optional dependencies were added for. This is the pattern
 * esbuild, swc and biome all converged on, and they converged on it because
 * every other option has one of the failures above.
 *
 * What that buys, concretely: `npm ci --offline` works, the artifact is in the
 * lockfile with an integrity hash, and `npm publish --provenance` attests the
 * binary itself rather than a script that will later fetch one.
 *
 * # Why this file has no imports
 *
 * It is the first thing that runs after npm finishes, and its only job is to
 * start another process. A build step between the tarball and this file is
 * another thing that can be missing when somebody's install goes wrong, so
 * there is not one: what is published is what is read.
 */

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const require = createRequire(import.meta.url);

/**
 * The platform package for the machine this is running on.
 *
 * Derived rather than looked up in a table: the name *is* the platform, so a
 * table here could only ever disagree with the one in package.json.
 */
export function platformPackage(platform = process.platform, arch = process.arch) {
  return `@ubugeeei/slidx-cli-${platform}-${arch}`;
}

/**
 * Every platform the wrapper declares a package for, read from its own
 * manifest — so the error message and the dependency list cannot disagree.
 *
 * `import.meta.dirname` rather than `new URL(..., import.meta.url)`: under a
 * module runner `import.meta.url` is not a file URL, and this file is imported
 * by its own tests.
 */
export function publishedPlatforms() {
  const manifest = join(import.meta.dirname, "..", "package.json");
  const { optionalDependencies } = JSON.parse(readFileSync(manifest, "utf8"));

  return Object.keys(optionalDependencies ?? {});
}

/**
 * Where the binary is, or `undefined` if npm did not install it.
 *
 * Resolved through `require.resolve` rather than by joining paths, because npm,
 * pnpm and yarn each lay `node_modules` out differently and only the resolver
 * knows which.
 */
export function findBinary(name = platformPackage()) {
  const file = process.platform === "win32" ? "slidx.exe" : "slidx";

  try {
    const path = require.resolve(`${name}/bin/${file}`);
    return existsSync(path) ? path : undefined;
  } catch {
    return undefined;
  }
}

/**
 * What to say when there is no binary to run.
 *
 * Three different situations end up here and they have different fixes, so the
 * message names which one it is rather than printing one generic sentence:
 * a platform slidx does not publish, an install that skipped optional
 * dependencies, or a package that is present but empty.
 */
export function missingBinaryMessage(name = platformPackage(), published = publishedPlatforms()) {
  const head = `slidx: no binary for ${process.platform}-${process.arch}.\n`;

  if (!published.includes(name)) {
    return (
      `${head}\n` +
      `slidx publishes prebuilt binaries for:\n\n` +
      published.map((entry) => `  ${entry.replace("@ubugeeei/slidx-cli-", "")}`).join("\n") +
      `\n\nOn anything else, build it from source:\n\n` +
      `  cargo install slidx_cli\n`
    );
  }

  return (
    `${head}\n` +
    `${name} should have been installed alongside slidx and was not.\n\n` +
    `The usual causes:\n\n` +
    `  - the install ran with --no-optional or --omit=optional\n` +
    `  - a lockfile was copied from a machine of a different platform\n` +
    `  - the package manager was interrupted partway through\n\n` +
    `Reinstalling usually fixes it:\n\n` +
    `  npm i -g slidx\n\n` +
    `Failing that, the shell installer does not go through npm at all:\n\n` +
    `  curl -fsSL https://raw.githubusercontent.com/ubugeeei-prod/slidx/main/install.sh | sh\n`
  );
}

function main() {
  const binary = findBinary();

  if (!binary) {
    process.stderr.write(missingBinaryMessage());
    process.exit(2);
  }

  // stdio is inherited, so the child owns the terminal: `slidx doctor` decides
  // about colour by looking at the real stream rather than at a pipe from here,
  // and `slidx lint | head` closes the pipe on the process that is writing.
  const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

  if (result.error) {
    process.stderr.write(`slidx: could not run ${binary}: ${result.error.message}\n`);
    process.exit(2);
  }

  // A child killed by a signal has a null status. Reporting that as 0 would
  // make a Ctrl-C during `slidx lint` look like a deck with nothing wrong.
  process.exit(result.status ?? 1);
}

/**
 * True when this file is the program rather than an import.
 *
 * Both sides are resolved through the filesystem before they are compared.
 * Node reports the entry point as its real path while `process.argv[1]` keeps
 * whatever symlinks were used to reach it — and a `node_modules` full of
 * symlinks is not an edge case, it is how pnpm installs everything. Comparing
 * the strings makes this file do nothing at all, silently, on those machines.
 */
function isProgram() {
  if (!process.argv[1]) return false;

  try {
    return realpathSync(process.argv[1]) === realpathSync(import.meta.filename);
  } catch {
    return false;
  }
}

if (isProgram()) {
  main();
}
