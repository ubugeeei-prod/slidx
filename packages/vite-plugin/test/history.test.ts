/**
 * The deck's history, as the editor reaches it.
 *
 * Against a real dev server and a real repository, because the claim being made
 * is about a deck in git rather than about a function: the panel has to work in
 * a repository, say so plainly in a directory that is not one, and describe a
 * commit in slides rather than in lines.
 */

import { execFile } from "node:child_process";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";

import { slidx } from "../src/index";

const run = promisify(execFile);

interface Session {
  root: string;
  url: string;
  server: ViteDevServer;
}

async function git(root: string, ...args: string[]): Promise<string> {
  const { stdout } = await run("git", ["-C", root, ...args]);
  return stdout;
}

async function serve(root: string): Promise<Session> {
  const server = await createServer({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    server: { port: 0, watch: null, hmr: false },
  });
  await server.listen();

  return { root, url: server.resolvedUrls!.local[0]!, server };
}

async function get(session: Session, path: string): Promise<Record<string, unknown>> {
  const response = await fetch(`${session.url}${path}`);
  return (await response.json()) as Record<string, unknown>;
}

/** A deck with three commits: written, retitled and restaged, then reordered. */
async function withHistory(): Promise<Session> {
  const root = await mkdtemp(join(tmpdir(), "slidx-history-"));
  await mkdir(join(root, "slides"), { recursive: true });

  const write = (name: string, source: string) =>
    writeFile(join(root, "slides", name), source, "utf8");

  await write("0001.md", "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n");
  await write("0002.md", "---\nbudget: 90s\n---\n\n## What goes wrong\n\n- the venue wifi\n");
  await write("0003.md", "## The fix\n");

  await git(root, "init", "--quiet");
  await git(root, "config", "user.email", "author@example.com");
  await git(root, "config", "user.name", "The Author");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "the deck as the author wrote it");

  await write("0002.md", "---\nbudget: 120s\n---\n\n## What actually goes wrong\n\n- the venue wifi\n");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "rework the middle");

  // Something that is not the deck, so the log has a commit to leave out.
  await writeFile(join(root, "README.md"), "# A repository that is not only a deck\n");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "write a readme");

  return serve(root);
}

describe("the deck's history in the editor", () => {
  let session: Session;

  beforeAll(async () => {
    session = await withHistory();
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  it("lists who changed the deck and when, newest first", async () => {
    const answer = (await get(session, "__slidx/history")) as {
      available: boolean;
      commits: { rev: string; author: string; date: string; subject: string }[];
    };

    expect(answer.available).toBe(true);
    expect(answer.commits.map((commit) => commit.subject)).toEqual([
      "rework the middle",
      "the deck as the author wrote it",
    ]);
    expect(answer.commits[0]!.author).toBe("The Author");
    expect(Number.isNaN(Date.parse(answer.commits[0]!.date))).toBe(false);
  });

  it("leaves out the commits that did not touch the deck", async () => {
    // A deck usually lives in a repository that holds other things. A history
    // panel showing a README commit is a panel an author stops reading.
    const answer = (await get(session, "__slidx/history")) as {
      commits: { subject: string }[];
    };

    expect(answer.commits.map((commit) => commit.subject)).not.toContain("write a readme");
  });

  it("says what a commit did to the deck in slides rather than in lines", async () => {
    // The whole reason this is not `git show`. git can say `+3 −3`; a parser
    // can say the slide was retitled and its budget grew.
    const { commits } = (await get(session, "__slidx/history")) as {
      commits: { rev: string }[];
    };

    const change = (await get(session, `__slidx/history/change?rev=${commits[0]!.rev}`)) as {
      subject: string;
      changes: string[];
      slides: number;
    };

    expect(change.subject).toBe('Retitle "What goes wrong" to "What actually goes wrong"');
    expect(change.changes.join("\n")).toContain("1m30s to 2m");
    expect(change.slides).toBe(3);
  });

  it("describes the deck's first commit as a deck arriving", async () => {
    // There is no earlier version to compare against, and "3 slides added"
    // would be a strange way to describe a talk turning up.
    const { commits } = (await get(session, "__slidx/history")) as {
      commits: { rev: string }[];
    };

    const change = (await get(session, `__slidx/history/change?rev=${commits[1]!.rev}`)) as {
      first: boolean;
      subject: string;
    };

    expect(change.first).toBe(true);
    expect(change.subject).toBe("Add the deck, 3 slides");
  });

  it("answers a revision this repository does not have rather than failing", async () => {
    // The panel builds its request from a log it read a moment ago. A rebase
    // since then is ordinary traffic, not a broken editor.
    const missing = await fetch(
      `${session.url}__slidx/history/change?rev=0123456789abcdef0123456789abcdef01234567`,
    );

    expect(missing.status).toBe(404);
  });

  it("ships the panel that reads these routes in the module the editor loads", async () => {
    // The routes existing and the panel existing are two facts; a person can
    // only reach the feature if the module the dev server serves has both. It
    // is the same check that would have caught a compiled step pipeline
    // nothing ever asked for.
    const module = await (await fetch(`${session.url}__slidx/editor.js`)).text();

    expect(module).toContain("slidx-revisions");
    expect(module).toContain("history/change?rev=");
  });

  it("refuses a revision that is a git option rather than an object name", async () => {
    // `--upload-pack` names a program to run. It never reaches a process:
    // the rule is checked before one starts.
    const hostile = await fetch(
      `${session.url}__slidx/history/change?rev=${encodeURIComponent("--upload-pack=touch /tmp/slidx-route")}`,
    );

    expect(hostile.status).toBe(404);
  });
});

describe("a deck that is not in a repository", () => {
  let session: Session;

  beforeAll(async () => {
    const root = await mkdtemp(join(tmpdir(), "slidx-nogit-"));
    await mkdir(join(root, "slides"), { recursive: true });
    await writeFile(join(root, "slides", "0001.md"), "# A deck nobody has committed\n");

    session = await serve(root);
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  it("loads the editor and says there is no history, rather than erroring", async () => {
    // A deck in a directory nobody ran `git init` in is an ordinary situation.
    const page = await fetch(`${session.url}__slidx/`);
    expect(page.status).toBe(200);

    const answer = (await get(session, "__slidx/history")) as {
      available: boolean;
      reason: string;
      commits: unknown[];
    };

    expect(answer.available).toBe(false);
    expect(answer.commits).toEqual([]);
    expect(answer.reason).toContain("not in a git repository");
  });

  it("still serves the deck and the editing routes", async () => {
    // History is one panel. Nothing else in the editor depends on it.
    const deck = (await get(session, "__slidx/deck")) as { deck: { slides: unknown[] } };

    expect(deck.deck.slides).toHaveLength(1);
  });
});
