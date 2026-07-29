/**
 * Assembles the per-platform npm packages a release publishes.
 *
 * Each one holds exactly one prebuilt binary and declares the `os` and `cpu` it
 * runs on. They are `optionalDependencies` of the `slidx` wrapper, so npm
 * installs the single one that matches this machine and silently skips the
 * others — which is what optional dependencies are for, and what makes a
 * postinstall download unnecessary. `packages/cli/bin/slidx.mjs` explains at
 * length why that matters.
 *
 * ## Generated rather than checked in
 *
 * esbuild and biome keep these as directories in their trees. Five near
 * identical `package.json` files is five things to remember to bump, and the
 * version, the platform list and the file layout are all already stated once
 * elsewhere. So the packages are built from `scripts/platforms.mjs` and the
 * Cargo workspace version at release time, and they cannot drift from either
 * because there is nothing to drift.
 *
 *     node scripts/build-platform-packages.mjs <binaries-dir> [<out-dir>]
 *
 * `binaries-dir` holds one directory per Rust target, each containing the
 * binary the release workflow built for it. Every platform in the table has to
 * be there: it names each missing one and then fails, rather than quietly
 * publishing four of five. An empty or absent platform package installs
 * cleanly and fails at the first `slidx`, which is the worst possible time for
 * anybody to find out.
 */

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

import { binaryName, PLATFORMS } from "./platforms.mjs";

const [binariesDir, outDir = "packages/cli/dist"] = process.argv.slice(2);

if (!binariesDir) {
  process.stderr.write("usage: build-platform-packages.mjs <binaries-dir> [<out-dir>]\n");
  process.exit(1);
}

const version = readCargoWorkspaceVersion();
const built = [];

for (const platform of PLATFORMS) {
  const binary = join(binariesDir, platform.target, binaryName(platform));

  if (!existsSync(binary)) {
    // Named individually, so a release that lost one build says which, rather
    // than making somebody diff the registry against the matrix.
    process.stderr.write(`error: no binary at ${binary}, so ${platform.npm} cannot be built\n`);
    continue;
  }

  writePackage(platform, binary);
  built.push(platform.npm);
}

process.stdout.write(
  `built ${built.length} platform package(s) in ${outDir}: ${built.join(", ")}\n`,
);

// A release that publishes the wrapper without the platform package for a
// machine leaves `npm i -g slidx` installing something that cannot run there.
if (built.length !== PLATFORMS.length) {
  process.stderr.write(
    `error: ${PLATFORMS.length - built.length} platform(s) had no binary; ` +
      `publishing the wrapper without them would install a slidx that cannot start\n`,
  );
  process.exit(1);
}

function writePackage(platform, binary) {
  const directory = join(outDir, platform.npm.replace("@slidx/", ""));
  mkdirSync(join(directory, "bin"), { recursive: true });

  const destination = join(directory, "bin", binaryName(platform));
  copyFileSync(binary, destination);

  // Set rather than inherited. npm records the mode in the tarball and restores
  // it on install, so a binary that arrived here through an artifact round-trip
  // without its executable bit would publish unrunnable and fail at `slidx` —
  // with a permission error that names nothing useful.
  chmodSync(destination, 0o755);

  writeFileSync(
    join(directory, "package.json"),
    `${JSON.stringify(manifest(platform), null, 2)}\n`,
  );

  writeFileSync(directory + "/README.md", readme(platform));
  copyFileSync("LICENSE", join(directory, "LICENSE"));
}

/**
 * One platform package's manifest.
 *
 * No `bin` field: this package is a container, and declaring a bin would put a
 * second `slidx` on the PATH that shadows the wrapper on whichever platform
 * happened to match. The wrapper resolves the file directly.
 *
 * No `exports` either, so `require.resolve("@slidx/cli-linux-x64/bin/slidx")`
 * reaches the file. An `exports` map would have to list it, which is one more
 * place the layout is written down.
 */
function manifest(platform) {
  return {
    name: platform.npm,
    version,
    description: `The slidx binary for ${platform.describes}`,
    license: "MIT",
    repository: {
      type: "git",
      url: "git+https://github.com/ubugeeei-prod/slidx.git",
      directory: "packages/cli",
    },
    os: [platform.os],
    cpu: [platform.cpu],
    files: ["bin", "LICENSE", "README.md"],
    publishConfig: { access: "public", provenance: true },
  };
}

function readme(platform) {
  return `# ${platform.npm}

The \`slidx\` binary for ${platform.describes}, built from
[ubugeeei-prod/slidx](https://github.com/ubugeeei-prod/slidx) at v${version}.

Nothing installs this package directly. It is an optional dependency of
[\`slidx\`](https://www.npmjs.com/package/slidx), which resolves whichever one
matches the machine it was installed on:

\`\`\`bash
npm i -g slidx
\`\`\`

The binary is in the published tarball rather than downloaded by an install
script, so \`npm ci --offline\` works, the artifact is in your lockfile with an
integrity hash, and npm's provenance attestation covers the executable itself.
`;
}

function readCargoWorkspaceVersion() {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const section = manifest.slice(manifest.indexOf("[workspace.package]"));
  const match = /^version\s*=\s*"([^"]+)"/m.exec(section);

  if (!match) throw new Error("no version in [workspace.package]");
  return match[1];
}
