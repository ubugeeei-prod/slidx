/**
 * The promise that a deck which uses no framework carries none.
 *
 * Every other test in this package asserts what an adapter does once a slide
 * has asked for it. This one asserts what is true when no slide asks for
 * anything, which is the property the whole project rests on: a deck that never
 * writes `angular` must not install Angular, must not resolve Angular, and must
 * not run Angular's compiler.
 *
 * None of that is visible from inside a module, so it is checked where it
 * actually lives — the manifest and the shape of the imports. Both are easy to
 * break with a change that looks harmless: one static `import … from
 * "@angular/core"` makes every deck's bundler resolve Angular, and a peer
 * dependency that is not marked optional makes every deck's package manager
 * install it. Neither shows up in a unit test of an adapter, and neither is a
 * type error.
 */

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

// Paths rather than `new URL(…, import.meta.url)`: Vite rewrites that pattern
// into an asset reference, and the rewritten URL is no longer a file one.
const packageRoot = join(import.meta.dirname, "..");
const adaptersDirectory = join(packageRoot, "src", "adapters");

interface Manifest {
  dependencies?: Record<string, string>;
  peerDependencies?: Record<string, string>;
  peerDependenciesMeta?: Record<string, { optional?: boolean }>;
  exports: Record<string, unknown>;
  scripts: Record<string, string>;
}

const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8")) as Manifest;

/**
 * `peer.ts` is the only file here that is not a framework: it holds the
 * variable-specifier import every adapter loads through, and nothing outside
 * this package should be able to reach it.
 */
const adapters = readdirSync(adaptersDirectory)
  .filter((file) => file.endsWith(".ts") && file !== "peer.ts")
  .map((file) => file.replace(/\.ts$/, ""))
  .sort();

function sourceOf(adapter: string): string {
  return readFileSync(join(adaptersDirectory, `${adapter}.ts`), "utf8");
}

/** Specifiers a deck's bundler has to resolve in order to build this file. */
function staticImports(source: string): string[] {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
  const found: string[] = [];

  for (const match of code.matchAll(/\bfrom\s+"([^"]+)"/g)) found.push(match[1]!);
  for (const match of code.matchAll(/^\s*import\s+"([^"]+)"/gm)) found.push(match[1]!);

  return found;
}

describe("the frameworks a deck opts into", () => {
  it("finds every adapter on disk rather than from a list", () => {
    // Everything below loops over this. A mis-filtered or empty directory read
    // would leave each of those loops passing while asserting nothing, so the
    // set itself is pinned.
    expect(adapters).toEqual(["angular", "react", "svelte", "three", "vue"]);
  });

  it("gives each adapter its own entry point, so importing one loads only one", () => {
    for (const adapter of adapters) {
      expect(manifest.exports[`./${adapter}`]).toBeDefined();
    }
  });

  it("publishes a file behind every entry point it advertises", () => {
    // A subpath in `exports` whose module was never built resolves to nothing,
    // and a deck finds that out at install time rather than here.
    for (const adapter of adapters) {
      expect(manifest.scripts["pack:lib"]).toContain(`src/adapters/${adapter}.ts`);
    }
  });

  it("marks every framework optional, so installing this package installs none of them", () => {
    for (const peer of Object.keys(manifest.peerDependencies ?? {})) {
      expect(manifest.peerDependenciesMeta?.[peer]?.optional).toBe(true);
    }
  });

  it("depends on nothing at all, so installing it installs one package", () => {
    expect(manifest.dependencies).toBeUndefined();
  });
});

describe("what a deck's bundler is asked to resolve", () => {
  it("imports nothing from outside this package", () => {
    // The single change that would break the whole promise: a static import of
    // a framework makes every deck resolve it at build time, including the
    // decks that deliberately do not have it installed.
    for (const adapter of adapters) {
      for (const specifier of staticImports(sourceOf(adapter))) {
        expect(specifier.startsWith(".")).toBe(true);
      }
    }
  });

  it("never writes a framework's name where an import can follow it", () => {
    // `import("@angular/core")` would pass the check above and still be
    // pre-bundled by Vite, which analyses literal dynamic imports the same way
    // it analyses static ones. Framework specifiers reach `import` through a
    // variable or they do not reach it at all.
    for (const adapter of adapters) {
      expect(sourceOf(adapter)).not.toMatch(/\bimport\s*\(\s*["'`]/);
    }
  });
});
