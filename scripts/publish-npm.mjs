/**
 * Publishes packed npm artifacts without repeating an irreversible upload.
 *
 * A release can stop after any tarball: authentication, the registry, and the
 * network are outside this process. Re-running the same command checks the
 * registry first and continues at the first version that is not there yet.
 *
 * ```sh
 * node scripts/publish-npm.mjs --list publish-order.txt
 * node scripts/publish-npm.mjs --provenance package-a.tgz package-b.tgz
 * node scripts/publish-npm.mjs --dry-run --list publish-order.txt
 * ```
 */

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { dirname, isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readFileSync } from "node:fs";

import { packedManifest } from "./pack-npm.mjs";

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = argumentsOf(process.argv.slice(2));

  if (options.tarballs.length === 0) {
    process.stderr.write(
      "usage: node scripts/publish-npm.mjs [--dry-run] [--provenance] " +
        "[--list <file>] <tarball>...\n",
    );
    process.exit(2);
  }

  await publish(options);
}

export function argumentsOf(args) {
  const options = { dryRun: false, provenance: false, tarballs: [] };

  for (let at = 0; at < args.length; at += 1) {
    const argument = args[at];

    if (argument === "--dry-run") {
      options.dryRun = true;
    } else if (argument === "--provenance") {
      options.provenance = true;
    } else if (argument === "--list") {
      const list = args[at + 1];
      if (!list) throw new Error("--list needs a file");
      options.tarballs.push(...tarballsIn(list));
      at += 1;
    } else if (argument.startsWith("--")) {
      throw new Error(`${argument} is not an option`);
    } else {
      options.tarballs.push(resolve(argument));
    }
  }

  return options;
}

function tarballsIn(list) {
  const directory = dirname(resolve(list));

  return readFileSync(list, "utf8")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((tarball) => (isAbsolute(tarball) ? tarball : resolve(directory, tarball)));
}

export async function publish(options, request = fetch, run = spawnSync) {
  for (const tarball of options.tarballs) {
    const manifest = packedManifest(tarball);
    const { name, version } = manifest;

    if (typeof name !== "string" || typeof version !== "string") {
      throw new Error(`${tarball}: package name and version are required`);
    }

    const existing = await registryVersion(name, version, request);
    if (existing) {
      const expected = tarballIntegrity(tarball);
      const actual = existing.dist?.integrity;
      if (actual !== expected) {
        throw new Error(`${name} ${version} exists with a different tarball`);
      }

      process.stdout.write(`  ${name} ${version} is already there\n`);
      continue;
    }

    if (options.dryRun) {
      process.stdout.write(`  ${name} ${version} would be published\n`);
      continue;
    }

    const args = ["publish", tarball, "--access", "public"];
    if (options.provenance) args.push("--provenance");

    process.stdout.write(`\n$ npm ${args.join(" ")}\n`);
    const result = run("npm", args, { encoding: "utf8" });
    process.stdout.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");

    if (result.status !== 0) {
      process.stderr.write(`\nerror: npm refused ${name} ${version}\n`);
      process.exitCode = 1;
      return;
    }
  }

  process.stdout.write(`\nnpm: ${options.tarballs.length} tarball(s) considered\n`);
}

export async function registryVersion(name, version, request = fetch) {
  const response = await request(`https://registry.npmjs.org/${encodeURIComponent(name)}`, {
    headers: { "user-agent": "slidx release (https://github.com/ubugeeei-prod/slidx)" },
  });

  if (response.status === 404) return undefined;
  if (!response.ok) throw new Error(`npm answered ${response.status} about ${name}`);

  const body = await response.json();
  return body.versions?.[version];
}

export function tarballIntegrity(tarball) {
  return `sha512-${createHash("sha512").update(readFileSync(tarball)).digest("base64")}`;
}
