/**
 * Writing an edit back to the files the deck was read from.
 *
 * A deck is one source to the parser and a directory of files to the author.
 * `slidx_edit` splices the source; this is the half that decides which file
 * each spliced byte was in. Everything here is stated as "which files changed,
 * and to what", because that is what a reviewer sees.
 */

import { describe, expect, it } from "vite-plus/test";

import { applyOperation, locate, revertOperation, type DeckFile } from "../src/edit";
import { joinDeck } from "../src/files";

const SEPARATOR = "---";

/** A deck written the way a person writes one, spacing quirks included. */
function deck(): DeckFile[] {
  return [
    {
      path: "/slides/0001.md",
      label: "slides/0001.md",
      source: "---\ntitle: Fast Decks\nduration: 20m\n---\n\n#   Fast Decks\n",
    },
    {
      path: "/slides/0002.md",
      label: "slides/0002.md",
      source: "## What we will cover\n\n*  Why the parser matters\n*  What the linter catches\n",
    },
    {
      path: "/slides/0003.md",
      label: "slides/0003.md",
      source: "---\nlayout: split\n---\n\n## Numbers\n\nLatency dropped to 120ms.\n",
    },
  ];
}

/** The deck after one operation, as the files it would be written to. */
async function edited(files: DeckFile[], op: Record<string, unknown>) {
  const result = await applyOperation(files, SEPARATOR, op);
  expect(result.error).toBeUndefined();

  return result;
}

/** The files a write plan touches, by label. */
function touched(writes: { path: string; source: string | null }[]): string[] {
  return writes.map((write) => write.path.replace("/slides/", "")).sort();
}

