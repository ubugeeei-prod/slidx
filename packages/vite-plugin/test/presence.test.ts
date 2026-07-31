/**
 * Two browsers on one dev server, and what each is told about the other.
 *
 * The unit tests either side of this one prove the pieces: the roster keeps a
 * block, the route reads one off a body, the editor posts one, the canvas draws
 * one. None of them proves the four are connected, and that is exactly the
 * failure this repository keeps finding — a feature built, tested and merged
 * with nobody able to reach it. So this opens two real streams over real HTTP
 * and asserts that what the second one says about itself arrives at the first.
 */

import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { readFrames, type Frame } from "@slidxjs/editor";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";

/** How long a read may wait for a frame that should already be on its way. */
const PATIENCE_MS = 10_000;

describe("what one browser is told about another", () => {
  let url: string;
  let server: ViteDevServer;
  const listening: AbortController[] = [];

  beforeAll(async () => {
    const root = await mkdtemp(join(tmpdir(), "slidx-presence-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), "# One\n\nA paragraph.\n");
    await writeFile(join(root, "slides", "0002.md"), "# Two\n");

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
    // Aborted rather than left to the server: an event stream ends when
    // somebody says so, and `close()` waits for the connections it is holding.
    for (const listener of listening) listener.abort();
    await server?.close();
  });

  /** Joins the room the way an editor does, and reads its frames as they land. */
  async function connect(): Promise<{ id: string; next(event: string): Promise<Frame> }> {
    const stopper = new AbortController();
    listening.push(stopper);

    const response = await fetch(`${url}__slidx/live`, { signal: stopper.signal });
    const reader = response.body!.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    let waiting: Frame[] = [];

    async function next(event: string): Promise<Frame> {
      const deadline = Date.now() + PATIENCE_MS;

      for (;;) {
        const found = waiting.findIndex((frame) => frame.event === event);
        if (found !== -1) return waiting.splice(found, 1)[0]!;
        if (Date.now() > deadline) throw new Error(`no ${event} frame arrived`);

        const { value, done } = await reader.read();
        if (done) throw new Error(`the stream ended before a ${event} frame`);

        buffer += decoder.decode(value, { stream: true });
        const read = readFrames(buffer);
        buffer = read.rest;
        waiting = [...waiting, ...read.frames];
      }
    }

    const hello = await next("hello");

    return { id: hello.data["id"] as string, next };
  }

  /** Says where a seat is, the way the editor's presence surface does. */
  async function here(said: Record<string, unknown>): Promise<void> {
    const response = await fetch(`${url}__slidx/here`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(said),
    });

    expect(response.status).toBe(204);
  }

  /** The viewer a roster frame has to say something about. */
  function viewerIn(frame: Frame, id: string): Record<string, unknown> | undefined {
    const viewers = frame.data["viewers"] as Record<string, unknown>[];

    return viewers.find((viewer) => viewer["id"] === id);
  }

  it("carries the slide and the block the other browser reported", async () => {
    const first = await connect();
    const second = await connect();

    await here({ id: second.id, slide: 1, block: 2 });

    // Whichever frame the roster change lands in: the second browser joining is
    // itself a change, so there is more than one on the way.
    for (;;) {
      const seen = viewerIn(await first.next("presence"), second.id);
      if (seen?.["block"] !== undefined) {
        expect(seen).toMatchObject({ slide: 1, block: 2 });
        return;
      }
    }
  });

  it("says nothing about a block once the other browser has deselected", async () => {
    const first = await connect();
    const second = await connect();

    await here({ id: second.id, slide: 1, block: 2 });
    await here({ id: second.id, slide: 1 });

    for (;;) {
      const seen = viewerIn(await first.next("presence"), second.id);
      if (seen !== undefined && seen["slide"] === 1 && seen["block"] === undefined) {
        expect("block" in seen).toBe(false);
        return;
      }
    }
  });
});
