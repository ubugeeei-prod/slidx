/**
 * A full editing session, judged by its diff.
 *
 * This is the specification the rest of the editor is built to satisfy. slidx
 * claims the canvas and the Markdown are two views of one document, and the
 * only honest test of that claim is what `git diff` says after an author has
 * spent a while in the editor. So this runs a real dev server against a real
 * git repository, posts the operations a real session produces, and asserts on
 * the diff — not on the model, which would only prove the editor agrees with
 * itself.
 */

import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";

const run = promisify(execFile);

/**
 * A deck written by hand, with the formatting a person actually leaves behind.
 *
 * Every quirk here is one a serialiser would tidy away: the aligned frontmatter
 * value, the extra spaces after the hashes, the `*` bullets, the paragraph
 * wrapped where the author felt like wrapping it. None of them may move.
 */
const DECK: Record<string, string> = {
  "0001.md": [
    "---",
    "title:  Making Decks Fast",
    "event:  SlidxConf 2026",
    "duration: 20m",
    "---",
    "",
    "#   Making Decks Fast",
    "",
  ].join("\n"),
  "0002.md": [
    "## What goes wrong",
    "",
    "*  the venue Wi-Fi is down and the fonts were on a CDN",
    "*  the body text was 18px and unreadable from row 12",
    // An em dash: three bytes, one JavaScript character. Everything the
    // session does to a later slide depends on counting the right one.
    "*  the projector washed out a colour pair — badly",
    "",
  ].join("\n"),
  "0003.md": [
    "## The result",
    "",
    "Rewriting the parser in Rust made the whole build 3.2x faster than",
    "the one we started with.",
    "",
  ].join("\n"),
  "0004.md": [
    "---",
    "layout: split",
    "---",
    "",
    "## Where to go next",
    "",
    "See the resources page.",
    "",
  ].join("\n"),
};

interface Session {
  root: string;
  url: string;
  server: ViteDevServer;
}

async function git(root: string, ...args: string[]): Promise<string> {
  const { stdout } = await run("git", ["-C", root, ...args]);
  return stdout;
}

async function open(): Promise<Session> {
  const root = await mkdtemp(join(tmpdir(), "slidx-session-"));
  await mkdir(join(root, "slides"), { recursive: true });

  for (const [name, source] of Object.entries(DECK)) {
    await writeFile(join(root, "slides", name), source);
  }

  // What a real project ignores, so a stray build artefact cannot be mistaken
  // for something the editor wrote.
  await writeFile(join(root, ".gitignore"), "node_modules/\ndist/\n.vite/\n");

  await git(root, "init", "--quiet");
  await git(root, "config", "user.email", "author@example.com");
  await git(root, "config", "user.name", "The Author");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "the deck as the author wrote it");

  const server = await createServer({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    // No watcher and no HMR socket. This test posts operations and reads the
    // files back; it never needs either — and on Windows both hold handles
    // that outlive `server.close()`, which crashes the worker *after* every
    // test in the run has passed. A green suite that fails at teardown is the
    // worst shape of failure to debug, so the handles are never opened.
    server: { port: 0, watch: null, hmr: false },
  });
  await server.listen();

  return { root, url: server.resolvedUrls!.local[0]!, server };
}

/** Posts one operation the way the editor does, and fails loudly if it was refused. */
async function post(session: Session, body: unknown): Promise<Record<string, unknown>> {
  const response = await fetch(`${session.url}__slidx/edit`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });

  const payload = (await response.json()) as Record<string, unknown>;
  expect(response.status, JSON.stringify(payload)).toBe(200);
  expect(payload.error, JSON.stringify(payload.error)).toBeUndefined();

  return payload;
}

