/**
 * Opt-in MDX at the Vite boundary.
 *
 * Rust owns the syntax and the static island contract; these tests own the two
 * things only the plugin knows: `.mdx` file discovery and keeping that file's
 * bytes as the visual editor's source of truth.
 */

import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build, createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src";

interface Fixture {
  root: string;
  server: ViteDevServer;
  url: string;
  production: string;
}

const COMPONENT = '<Counter start={128} label="people">\n\n**128 people**\n\n</Counter>';

let fixture: Fixture;

beforeAll(async () => {
  const root = await mkdtemp(join(tmpdir(), "slidx-mdx-"));
  await mkdir(join(root, "slides"), { recursive: true });
  await writeFile(join(root, "slides", "0001.mdx"), `# Sign-ups\n\n${COMPONENT}\n`);
  await writeFile(
    join(root, "islands.mjs"),
    `export default {
  register() {},
  lookup() { return { name: "Counter", async mount() { return { unmount() {} }; } }; },
  has() { return true; },
  names() { return ["Counter"]; },
};\n`,
  );

  await build({
    root,
    logLevel: "silent",
    plugins: [
      slidx({
        mdx: true,
        islands: "./islands.mjs",
        presenter: false,
        print: false,
        og: false,
        overflow: false,
      }),
    ],
    build: { outDir: join(root, "dist"), minify: false },
  });

  const production = await readFile(join(root, "dist", "slides", "index.html"), "utf8");
  const server = await createServer({
    root,
    logLevel: "silent",
    plugins: [
      slidx({
        mdx: true,
        islands: "./islands.mjs",
        presenter: false,
        print: false,
        og: false,
        overflow: false,
      }),
    ],
    server: { host: "127.0.0.1", port: 0, watch: null, hmr: false },
  });
  await server.listen();

  const url = server.resolvedUrls?.local[0];
  if (!url) throw new Error("the MDX dev server has no local URL");
  fixture = { root, server, url, production };
});

afterAll(async () => {
  await fixture.server.close();
});

describe("opt-in MDX", () => {
  it("discovers an mdx slide and emits the static island contract", () => {
    expect(fixture.production).toContain('data-slidx-island="Counter"');
    expect(fixture.production).toContain(
      'data-slidx-island-props="{&quot;label&quot;:&quot;people&quot;,&quot;start&quot;:128}"',
    );
    expect(fixture.production).toContain("<strong>128 people</strong>");
    expect(fixture.production).toMatch(/<script type="module" src="[^"]+"><\/script>/);
  });

  it("renders the same contract in dev", async () => {
    const response = await fetch(`${fixture.url}slides/`);
    const html = await response.text();

    expect(response.status).toBe(200);
    expect(html).toContain('data-slidx-island="Counter"');
    expect(html).toContain("<strong>128 people</strong>");
  });

  it("edits Markdown without serialising or rewriting MDX", async () => {
    const response = await fetch(`${fixture.url}__slidx/edit`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ op: { op: "setHeading", slide: 0, text: "Registrations" } }),
    });
    const payload = (await response.json()) as { error?: unknown };
    const source = await readFile(join(fixture.root, "slides", "0001.mdx"), "utf8");

    expect(response.status, JSON.stringify(payload)).toBe(200);
    expect(payload.error).toBeUndefined();
    expect(source).toBe(`# Registrations\n\n${COMPONENT}\n`);
  });

  it("blocks expressions that would require executing deck source", async () => {
    const root = await mkdtemp(join(tmpdir(), "slidx-mdx-dynamic-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(
      join(root, "slides", "0001.mdx"),
      "# Unsafe\n\n<Counter value={window.secret}>static fallback</Counter>\n",
    );

    await expect(
      build({
        root,
        logLevel: "silent",
        plugins: [
          slidx({
            mdx: true,
            presenter: false,
            print: false,
            og: false,
            overflow: false,
          }),
        ],
        build: { outDir: join(root, "dist") },
      }),
    ).rejects.toThrow();
  });
});
