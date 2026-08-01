import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Readable } from "node:stream";

import { build, createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";
import { uploadMedia } from "../src/media-upload";
import { resolveOptions } from "../src/options";
import { CREDENTIAL_HEADER, Grant } from "../src/share";
import { createEditSession } from "../src/session";

async function chromiumAvailable(): Promise<boolean> {
  try {
    const { chromium } = await import("playwright");
    const browser = await chromium.launch();
    await browser.close();
    return true;
  } catch {
    return false;
  }
}

const hasChromium = await chromiumAvailable();
const browserTest = it.skipIf(!hasChromium);
const ONE_PIXEL_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
  "base64",
);

describe("dropped media uploads", () => {
  let root: string;
  let url: string;
  let server: ViteDevServer;

  beforeAll(async () => {
    root = await mkdtemp(join(tmpdir(), "slidx-media-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), "# One\n");

    server = await createServer({
      root,
      logLevel: "silent",
      plugins: [slidx({ presenter: false, print: false, og: false, overflow: false })],
      server: { port: 0, watch: null, hmr: false },
    });
    await server.listen();
    url = server.resolvedUrls!.local[0]!;
  }, 60_000);

  afterAll(async () => {
    await server?.close();
  });

  async function drop(name: string, type: string, body: BodyInit) {
    return fetch(`${url}__slidx/media`, {
      method: "POST",
      headers: { "content-type": type, "x-slidx-name": encodeURIComponent(name) },
      body,
    });
  }

  it("streams an image byte for byte and returns its deck-root URL", async () => {
    const bytes = new Uint8Array([0, 255, 13, 10, 137, 80, 78, 71]);
    const response = await drop("営業 chart.PNG", "image/png", bytes);

    expect(response.status).toBe(201);
    expect(await response.json()).toEqual({
      kind: "image",
      src: "/slides/assets/%E5%96%B6%E6%A5%AD-chart.png",
      alt: "営業 chart",
    });
    expect(await readFile(join(root, "slides/assets/営業-chart.png"))).toEqual(Buffer.from(bytes));
  });

  it("uses the MIME format and never trusts a misleading extension", async () => {
    const response = await drop("quarterly.svg", "image/png", new Uint8Array([1, 2, 3]));

    expect(await response.json()).toMatchObject({
      kind: "image",
      src: "/slides/assets/quarterly.png",
    });
    expect(await readFile(join(root, "slides/assets/quarterly.png"))).toEqual(
      Buffer.from([1, 2, 3]),
    );
  });

  it("keeps an existing asset and chooses a collision-safe name", async () => {
    await writeFile(join(root, "slides/assets/demo.mov"), "existing");
    const response = await drop("demo.mov", "video/quicktime", new Uint8Array([4, 5, 6]));

    expect(await response.json()).toEqual({
      kind: "video",
      src: "/slides/assets/demo-2.mov",
      alt: "demo",
    });
    expect(await readFile(join(root, "slides/assets/demo.mov"), "utf8")).toBe("existing");
    expect(await readFile(join(root, "slides/assets/demo-2.mov"))).toEqual(Buffer.from([4, 5, 6]));
  });

  it("rejects paths and formats outside the media allowlist", async () => {
    const traversal = await drop("../escape.png", "image/png", new Uint8Array([1]));
    const markup = await drop("page.html", "text/html", new TextEncoder().encode("<script>"));

    expect(traversal.status).toBe(400);
    expect(await traversal.json()).toMatchObject({ message: expect.stringContaining("path") });
    expect(markup.status).toBe(415);
    expect(await markup.json()).toMatchObject({ message: expect.stringContaining("PNG") });
    await expect(readFile(join(root, "escape.png"))).rejects.toThrow();
  });

  it("does not let a read-only collaborator create an asset", async () => {
    const session = createEditSession(root, resolveOptions(), {
      sharing: { on: true, grant: () => Grant.Read },
    });
    const request = {
      url: "/__slidx/media",
      method: "POST",
      headers: {},
      socket: { remoteAddress: "192.0.2.1" },
    } as unknown as IncomingMessage;
    let answer = "";
    const response = {
      statusCode: 0,
      setHeader: () => {},
      end: (value: string) => {
        answer = value;
      },
    } as unknown as ServerResponse;

    try {
      expect(await session.handle(request, response)).toBe(true);
      expect(response.statusCode).toBe(403);
      expect(JSON.parse(answer)).toMatchObject({ message: expect.stringContaining("read") });
    } finally {
      session.close();
    }
  });

  it("tells a read-only collaborator the access their workspace should show", async () => {
    const session = createEditSession(root, resolveOptions(), {
      sharing: { on: true, grant: () => Grant.Read },
    });
    const request = {
      url: "/__slidx/deck",
      method: "GET",
      headers: { [CREDENTIAL_HEADER]: "read-capability" },
      socket: { remoteAddress: "192.0.2.1" },
    } as unknown as IncomingMessage;
    let answer = "";
    const headers = new Map<string, string | number | readonly string[]>();
    const response = {
      statusCode: 0,
      setHeader: (name: string, value: string | number | readonly string[]) => {
        headers.set(name, value);
      },
      end: (value: string) => {
        answer = value;
      },
    } as unknown as ServerResponse;

    try {
      expect(await session.handle(request, response)).toBe(true);
      expect(response.statusCode).toBe(200);
      expect(JSON.parse(answer)).toMatchObject({ access: { canEdit: false } });
      expect(headers.get("set-cookie")).toContain("HttpOnly; SameSite=Strict");
    } finally {
      session.close();
    }
  });

  it("serves the content-free editor shell before a fragment capability can be presented", async () => {
    const session = createEditSession(root, resolveOptions(), {
      sharing: { on: true, grant: () => Grant.None },
    });
    const request = {
      url: "/__slidx/",
      method: "GET",
      headers: {},
      socket: { remoteAddress: "192.0.2.1" },
    } as IncomingMessage;
    let answer = "";
    const response = {
      statusCode: 0,
      setHeader: () => {},
      end: (value: string) => {
        answer = value;
      },
    } as unknown as ServerResponse;

    try {
      expect(await session.handle(request, response)).toBe(true);
      expect(response.statusCode).toBe(200);
      expect(answer).toContain('<div id="slidx-editor">');
      expect(answer).toContain("<title>slidx — editor</title>");
      expect(answer).toContain('<link rel="icon" href="data:image/svg+xml,');
      expect(answer).not.toContain("# One");
    } finally {
      session.close();
    }
  });

  it("removes a partial file when a chunk crosses the size limit", async () => {
    const request = Object.assign(Readable.from([Buffer.from("12"), Buffer.from("34")]), {
      headers: {
        "content-type": "image/png",
        "x-slidx-name": encodeURIComponent("too-large.png"),
      },
    }) as unknown as IncomingMessage;

    await expect(uploadMedia(request, root, "slides", "slides", 3)).rejects.toMatchObject({
      status: 413,
    });
    expect(await readdir(join(root, "slides/assets"))).not.toContain("too-large.png");
  });

  it("does not leave a file behind for an empty upload", async () => {
    const response = await drop("empty.webp", "image/webp", new Uint8Array());

    expect(response.status).toBe(400);
    expect(await response.json()).toMatchObject({ message: expect.stringContaining("empty") });
    expect(await readdir(join(root, "slides/assets"))).not.toContain("empty.webp");
  });

  browserTest(
    "drops a real browser File onto the visual editor and survives reload and build",
    async () => {
      const { chromium } = await import("playwright");
      const browser = await chromium.launch();
      const context = await browser.newContext({
        colorScheme: "dark",
        viewport: { width: 1_600, height: 1_000 },
      });
      const page = await context.newPage();
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));

      try {
        await page.goto(`${url}__slidx/`, { waitUntil: "domcontentloaded" });
        await page.waitForFunction(
          () =>
            document
              .querySelector<HTMLIFrameElement>(".slidx-canvas-frame")
              ?.contentDocument?.querySelector("[data-slidx-region]") !== null,
        );

        // The drag is offered again on every poll rather than once, because a
        // region in the canvas document is not the same moment as an editor
        // listening to that document: the drop surface binds itself on the
        // frame's `load`, which is after the markup it just measured is there.
        // A single dispatch into that window lands on nothing and no later
        // event ever arrives, so the wait would sit out its whole timeout on a
        // deck that works.
        await page.waitForFunction(
          (bytes) => {
            const frame = document.querySelector<HTMLIFrameElement>(".slidx-canvas-frame");
            const preview = frame?.contentDocument;
            const region = preview?.querySelector("[data-slidx-region]");
            if (!preview || !region) return false;

            const rect = region.getBoundingClientRect();
            const transfer = new DataTransfer();
            transfer.items.add(
              new File([new Uint8Array(bytes)], "browser-drop.png", { type: "image/png" }),
            );
            const init: DragEventInit = {
              bubbles: true,
              cancelable: true,
              clientX: rect.left + rect.width / 2,
              clientY: rect.top + 4,
              dataTransfer: transfer,
            };
            preview.dispatchEvent(new DragEvent("dragenter", init));
            preview.dispatchEvent(new DragEvent("dragover", init));
            (
              window as unknown as {
                __slidxDrop: { preview: Document; init: DragEventInit };
              }
            ).__slidxDrop = { preview, init };

            return (
              document.querySelector(".slidx-media-drop")?.getAttribute("data-target") === "body"
            );
          },
          [...ONE_PIXEL_PNG],
        );
        const chrome = await page.locator(".slidx-media-drop").evaluate((element) => {
          const style = getComputedStyle(element);
          return {
            active: element.getAttribute("data-active"),
            padding: style.padding,
            border: style.borderTopWidth,
            background: getComputedStyle(document.body).backgroundColor,
          };
        });
        expect(chrome).toEqual({
          active: "true",
          padding: "24px",
          border: "1px",
          // The editor chrome's dark paper, from the committed brand palette.
          background: "rgb(19, 23, 30)",
        });

        await page.evaluate(() => {
          const holder = window as unknown as {
            __slidxDrop?: { preview: Document; init: DragEventInit };
          };
          const drop = holder.__slidxDrop;
          if (!drop) throw new Error("the file drag was not held");
          drop.preview.dispatchEvent(new DragEvent("drop", drop.init));
          delete holder.__slidxDrop;
        });
        await page.locator('.slidx-media-drop[data-active="false"]').waitFor({ timeout: 15_000 });
        await page.waitForFunction(
          () =>
            document
              .querySelector<HTMLIFrameElement>(".slidx-canvas-frame")
              ?.contentDocument?.querySelector('img[src*="browser-drop.png"]') !== null,
        );

        const source = await readFile(join(root, "slides/0001.md"), "utf8");
        expect(source).toMatch(
          /^!\[browser drop\]\(<\/slides\/assets\/browser-drop\.png>\)\n\n# One/,
        );
        expect(await readFile(join(root, "slides/assets/browser-drop.png"))).toEqual(ONE_PIXEL_PNG);

        await page.reload({ waitUntil: "domcontentloaded" });
        await page.waitForFunction(
          () =>
            document
              .querySelector<HTMLIFrameElement>(".slidx-canvas-frame")
              ?.contentDocument?.querySelector('img[src*="browser-drop.png"]') !== null,
        );

        await build({
          root,
          logLevel: "silent",
          plugins: [slidx({ presenter: false, print: false, og: false, overflow: false })],
          build: { outDir: join(root, "dist") },
        });
        expect(await readFile(join(root, "dist/slides/assets/browser-drop.png"))).toEqual(
          ONE_PIXEL_PNG,
        );
        expect(errors).toEqual([]);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );
});
