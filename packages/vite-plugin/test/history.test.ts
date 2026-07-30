/**
 * The deck's history, as the editor reaches it.
 *
 * Against a real dev server and a real repository, because the claim being made
 * is about a deck in git rather than about a function: the panel has to work in
 * a repository, say so plainly in a directory that is not one, and describe a
 * commit in slides rather than in lines.
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

/**
 * A deck with three commits, in a project *under* its repository.
 *
 * Where decks live: a talk in a monorepo, a deck beside the demo it is about,
 * this repository's own example. A deck whose project is the repository root
 * is the easy case and hides a real one — git resolves `<rev>:<path>` from the
 * top of the repository and prints tree paths from the directory it ran in.
 */
async function withHistory(): Promise<Session> {
  const repo = await mkdtemp(join(tmpdir(), "slidx-history-"));
  const root = join(repo, "talks", "making-decks-fast");
  await mkdir(join(root, "slides"), { recursive: true });

  const write = (name: string, source: string) =>
    writeFile(join(root, "slides", name), source, "utf8");

  await write(
    "0001.md",
    "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n",
  );
  await write("0002.md", "---\nbudget: 90s\n---\n\n## What goes wrong\n\n- the venue wifi\n");
  await write("0003.md", "## The fix\n");

  await git(repo, "init", "--quiet");
  await git(repo, "config", "user.email", "author@example.com");
  await git(repo, "config", "user.name", "The Author");
  await git(repo, "add", "-A");
  await git(repo, "commit", "--quiet", "-m", "the deck as the author wrote it");

  await write(
    "0002.md",
    "---\nbudget: 120s\n---\n\n## What actually goes wrong\n\n- the venue wifi\n",
  );
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

describe("the deck as a commit had it", () => {
  let session: Session;

  beforeAll(async () => {
    session = await withHistory();
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  it("renders an old commit through the route that renders the working copy", async () => {
    // The whole property this feature rests on: the renderer is a pure
    // function of the source, so the page for an old commit is the real page —
    // the same URL, the same shell, the same theme, the same WebAssembly
    // module. A second way of drawing it would be a second answer about
    // layout, which is the bug this architecture exists to prevent.
    const { commits } = (await get(session, "__slidx/history")) as { commits: { rev: string }[] };
    const first = commits[1]!.rev;

    const now = await (await fetch(`${session.url}slides/2/`)).text();
    const then = await (await fetch(`${session.url}slides/2/?rev=${first}`)).text();

    expect(now).toContain("What actually goes wrong");
    expect(then).toContain("What goes wrong");
    expect(then).not.toContain("What actually goes wrong");

    // Same document, not a preview of one: everything the real page has.
    expect(then).toContain("<!doctype html>");
    expect(then).toContain("slidx-slide-body");
  });

  it("leaves the working copy alone when it renders the past", async () => {
    const { commits } = (await get(session, "__slidx/history")) as { commits: { rev: string }[] };
    await fetch(`${session.url}slides/2/?rev=${commits[1]!.rev}`);

    const source = await readFile(join(session.root, "slides", "0002.md"), "utf8");
    expect(source).toContain("What actually goes wrong");
  });

  it("says there is no such revision rather than quietly showing the present", async () => {
    // A page that silently fell back to the working copy would be the worst
    // possible answer: it looks like history and is not.
    const missing = await fetch(
      `${session.url}slides/2/?rev=0123456789abcdef0123456789abcdef01234567`,
    );

    expect(missing.status).toBe(404);
    expect(await missing.text()).toContain("revision");
  });
});

describe("putting the deck back", () => {
  let session: Session;

  beforeAll(async () => {
    session = await withHistory();
  }, 60_000);

  afterAll(async () => {
    await session?.server.close();
  });

  const restore = async (rev: string) => {
    const response = await fetch(`${session.url}__slidx/history/restore`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ rev }),
    });

    return (await response.json()) as {
      restored?: string;
      previous?: string;
      refused?: string;
      changed?: string[];
    };
  };

  it("refuses rather than writing over work that is not committed", async () => {
    // Looking at history invites going back, and going back must never be the
    // thing that loses an afternoon. The refusal names what is at risk.
    await writeFile(
      join(session.root, "slides", "0003.md"),
      "## The fix\n\nsomething written since the last commit\n",
    );

    const { commits } = (await get(session, "__slidx/history")) as { commits: { rev: string }[] };
    const answer = await restore(commits[1]!.rev);

    expect(answer.restored).toBeUndefined();
    expect(answer.refused).toContain("not committed");
    expect(answer.changed?.join(" ")).toContain("0003.md");

    // And the file it refused to overwrite is exactly as it was.
    expect(await readFile(join(session.root, "slides", "0003.md"), "utf8")).toContain(
      "since the last commit",
    );
  });

  /** Where the deck was before the restore below, for the undo that follows. */
  let undoTarget: string;

  it("puts the deck back to a commit, and says what undoes it", async () => {
    await writeFile(join(session.root, "slides", "0003.md"), "## The fix\n");
    const before = await readFile(join(session.root, "slides", "0002.md"), "utf8");

    const { commits } = (await get(session, "__slidx/history")) as { commits: { rev: string }[] };
    const answer = await restore(commits[1]!.rev);

    expect(answer.restored).toBe(commits[1]!.rev);
    // Where the working copy was, which is HEAD rather than the newest commit
    // that touched the deck — a commit that changed nothing under the deck is
    // still the tree the author's files came from.
    expect(answer.previous).toMatch(/^[0-9a-f]{40}$/);
    undoTarget = answer.previous!;

    const restored = await readFile(join(session.root, "slides", "0002.md"), "utf8");
    expect(restored).toContain("What goes wrong");
    expect(restored).not.toContain("What actually goes wrong");
    expect(restored).not.toBe(before);
  });

  it("undoes a restore with one more restore, back to the byte", async () => {
    // The reason a restore may leave the deck dirty and still be safe: this
    // session knows which dirt is its own, so it can offer to put it back. And
    // undo is not a special path — it is this operation naming the commit the
    // deck was at.
    const undone = await restore(undoTarget);

    expect(undone.restored).toBe(undoTarget);
    expect(await readFile(join(session.root, "slides", "0002.md"), "utf8")).toContain(
      "What actually goes wrong",
    );

    // Nothing left over: the round trip is byte for byte, so git has nothing
    // to report about the deck at all.
    const clean = await git(session.root, "status", "--porcelain", "--", "slides");
    expect(clean).toBe("");
  });

  it("refuses a revision that is a git option rather than an object name", async () => {
    const answer = await restore("--upload-pack=touch /tmp/slidx-restore-route");

    expect(answer.restored).toBeUndefined();
    expect(answer.refused).toBeTruthy();
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
