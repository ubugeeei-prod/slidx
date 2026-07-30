/**
 * Collaborative editing, and the one property that makes it honest.
 *
 * `packages/editor/src/history.ts` refuses to hold a second model of the
 * document, and says that is the failure the architecture exists to prevent. A
 * CRDT is on its face exactly that second model, so the claim has to be tested
 * rather than argued: **with a single editor connected, the bytes on disk are
 * identical to what the splice would have written on its own.**
 *
 * That is the first block below, and it runs every operation the editor can send
 * through both paths and compares the files. Everything after it is about the
 * cases where the shared document does earn its place.
 */

import { describe, expect, it } from "vite-plus/test";

import { createSharedDeck, spliceBetween } from "../src/collab";
import { createRoster, PRESENCE_TIMEOUT_MS } from "../src/collab/presence";
import { createRoom } from "../src/collab/room";
import { frame } from "../src/collab/stream";
import { applyOperation, revertOperation, type DeckFile } from "../src/edit";
import { Grant } from "../src/share";

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
      source: "---\nlayout: split\n---\n\n## Numbers\n\nLatency dropped to 120ms 🎉.\n",
    },
  ];
}

/** A room with nothing else connected, which is the case under test. */
function alone() {
  return createRoom({ deckState: () => Promise.resolve({}) });
}

/** Every operation the editor can send, in the shapes it sends them. */
const OPERATIONS: Record<string, Record<string, unknown>> = {
  setHeading: { op: "setHeading", slide: 1, text: "What we will cover today" },
  setBody: { op: "setBody", slide: 2, body: "## Numbers\n\nLatency dropped to 38ms.\n" },
  setField: { op: "setField", slide: 1, key: "budget", value: "90s" },
  setNotes: { op: "setNotes", slide: 0, notes: "Open with the outcome." },
  insertSlide: { op: "insertSlide", at: 1, body: "## Inserted\n" },
  removeSlide: { op: "removeSlide", slide: 1 },
  moveSlide: { op: "moveSlide", slide: 2, to: 0 },
  addMark: {
    op: "addMark",
    slide: 2,
    range: { start: 24, end: 29 },
    attributes: { classes: ["accent"] },
  },
  addStep: { op: "addStep", slide: 1, action: { reveal: { target: "#a", options: {} } } },
};

describe("a shared document under the splice", () => {
  for (const [name, op] of Object.entries(OPERATIONS)) {
    it(`writes the same bytes for ${name} with one editor as it would with none`, async () => {
      // The property in full. Not "an equivalent document" and not "the same
      // rendering" — the same files, byte for byte, including the author's `*`
      // bullets and the three spaces after their `#`.
      const direct = await applyOperation(deck(), SEPARATOR, op);
      const shared = await applyOperation(deck(), SEPARATOR, op, alone().reconciler);

      expect(shared.error).toBeUndefined();
      expect(shared.writes).toEqual(direct.writes);
      expect(shared.source).toBe(direct.source);
    });
  }

  it("hands undo back unchanged when nothing else merged in", async () => {
    // An operation that went through the shared document alone is as undoable
    // as one that did not. Anything less would make collaboration cost the
    // author their undo stack.
    const direct = await applyOperation(deck(), SEPARATOR, OPERATIONS["setHeading"]!);
    const shared = await applyOperation(
      deck(),
      SEPARATOR,
      OPERATIONS["setHeading"]!,
      alone().reconciler,
    );

    expect(shared.undo).toEqual(direct.undo);
    expect(shared.undo.length).toBeGreaterThan(0);
  });

  it("takes an edit off the undo stack back through the same document", async () => {
    const room = alone();
    const forward = await applyOperation(deck(), SEPARATOR, OPERATIONS["setHeading"]!);
    const changed = deck().map((file) =>
      file.path === "/slides/0002.md" ? { ...file, source: forward.writes[0]!.source! } : file,
    );

    const back = await revertOperation(changed, SEPARATOR, forward.undo, room.reconciler);

    expect(back.writes[0]!.source).toBe(deck()[1]!.source);
  });

  it("refuses an operation naming a slide the deck does not have, and writes nothing", async () => {
    const result = await applyOperation(
      deck(),
      SEPARATOR,
      { op: "setHeading", slide: 40, text: "nowhere" },
      alone().reconciler,
    );

    expect(result.error).toBeDefined();
    expect(result.writes).toEqual([]);
  });
});

