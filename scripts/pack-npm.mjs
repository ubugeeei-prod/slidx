/**
 * Creates the exact tarballs handed to `npm publish`.
 *
 * npm packs a workspace manifest literally, leaving `workspace:*` in the
 * published package. pnpm resolves those references to this release's version
 * while packing. npm still performs the publish so trusted publishing and its
 * short-lived OIDC credential remain the only registry authentication path.
 */

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [destination, ...directories] = process.argv.slice(2);

  if (!destination || directories.length === 0) {
    process.stderr.write("usage: node scripts/pack-npm.mjs <destination> <package>...\n");
    process.exit(2);
  }

  mkdirSync(destination, { recursive: true });

  for (const directory of directories) {
    process.stdout.write(`${pack(directory, destination)}\n`);
  }
}

export function pack(directory, destination) {
  const output = execFileSync(
    "pnpm",
    ["--dir", directory, "pack", "--pack-destination", destination, "--json"],
    { encoding: "utf8" },
  );
  const report = JSON.parse(output);
  const filename = (Array.isArray(report) ? report[0] : report)?.filename;

  if (typeof filename !== "string" || filename.length === 0) {
    throw new Error(`${directory}: pnpm pack did not report a tarball`);
  }

  const tarball = isAbsolute(filename) ? filename : resolve(filename);
  const manifest = packedManifest(tarball);

  assertPublishableManifest(manifest, directory);
  return tarball;
}

export function assertPublishableManifest(manifest, directory = manifest.name ?? "package") {
  for (const [field, dependencies] of Object.entries(manifest)) {
    if (!/dependencies$/i.test(field) || !dependencies || typeof dependencies !== "object") {
      continue;
    }

    for (const [name, requirement] of Object.entries(dependencies)) {
      if (typeof requirement === "string" && requirement.startsWith("workspace:")) {
        throw new Error(`${directory}: ${field}.${name} still uses ${requirement}`);
      }
    }
  }
}

export function packedManifest(tarball) {
  const archive = gunzipSync(readFileSync(tarball));

  for (let offset = 0; offset + 512 <= archive.length;) {
    const header = archive.subarray(offset, offset + 512);
    const name = text(header.subarray(0, 100));
    const size = Number.parseInt(text(header.subarray(124, 136)) || "0", 8);
    const body = offset + 512;

    if (name === "package/package.json") {
      return JSON.parse(archive.subarray(body, body + size).toString("utf8"));
    }

    offset = body + Math.ceil(size / 512) * 512;
  }

  throw new Error(`${tarball}: package/package.json is missing`);
}

function text(bytes) {
  return bytes.toString("utf8").replace(/\0.*$/s, "").trim();
}
