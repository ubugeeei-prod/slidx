/**
 * Opting a deck into framework islands at the Vite boundary.
 *
 * The adapter package already owns mounting and teardown. These tests own the
 * seam nobody else can see: a setup module becoming one bundled client, that
 * client reaching only pages with an island, and the same route working in
 * dev. The ordinary zero-JavaScript deck remains pinned in `build.test.ts`.
 */

import { mkdtemp, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build, createServer, preview, type PreviewServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src";
import { islandClientSource, withIslandClient } from "../src/islands";

interface Fixture {
  root: string;
  files: string[];
  client: string;
  server: ViteDevServer;
  url: string;
  preview: PreviewServer;
  previewUrl: string;
}

let fixture: Fixture;

async function chromiumLaunchable(): Promise<boolean> {
  try {
    const { chromium } = await import("playwright");
    const browser = await chromium.launch();
    await browser.close();
    return true;
  } catch {
    return false;
  }
}

const hasChromium = await chromiumLaunchable();

beforeAll(async () => {
  const root = await mkdtemp(join(tmpdir(), "slidx-islands-"));
  await mkdir(join(root, "slides"), { recursive: true });
  await writeFile(
    join(root, "slides", "0001.md"),
    [
      "# Interactive",
      "",
      `<div data-slidx-island="counter" data-slidx-island-props='{"value":7}'>Static 7</div>`,
      "",
    ].join("\n"),
  );
  await writeFile(join(root, "slides", "0002.md"), "# Plain\n\nNothing to hydrate.\n");
  await writeFile(
    join(root, "islands.mjs"),
    `
const definition = {
  name: "counter",
  async mount(target, props) {
    target.textContent = \`Mounted \${props.value}\`;
    return { unmount() {} };
  },
};

export default {
  register() {},
  lookup(name) { return name === definition.name ? definition : undefined; },
  has(name) { return name === definition.name; },
  names() { return [definition.name]; },
};
`,
  );

  await build({
    root,
    logLevel: "silent",
    plugins: [slidx({ islands: "./islands.mjs", og: false, overflow: false })],
    build: { outDir: join(root, "dist"), minify: false },
  });

  const files = await walk(join(root, "dist"));
  const client = files.find((file) => file.startsWith("assets/") && file.endsWith(".js"));
  if (!client) throw new Error("the islands build emitted no client entry");

  const server = await createServer({
    root,
    logLevel: "silent",
    plugins: [slidx({ islands: "./islands.mjs", og: false, overflow: false })],
    // The fixture reads routes and never edits a file. Opening a filesystem
    // watcher would add no coverage, and on Windows its native handle can
    // outlive `server.close()` and abort the Vitest worker after every
    // assertion has passed.
    server: { host: "127.0.0.1", port: 0, watch: null, hmr: false },
  });
  await server.listen();

  const url = server.resolvedUrls?.local[0];
  if (!url) throw new Error("the islands dev server has no local URL");

  const production = await preview({
    root,
    logLevel: "silent",
    build: { outDir: join(root, "dist") },
    preview: { host: "127.0.0.1", port: 0 },
  });
  const previewUrl = production.resolvedUrls?.local[0];
  if (!previewUrl) throw new Error("the islands preview server has no local URL");

  fixture = {
    root,
    files,
    client,
    server,
    url,
    preview: production,
    previewUrl,
  };
}, 60_000);

afterAll(async () => {
  await fixture?.server.close();
  if (fixture?.preview) {
    await new Promise<void>((resolve) => fixture.preview.httpServer.close(() => resolve()));
  }
});

describe("the page boundary", () => {
  it("leaves a page with no island byte-for-byte alone", () => {
    const html = "<html><body><p>Plain</p></body></html>";
    expect(withIslandClient(html, "/client.js")).toBe(html);
  });

  it("adds one module client before the body closes", () => {
    const html = '<html><body><div data-slidx-island="chart">fallback</div></body></html>';
    const page = withIslandClient(html, "/client.js");

    expect(page.match(/<script type="module"/g)).toHaveLength(1);
    expect(page).toContain('<script type="module" src="/client.js"></script>\n</body>');
  });

  it("addresses one client correctly from every deck depth", () => {
    expect(islandClientSource("slides/index.html", "assets/islands.js")).toBe(
      "../assets/islands.js",
    );
    expect(islandClientSource("slides/2/index.html", "assets/islands.js")).toBe(
      "../../assets/islands.js",
    );
    expect(islandClientSource("slides/presenter/index.html", "assets/islands.js")).toBe(
      "../../assets/islands.js",
    );
  });
});

describe("a production build", () => {
  it("bundles the opted-in setup as one client entry", async () => {
    const clients = fixture.files.filter(
      (file) => file.startsWith("assets/") && file.endsWith(".js"),
    );

    expect(clients).toEqual([fixture.client]);
    expect(await readFile(join(fixture.root, "dist", fixture.client), "utf8")).toContain("Mounted");
  });

  it("hydrates the island page and keeps its static fallback", async () => {
    const page = await built("slides/index.html");
    const source = islandClientSource("slides/index.html", fixture.client);

    expect(page).toContain("Static 7");
    expect(page).toContain(`<script type="module" src="${source}"></script>`);
  });

  it("puts no client on a slide that asks for none", async () => {
    const page = await built("slides/2/index.html");

    expect(page).not.toContain(fixture.client);
    expect(page).not.toContain('<script type="module"');
  });

  it("does not start an island inside a presenter's static next-slide preview", async () => {
    const page = await built("slides/presenter/index.html");

    expect(page).not.toContain(fixture.client);
  });

  it("keeps the printable fallback independent of the client bundle", async () => {
    const page = await built("slides/print/index.html");

    expect(page).toContain("Static 7");
    expect(page).not.toContain(fixture.client);
  });
});

describe("the dev server", () => {
  it("serves the same setup entry from the deck server", async () => {
    const page = await fetch(new URL("slides/", fixture.url)).then((response) => response.text());
    const source = page.match(/<script type="module" src="([^"]*islands[^"]*)"/)?.[1];

    expect(source).toBeDefined();

    const client = await fetch(new URL(source!, fixture.url));
    expect(client.ok).toBe(true);
    expect(await client.text()).toContain("hydrateIslands");
  });

  it("keeps a plain dev slide free of the island client", async () => {
    const page = await fetch(new URL("slides/2/", fixture.url)).then((response) => response.text());

    expect(page).not.toContain("islands");
    expect(page).not.toContain('src="/__slidx/islands.js"');
  });

  it.skipIf(!hasChromium)(
    "mounts the registered island in dev and production",
    async () => {
      const { chromium } = await import("playwright");
      const browser = await chromium.launch();

      try {
        const tab = await browser.newPage();
        const errors: string[] = [];
        tab.on("console", (message) => {
          if (message.type() === "error") errors.push(message.text());
        });
        tab.on("pageerror", (error) => errors.push(error.message));

        for (const [surface, base] of [
          ["dev", fixture.url],
          ["production", fixture.previewUrl],
        ] as const) {
          errors.length = 0;
          await tab.goto(new URL("slides/", base).href);

          await expect
            .poll(() => tab.locator('[data-slidx-island="counter"]').textContent(), {
              // On a saturated Windows matrix the module fetch can begin after
              // Vitest's one-second polling default. This is a real browser
              // boundary, so wait for the observable mount instead of racing
              // the runner's process scheduler.
              timeout: 10_000,
              interval: 100,
            })
            .toBe("Mounted 7");
          expect(errors, `${surface} island client errors`).toEqual([]);
        }
      } finally {
        await browser.close();
      }
    },
    // This launches a real browser and visits both a dev and preview server.
    // A fresh Windows VM can spend the default five seconds launching Chromium
    // before either hydration assertion has had a chance to run.
    20_000,
  );
});

async function built(file: string): Promise<string> {
  return readFile(join(fixture.root, "dist", file), "utf8");
}

async function walk(directory: string, prefix = ""): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const found: string[] = [];

  for (const entry of entries) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...(await walk(join(directory, entry.name), path)));
    else found.push(path);
  }

  return found.sort();
}