describe("what the shared document is for", () => {
  it("keeps a slide the author saved while an operation was being planned", async () => {
    // The reason this exists, and the reason it needs a CRDT rather than care.
    // Planning an operation means parsing a deck, which means an await, and the
    // author's own text editor saves a file during it. The operation's byte
    // offsets are then stale. Applying them literally would land the change in
    // the wrong place; moving them by guesswork would be a merge algorithm
    // written by hand.
    const room = alone();
    const pending = room.reconciler.begin("# One\n\n---\n\n# Two\n");

    // The author saves 0002.md. The watcher folds it in, mid-flight.
    room.reconciler.adopt("# One\n\n---\n\n# Two, and a new line\n");

    // The operation was planned against the deck as it was before that.
    const written = pending.settle("# One\n\n---\n\n# Two\n", "# First\n\n---\n\n# Two\n");

    expect(written).toBe("# First\n\n---\n\n# Two, and a new line\n");
  });

  it("keeps a slide the author saved above the one an operation changed", async () => {
    // The half a stale offset gets wrong. The disk change is *earlier* in the
    // document than the operation's, so every offset after it has moved.
    const room = alone();
    const pending = room.reconciler.begin("# One\n\n---\n\n# Two\n");

    room.reconciler.adopt("# One, expanded considerably\n\n---\n\n# Two\n");

    const written = pending.settle("# One\n\n---\n\n# Two\n", "# One\n\n---\n\n# Second\n");

    expect(written).toBe("# One, expanded considerably\n\n---\n\n# Second\n");
  });

  it("changes nothing at all when an operation asks for what the deck already says", async () => {
    // The same rule `EditBuilder` keeps on the Rust side: idempotence is a
    // property of the mechanism rather than of each operation.
    const shared = createSharedDeck("# One\n");

    expect(shared.adopt("# One\n")).toBe(false);
    expect(shared.text()).toBe("# One\n");
  });

  it("folds a file that changed on disk in as the smallest range that differs", () => {
    // Not the whole line: the shared tail is left alone, because a splice is
    // "change these bytes" and `ody.` did not change.
    const splice = spliceBetween("# One\n\nBody.\n", "# One\n\nA longer body.\n");

    expect(splice).toEqual({ at: 7, remove: 1, text: "A longer b" });
  });

  it("names no range when two readings of a deck are the same", () => {
    expect(spliceBetween("# One\n", "# One\n")).toBeNull();
  });

  it("never splits a character in half, however the two readings line up", () => {
    // An emoji is two UTF-16 units. A prefix that ended between them would put
    // half a character into the document, and a slide title with an emoji in it
    // is not unusual.
    for (const [before, after] of [
      ["a🎉b", "a🎉c"],
      ["a🎉b", "a🎈b"],
      ["🎉", "🎈"],
      ["x", "🎉"],
      ["🎉🎈", "🎉🎉🎈"],
    ]) {
      const splice = spliceBetween(before!, after!);
      const applied =
        splice === null
          ? before!
          : before!.slice(0, splice.at) + splice.text + before!.slice(splice.at + splice.remove);

      expect(applied).toBe(after);
      expect(/[\uD800-\uDBFF]$/.test(before!.slice(0, splice?.at ?? 0))).toBe(false);
    }
  });

  it("puts every byte of a deck written in Japanese back where it was", async () => {
    const japanese: DeckFile[] = [
      { path: "/slides/0001.md", label: "slides/0001.md", source: "# 速いデッキ\n" },
      { path: "/slides/0002.md", label: "slides/0002.md", source: "## 何を話すか\n\n- パーサー\n" },
    ];

    const direct = await applyOperation(japanese, SEPARATOR, {
      op: "setHeading",
      slide: 1,
      text: "何を話すか、正確に",
    });
    const shared = await applyOperation(
      japanese,
      SEPARATOR,
      { op: "setHeading", slide: 1, text: "何を話すか、正確に" },
      alone().reconciler,
    );

    expect(shared.writes).toEqual(direct.writes);
  });
});

