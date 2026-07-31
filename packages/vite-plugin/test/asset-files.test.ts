import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build, createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";

describe("deck-owned asset files", () => {
  let root: string;
  let url: string;
  let server: ViteDevServer;
  const video = Buffer.from([0, 1, 2, 3, 4, 5, 254, 255]);
  const chart = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  beforeAll(async () => {
    root = await mkdtemp(join(tmpdir(), "slidx-deck-assets-"));
    await mkdir(join(root, "talk/assets/nested"), { recursive: true });
    await writeFile(
      join(root, "talk/0001.md"),
      '# One\n\n<video controls src="/deck/assets/demo.mp4"></video>\n',
    );
    await writeFile(join(root, "talk/assets/demo.mp4"), video);
    await writeFile(join(root, "talk/assets/nested/chart.png"), chart);
    await writeFile(join(root, "talk/assets/.private"), "not an asset");

    const options = {
      srcDir: "talk",
      base: "deck",
      presenter: false,
      print: false,
      og: false,
      overflow: false,
    };
    await build({
      root,
      logLevel: "silent",
      plugins: [slidx(options)],
      build: { outDir: join(root, "dist") },
    });

    server = await createServer({
      root,
      logLevel: "silent",
      plugins: [slidx(options)],
      server: { port: 0, watch: null, hmr: false },
    });
    await server.listen();
    url = server.resolvedUrls!.local[0]!;
  }, 60_000);

  afterAll(async () => {
    await server?.close();
  });

  it("copies nested assets into the production route byte for byte", async () => {
    expect(await readFile(join(root, "dist/deck/assets/demo.mp4"))).toEqual(video);
    expect(await readFile(join(root, "dist/deck/assets/nested/chart.png"))).toEqual(chart);
    await expect(readFile(join(root, "dist/deck/assets/.private"))).rejects.toThrow();
  });

  it("serves the same route when source and public deck directories differ", async () => {
    const response = await fetch(`${url}deck/assets/demo.mp4`);

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("video/mp4");
    expect(response.headers.get("accept-ranges")).toBe("bytes");
    expect(Buffer.from(await response.arrayBuffer())).toEqual(video);
  });

  it("answers a single byte range so a dropped video can seek", async () => {
    const response = await fetch(`${url}deck/assets/demo.mp4`, {
      headers: { range: "bytes=2-5" },
    });
    const suffix = await fetch(`${url}deck/assets/demo.mp4`, {
      headers: { range: "bytes=-2" },
    });
    const head = await fetch(`${url}deck/assets/demo.mp4`, { method: "HEAD" });

    expect(response.status).toBe(206);
    expect(response.headers.get("content-range")).toBe(`bytes 2-5/${video.byteLength}`);
    expect(response.headers.get("content-length")).toBe("4");
    expect(Buffer.from(await response.arrayBuffer())).toEqual(video.subarray(2, 6));
    expect(Buffer.from(await suffix.arrayBuffer())).toEqual(video.subarray(-2));
    expect(head.status).toBe(200);
    expect(head.headers.get("content-length")).toBe(String(video.byteLength));
    expect((await head.arrayBuffer()).byteLength).toBe(0);
  });

  it("rejects malformed or escaping ranges and paths", async () => {
    const range = await fetch(`${url}deck/assets/demo.mp4`, {
      headers: { range: "bytes=999-1000" },
    });
    const traversal = await fetch(`${url}deck/assets/%2e%2e%2f0001.md`);

    expect(range.status).toBe(416);
    expect(range.headers.get("content-range")).toBe(`bytes */${video.byteLength}`);
    expect(traversal.status).not.toBe(200);
    expect(await traversal.text()).not.toContain("# One");
  });
});