describe("writing an edit back to the files it came from", () => {
  it("rewrites only the file holding the slide that changed", async () => {
    const { writes } = await edited(deck(), {
      op: "setHeading",
      slide: 1,
      text: "What we will cover today",
    });

    expect(touched(writes)).toEqual(["0002.md"]);
    expect(writes[0]!.source).toBe(
      "## What we will cover today\n\n*  Why the parser matters\n*  What the linter catches\n",
    );
  });

  it("leaves the spacing the author chose in the lines it did not name", async () => {
    // The `*` bullets and the three spaces after the `#` are what a serialiser
    // would tidy away. The whole point of a splice is that they are not read.
    const { writes } = await edited(deck(), {
      op: "setField",
      slide: 1,
      key: "budget",
      value: "90s",
    });

    expect(writes[0]!.source).toContain("*  Why the parser matters");
  });

  it("keeps the trailing newline the author left on a file", async () => {
    // A file that loses its final newline shows up as a whole-file diff in
    // every review tool, which is the opposite of the point.
    const { writes } = await edited(deck(), { op: "setNotes", slide: 2, notes: "read the graph" });

    expect(writes[0]!.source!.endsWith("\n")).toBe(true);
    expect(writes[0]!.source!.endsWith("\n\n")).toBe(false);
  });

  it("writes nothing at all when the operation asks for what the deck already says", async () => {
    // Not "writes the same bytes" — no write. The difference is a file whose
    // modification time never moves and a watcher that never fires.
    const { writes, undo } = await edited(deck(), {
      op: "setHeading",
      slide: 1,
      text: "What we will cover",
    });

    expect(writes).toEqual([]);
    expect(undo).toEqual([]);
  });

  it("deletes the file a removed slide was the whole of", async () => {
    const { writes } = await edited(deck(), { op: "removeSlide", slide: 1 });

    expect(writes).toEqual([{ path: "/slides/0002.md", label: "slides/0002.md", source: null }]);
  });

  it("moves the deck's frontmatter to the file of the slide that is now first", async () => {
    // The deck's title belongs to no slide, so deleting the first slide must
    // not take it away. It follows whichever slide now opens the deck.
    const { writes } = await edited(deck(), { op: "removeSlide", slide: 0 });

    expect(touched(writes)).toEqual(["0001.md", "0002.md"]);
    expect(writes.find((write) => write.label === "slides/0001.md")!.source).toBeNull();
    expect(writes.find((write) => write.label === "slides/0002.md")!.source).toBe(
      "---\ntitle: Fast Decks\nduration: 20m\n---\n\n## What we will cover\n\n" +
        "*  Why the parser matters\n*  What the linter catches\n",
    );
  });

  it("moves a slide's bytes between files rather than renaming a file", async () => {
    // A slide file's name is the author's — `intro.md` says something, and so
    // does a numeric prefix. Reordering is a change to what the files hold,
    // never to what they are called.
    const { writes } = await edited(deck(), { op: "moveSlide", slide: 2, to: 1 });

    expect(touched(writes)).toEqual(["0002.md", "0003.md"]);
    expect(writes.find((write) => write.label === "slides/0002.md")!.source).toContain(
      "## Numbers",
    );
    expect(writes.find((write) => write.label === "slides/0003.md")!.source).toContain(
      "## What we will cover",
    );
  });

  it("puts an inserted slide in the file of the slide it pushed down", async () => {
    const { writes } = await edited(deck(), { op: "insertSlide", at: 1, body: "## A new point" });

    expect(touched(writes)).toEqual(["0002.md"]);
    expect(writes[0]!.source).toBe(
      "## A new point\n\n---\n\n## What we will cover\n\n" +
        "*  Why the parser matters\n*  What the linter catches\n",
    );
  });

  it("writes the right file when the deck is not pure ASCII", async () => {
    // Every span the pipeline reports is a byte offset. An em dash is three
    // bytes and one JavaScript character, so counting characters puts every
    // later slide two bytes out and a write lands inside a word. This was
    // found by opening the example deck, which has one.
    const files: DeckFile[] = [
      { path: "/slides/0001.md", label: "slides/0001.md", source: "# One — really\n" },
      { path: "/slides/0002.md", label: "slides/0002.md", source: "# Two\n" },
      { path: "/slides/0003.md", label: "slides/0003.md", source: "# 日本語\n" },
    ];

    const { writes } = await edited(files, { op: "setHeading", slide: 2, text: "Retitled" });

    expect(writes).toEqual([
      { path: "/slides/0003.md", label: "slides/0003.md", source: "# Retitled\n" },
    ]);
  });

  it("keeps the other slides of a file that holds more than one", async () => {
    const files: DeckFile[] = [
      { path: "/talk.md", label: "talk.md", source: "# One\n\n---\n\n# Two\n\n---\n\n# Three\n" },
    ];
    const { writes } = await edited(files, { op: "setHeading", slide: 1, text: "Middle" });

    expect(writes[0]!.source).toBe("# One\n\n---\n\n# Middle\n\n---\n\n# Three\n");
  });

  it("refuses a deck whose files do not break on slide boundaries", async () => {
    // An unclosed fence swallows the separator between two files, so one slide
    // spans both and there is no file to write it to. Saying so beats writing
    // half of it.
    const files: DeckFile[] = [
      { path: "/slides/0001.md", label: "slides/0001.md", source: "```md\n" },
      { path: "/slides/0002.md", label: "slides/0002.md", source: "```\n" },
    ];

    await expect(
      applyOperation(files, SEPARATOR, { op: "setHeading", slide: 0, text: "x" }),
    ).rejects.toThrow(/slides\/0001\.md/);
  });

  it("hands back an operation that names a slide the deck no longer has", async () => {
    // The editor posts operations built from a deck it parsed a keystroke ago.
    // That race is ordinary traffic, so it comes back as an answer.
    const result = await applyOperation(deck(), SEPARATOR, { op: "removeSlide", slide: "deleted" });

    expect(result.error).toEqual({ error: "noSuchSlide", slide: "deleted" });
    expect(result.writes).toEqual([]);
  });
});

