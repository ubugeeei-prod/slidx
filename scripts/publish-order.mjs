/**
 * What to publish, and in what order.
 *
 * Both registries refuse a package whose dependencies they have not seen yet,
 * so a release is a sequence rather than a set. That sequence used to be typed
 * into `release.yml` by hand, and it was wrong: `slidx_cli` had gained
 * `slidx_highlight` and `slidx_publish` as dependencies and neither was in the
 * list, so a tag push would have published five crates and then failed on the
 * sixth — with the five already permanent on crates.io. `@ubugeeei/slidx-vite-plugin`,
 * which is the package the README tells people to install, was not in the npm
 * list at all.
 *
 * So the list is derived rather than maintained. Adding a crate or a package
 * changes nothing here and nothing in the workflow; the order falls out of the
 * manifests that already declare the dependencies.
 *
 * Usage: `node scripts/publish-order.mjs crates|npm`
 */

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * Kahn's algorithm, with ties broken by name.
 *
 * Deterministic order matters beyond tidiness: a release that fails halfway
 * has to be resumable by hand, and that is only possible if "what had already
 * gone" is the same list every time.
 */
function topological(nodes) {
  const remaining = new Map(nodes.map((node) => [node.name, new Set(node.deps)]));
  const order = [];

  while (remaining.size > 0) {
    const ready = [...remaining]
      .filter(([, deps]) => [...deps].every((dep) => !remaining.has(dep)))
      .map(([name]) => name)
      .sort();

    if (ready.length === 0) {
      throw new Error(`a dependency cycle involves: ${[...remaining.keys()].sort().join(", ")}`);
    }

    for (const name of ready) {
      order.push(name);
      remaining.delete(name);
    }
  }

  return order;
}

/** Every workspace crate that is not marked `publish = false`. */
function crates() {
  const root = "crates";
  const names = readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);

  const nodes = names
    .map((directory) => {
      const manifest = readFileSync(join(root, directory, "Cargo.toml"), "utf8");
      const name = /^name\s*=\s*"([^"]+)"/m.exec(manifest)?.[1];
      if (name === undefined || /^publish\s*=\s*false/m.test(manifest)) return undefined;

      // Workspace crates are declared one per line as `slidx_x = { … }`, which
      // is the only place a slidx name appears at the start of a line.
      const deps = [...manifest.matchAll(/^(slidx_[a-z_]+)\s*=/gm)].map((match) => match[1]);

      return { name, deps };
    })
    .filter((node) => node !== undefined);

  const known = new Set(nodes.map((node) => node.name));

  return topological(
    nodes.map((node) => ({ ...node, deps: node.deps.filter((d) => known.has(d)) })),
  );
}

/**
 * The `slidx` wrapper, which cannot be ordered from a manifest.
 *
 * Its dependencies are the five `@ubugeeei/slidx-cli-*` platform packages, and those do
 * not exist until `build-platform-packages.mjs` writes them at release time.
 * Read from here it looks like it depends on nothing and would sort first —
 * which is the one position it must never take, because a wrapper published
 * before its binaries installs without one and npm calls that a success.
 *
 * So the workflow places it last, after the generated packages, by hand.
 */
const WRAPPER = "cli";

/**
 * Every publishable npm package, as a directory.
 *
 * Directories rather than names because that is what the workflow has to `cd`
 * into. The per-platform binary packages are generated at release time and are
 * not here — they have no manifest to read until they exist.
 */
function npm() {
  const root = "packages";
  const nodes = readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name !== WRAPPER)
    .map((entry) => {
      const directory = join(root, entry.name);
      const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
      if (manifest.private === true) return undefined;

      const deps = Object.keys({ ...manifest.dependencies, ...manifest.optionalDependencies });

      return { name: manifest.name, directory, deps };
    })
    .filter((node) => node !== undefined);

  const byName = new Map(nodes.map((node) => [node.name, node]));
  const order = topological(
    nodes.map((node) => ({ ...node, deps: node.deps.filter((dep) => byName.has(dep)) })),
  );

  return order.map((name) => byName.get(name).directory);
}

const [, , which] = process.argv;

if (which === "crates") {
  process.stdout.write(`${crates().join("\n")}\n`);
} else if (which === "npm") {
  process.stdout.write(`${npm().join("\n")}\n`);
} else {
  process.stderr.write("usage: node scripts/publish-order.mjs crates|npm\n");
  process.exit(2);
}