describe("the roster", () => {
  it("calls the author you and everybody else a guest, in the order they arrived", () => {
    const roster = createRoster();
    roster.seen("a", { local: true, canEdit: true });
    roster.seen("b", { local: false, canEdit: false });
    roster.seen("c", { local: false, canEdit: true });

    expect(roster.viewers().map((viewer) => viewer.label)).toEqual(["you", "guest 2", "guest 3"]);
  });

  it("puts the author first however late they connected", () => {
    // A roster that reordered itself would be unreadable exactly when it
    // matters, which is while somebody else is typing.
    const roster = createRoster();
    roster.seen("b", { local: false, canEdit: false });
    roster.seen("a", { local: true, canEdit: true });

    expect(roster.viewers()[0]!.label).toBe("you");
  });

  it("says which slide each person is on, counting from zero as the deck does", () => {
    const roster = createRoster();
    roster.seen("a", { local: true, canEdit: true });
    roster.moved("a", { slide: 4 });

    expect(roster.viewers()[0]!.slide).toBe(4);
  });

  it("forgets somebody whose phone stopped answering rather than closed", () => {
    // A phone that walked out of range does not disconnect; it goes quiet. A
    // roster showing a co-presenter who left the building is worse than one
    // showing nobody.
    let now = 1_000;
    const roster = createRoster(() => now);
    roster.seen("gone", { local: false, canEdit: false });

    now += PRESENCE_TIMEOUT_MS + 1;

    expect(roster.viewers()).toEqual([]);
  });

  it("ignores a position from a seat nobody was given", () => {
    // The seat id is issued by the server on the stream. A viewer that made one
    // up would otherwise appear in everybody's roster.
    const roster = createRoster();
    roster.moved("invented", { slide: 3 });

    expect(roster.viewers()).toEqual([]);
  });
});

describe("the stream", () => {
  it("carries a deck source without the frame ending early on a newline", () => {
    // A blank line ends an event. A deck is nothing but newlines, so this is
    // the one mistake that looks like it works until the deck has two slides.
    const text = frame("state", { source: "# One\n\n---\n\n# Two\n" });
    const body = text.split("\n").filter((line) => line.startsWith("data: "));

    expect(body).toHaveLength(1);
    expect(text.endsWith("\n\n")).toBe(true);
    expect(text.startsWith("event: state\n")).toBe(true);
  });
});

describe("what a room does with a reconciler before anybody edits", () => {
  it("adopts the deck the files say before an operation is planned against it", async () => {
    const room = createRoom({ deckState: () => Promise.resolve({}) });

    expect(room.reconciler.begin("# One\n").settle("# One\n", "# Two\n")).toBe("# Two\n");
  });

  it("reports nobody connected until somebody joins the stream", () => {
    expect(createRoom({ deckState: () => Promise.resolve({}) }).viewers()).toEqual([]);
  });

  it("answers no route it does not own", async () => {
    const room = createRoom({ deckState: () => Promise.resolve({}) });
    const request = { url: "/__slidx/deck", method: "GET", socket: {} };
    const answered = await room.handle(request as never, {} as never, {
      grant: Grant.Write,
      local: true,
    });

    expect(answered).toBe(false);
  });
});