describe("the deck read back off disk", () => {
  it("is byte for byte the deck the operation returned", async () => {
    // The editor holds byte offsets from the last answer and the next request
    // re-reads the files, so joining has to be the exact inverse of cutting.
    // Where it is not, every offset shifts and the next edit lands on the
    // wrong bytes — which reads as the editor corrupting the file at random.
    const operations = [
      { op: "setHeading", slide: 1, text: "Retitled" },
      { op: "setField", slide: 1, key: "budget", value: "90s" },
      { op: "addStep", slide: 1, action: { reveal: { target: ".a", options: {} } } },
      { op: "setNotes", slide: 2, notes: "said out loud" },
      { op: "insertSlide", at: 2, body: "## Added" },
      { op: "insertSlide", at: 3, body: "## Appended" },
      { op: "moveSlide", slide: 0, to: 2 },
      { op: "removeSlide", slide: 1 },
    ];

    for (const op of operations) {
      const result = await edited(deck(), op);
      const written = applied(deck(), result.writes);

      expect(joinDeck(written, SEPARATOR).source, `${JSON.stringify(op)} did not round trip`).toBe(
        result.source,
      );
    }
  });
});

describe("taking an edit back", () => {
  it("restores every file byte for byte, including the ones it had deleted", async () => {
    const before = deck();
    const done = await edited(before, { op: "removeSlide", slide: 1 });

    const undone = await revertOperation(applied(before, done.writes), SEPARATOR, done.undo);

    expect(applied(before, [...done.writes, ...undone.writes])).toEqual(before);
  });

  it("survives a session of several operations, in reverse", async () => {
    let files = deck();
    const history: Awaited<ReturnType<typeof applyOperation>>[] = [];

    for (const op of [
      { op: "setHeading", slide: 0, text: "Making Decks Fast" },
      { op: "insertSlide", at: 1, body: "## The problem" },
      { op: "setNotes", slide: 1, notes: "one sentence" },
      { op: "moveSlide", slide: 3, to: 1 },
    ]) {
      const result = await edited(files, op);
      history.push(result);
      files = applied(files, result.writes);
    }

    for (const step of history.reverse()) {
      const undone = await revertOperation(files, SEPARATOR, step.undo);
      files = applied(files, undone.writes);
    }

    expect(files).toEqual(deck());
  });

  it("moves, undoes and redoes one slide without multiplying a multi-slide file", async () => {
    let files = deck();

    const inserted = await edited(files, {
      op: "insertSlide",
      at: 1,
      body: "## Edited in Markdown\n\nBoth views stay synchronized.",
    });
    files = applied(files, inserted.writes);

    const duplicated = await edited(files, { op: "duplicateSlide", slide: 1 });
    expect((await locate(duplicated.source, SEPARATOR)).slides).toHaveLength(5);
    files = applied(files, duplicated.writes);
    expect(joinDeck(files, SEPARATOR).source).toBe(duplicated.source);

    const moved = await edited(files, { op: "moveSlide", slide: 3, to: 2 });
    files = applied(files, moved.writes);

    const undone = await revertOperation(files, SEPARATOR, moved.undo);
    files = applied(files, undone.writes);

    const redone = await revertOperation(files, SEPARATOR, undone.undo);
    files = applied(files, redone.writes);

    expect(joinDeck(files, SEPARATOR).source).toBe(moved.source);
  });
});

/**
 * The file set with a write plan applied, as the dev server holds it.
 *
 * A deleted file keeps its place with nothing in it. On disk it is gone; in
 * the session's list it is the gap an undo puts its slides back into.
 */
function applied(
  files: DeckFile[],
  writes: { label: string; source: string | null }[],
): DeckFile[] {
  const next = files.map((file) => ({ ...file }));

  for (const write of writes) {
    const found = next.find((file) => file.label === write.label);
    if (found) found.source = write.source ?? "";
  }

  return next;
}
