import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Readable } from "node:stream";

import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";
import { uploadMedia } from "../src/media-upload";
import { resolveOptions } from "../src/options";
import { Grant } from "../src/share";
import { createEditSession } from "../src/session";

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
      expect(response.statusCode).toBe(403);
      expect(JSON.parse(answer)).toMatchObject({ message: expect.stringContaining("read") });
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
});