describe("an editing session, seen as a diff", () => {
  let session: Session;
  let diff: string;
  let status: string;

  beforeAll(async () => {
    session = await open();

    // Retitling a slide from the canvas.
    await post(session, { op: { op: "setHeading", slide: 1, text: "What goes wrong on stage" } });

    // Selecting three words and giving them a class: how "add an animation to
    // this phrase" is spelled in a file.
    const body = DECK["0003.md"]!.trim();
    const start = body.indexOf("3.2x faster");
    await post(session, {
      op: {
        op: "addMark",
        slide: 2,
        range: { start, end: start + "3.2x faster".length },
        attributes: { key: "result", classes: ["accent"] },
      },
    });

    // The inspector, writing a field and a step onto the same slide.
    await post(session, { op: { op: "setField", slide: 2, key: "budget", value: "90s" } });
    await post(session, {
      op: { op: "addStep", slide: 2, action: { reveal: { target: "#result", options: {} } } },
    });

    // The notes pane.
    await post(session, {
      op: { op: "setNotes", slide: 0, notes: "Open with the outcome, not the agenda." },
    });

    // The outline: a new slide, dragged somewhere, then dragged back.
    await post(session, { op: { op: "insertSlide", at: 3, body: "## One more thing" } });
    const moved = await post(session, { op: { op: "moveSlide", slide: 3, to: 1 } });
    await post(session, { edit: moved.undo });

    diff = await git(session.root, "diff");
    status = await git(session.root, "status", "--porcelain");
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  it("touches only the files the session named", () => {
    // Modified, all four of them, and nothing added, deleted, or renamed.
    expect(status.split("\n").filter(Boolean).sort()).toEqual([
      " M slides/0001.md",
      " M slides/0002.md",
      " M slides/0003.md",
      " M slides/0004.md",
    ]);
  });

  it("leaves every line the session did not name exactly as the author typed it", () => {
    // A removal line for any of these means a serialiser got in: the aligned
    // frontmatter, the loose hashes, the `*` bullets, the hand-wrapped
    // paragraph. Not one of them is what an operation named.
    for (const untouched of [
      "title:  Making Decks Fast",
      "event:  SlidxConf 2026",
      "#   Making Decks Fast",
      "*  the venue Wi-Fi is down and the fonts were on a CDN",
      "*  the body text was 18px and unreadable from row 12",
      "the one we started with.",
    ]) {
      expect(diff, `the session rewrote ${JSON.stringify(untouched)}`).not.toContain(
        `-${untouched}`,
      );
    }
  });

  it("removes one line per thing the session actually changed", () => {
    // Eight operations, and only the two lines that were genuinely replaced —
    // a heading and a sentence — come out. Everything else is an addition.
    expect(removed(diff)).toEqual([
      "## What goes wrong",
      "Rewriting the parser in Rust made the whole build 3.2x faster than",
    ]);
  });

  it("writes a slide's fields as a frontmatter block a person would have typed", () => {
    // Two operations on one slide, and one block. A key and a step list, in
    // the order they were asked for, with the sentence around them untouched
    // apart from the three words that were marked.
    expect(hunk(diff, "slides/0003.md")).toEqual([
      "+---",
      "+budget: 90s",
      "+steps:",
      '+  - reveal: "#result"',
      "+---",
      "+",
      " ## The result",
      " ",
      "-Rewriting the parser in Rust made the whole build 3.2x faster than",
      "+Rewriting the parser in Rust made the whole build [3.2x faster]{#result .accent} than",
      " the one we started with.",
    ]);
  });

  it("leaves no trace of the slide that was dragged and dragged back", () => {
    // The reorder and its undo cancel byte for byte, so the only change to the
    // file that held them is the slide the author actually added.
    expect(hunk(diff, "slides/0004.md")).toEqual([
      "+## One more thing",
      "+",
      " ---",
      " layout: split",
      " ---",
    ]);
  });

  it("puts the speaker's notes next to the slide they belong to", () => {
    expect(hunk(diff, "slides/0001.md")).toEqual([
      " ---",
      " ",
      " #   Making Decks Fast",
      "+",
      "+<!-- notes: Open with the outcome, not the agenda. -->",
    ]);
  });

  it("leaves the deck parsing without a blocking diagnostic", async () => {
    const response = await fetch(`${session.url}__slidx/deck`);
    const payload = (await response.json()) as {
      deck: { hasBlocking: boolean; slides: unknown[] };
    };

    expect(payload.deck.hasBlocking).toBe(false);
    expect(payload.deck.slides).toHaveLength(5);
  });

  it("keeps every file ending in a newline", async () => {
    for (const name of Object.keys(DECK)) {
      const source = await readFile(join(session.root, "slides", name), "utf8");
      expect(source.endsWith("\n"), `${name} lost its final newline`).toBe(true);
    }
  });
});

describe("the editor in the dev server", () => {
  let session: Session;

  beforeAll(async () => {
    session = await open();
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  it("is served by the server that already has the deck, with nothing else to start", async () => {
    // `vite dev` gives the author their deck and the editor. A second process
    // would be a second port to remember and a second copy of the deck.
    const response = await fetch(`${session.url}__slidx/`);
    const page = await response.text();

    expect(response.headers.get("content-type")).toContain("text/html");
    expect(page).toContain('<div id="slidx-editor">');
    expect(page).toContain('import { mount } from "/__slidx/editor.js"');
    expect(page).toContain('deckBase: "slides"');
  });

  it("serves the editor as one module and nothing to install", async () => {
    const response = await fetch(`${session.url}__slidx/editor.js`);

    expect(response.headers.get("content-type")).toContain("javascript");
    expect(await response.text()).toContain("mount");
  });

  it("keeps the editor out of search results", async () => {
    // It writes to the author's files. It is not a page anyone should reach
    // from anywhere but their own machine.
    expect(await (await fetch(`${session.url}__slidx/`)).text()).toContain('name="robots"');
  });

  it("says which slide has a problem in the same call that returns the deck", async () => {
    const response = await fetch(`${session.url}__slidx/deck`);
    const payload = (await response.json()) as {
      spans: { body: { start: number; end: number } }[];
      deck: { diagnostics: unknown[]; slides: { frontmatter?: Record<string, unknown> }[] };
    };

    // Live diagnostics cost the editor nothing: the pipeline returns them with
    // every parse, so wiring them is reading a field.
    expect(payload.deck.diagnostics).toBeInstanceOf(Array);
    expect(payload.spans).toHaveLength(4);
    expect(payload.deck.slides[0]!.frontmatter).toMatchObject({ title: "Making Decks Fast" });
  });
});

describe("the editing routes", () => {
  it("answer an operation that names a slide the deck does not have", async () => {
    const session = await open();

    try {
      const response = await fetch(`${session.url}__slidx/edit`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ op: { op: "setHeading", slide: 99, text: "x" } }),
      });

      expect(response.status).toBe(200);
      expect(await response.json()).toMatchObject({ error: { error: "noSuchSlide", slide: 99 } });
      expect((await git(session.root, "status", "--porcelain")).trim()).toBe("");
    } finally {
      await session.server.close();
    }
  }, 60_000);
});

/** The lines a diff takes away, without the marker. */
function removed(diff: string): string[] {
  return diff
    .split("\n")
    .filter((line) => line.startsWith("-") && !line.startsWith("---"))
    .map((line) => line.slice(1));
}

/** One file's diff below its header, exactly as git prints it. */
function hunk(diff: string, label: string): string[] {
  const rest = diff.slice(diff.indexOf(`diff --git a/${label}`));
  const next = rest.indexOf("\ndiff --git ");
  const lines = (next === -1 ? rest : rest.slice(0, next)).split("\n");

  const body = lines.slice(lines.findIndex((line) => line.startsWith("@@")) + 1);
  while (body.length > 0 && body[body.length - 1] === "") body.pop();

  return body;
}
