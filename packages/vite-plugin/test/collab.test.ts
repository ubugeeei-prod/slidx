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

import { Readable } from "node:stream";

import { describe, expect, it } from "vite-plus/test";

import type { EditOp } from "@slidxjs/editor";

import { createSharedDeck, spliceBetween } from "../src/collab";
import { createRoster, PRESENCE_TIMEOUT_MS } from "../src/collab/presence";
import { createRoom, type Room } from "../src/collab/room";
import { createStream, frame } from "../src/collab/stream";
import { applyOperation, revertOperation, type DeckFile } from "../src/edit";
import { Grant } from "../src/share";

const SEPARATOR = "---";
const SECOND_BODY =
  "## What we will cover\n\n*  Why the parser matters\n*  What the linter catches\n";
const THIRD_BODY = "## Numbers\n\nLatency 🎉 dropped to [120ms]{#result .accent}.\n";

/** A half-open byte span for words in a slide body, including non-ASCII text before them. */
function spanOf(body: string, words: string) {
  const start = body.indexOf(words);
  if (start < 0) throw new Error(`${JSON.stringify(words)} is not in the fixture body`);
  const bytes = new TextEncoder();

  return {
    start: bytes.encode(body.slice(0, start)).byteLength,
    end: bytes.encode(body.slice(0, start + words.length)).byteLength,
  };
}

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
      source:
        '---\nlayout: split\nsteps:\n  - reveal: ".parser"\n  - hide: ".lint"\n---\n\n' +
        SECOND_BODY,
    },
    {
      path: "/slides/0003.md",
      label: "slides/0003.md",
      source: THIRD_BODY,
    },
    {
      path: "/slides/0004.md",
      label: "slides/0004.md",
      source: "---\nautoSteps: list\n---\n\n## Generated\n\n- one\n- two\n",
    },
  ];
}

/** A room with nothing else connected, which is the case under test. */
function alone() {
  return createRoom({ deckState: () => Promise.resolve({}) });
}

/** Every operation the editor can send, in the shapes it sends them. */
const OPERATIONS = {
  setHeading: { op: "setHeading", slide: 1, text: "What we will cover today" },
  setBody: { op: "setBody", slide: 2, body: "## Numbers\n\nLatency dropped to 38ms.\n" },
  setText: { op: "setText", slide: 2, range: spanOf(THIRD_BODY, "120ms"), text: "38ms" },
  setField: { op: "setField", slide: 1, key: "budget", value: "90s" },
  setStyle: { op: "setStyle", slide: 1, property: "layout", value: "aside" },
  setBlockStyle: {
    op: "setBlockStyle",
    slide: 1,
    block: 1,
    property: "x",
    value: "12.5%",
  },
  setNotes: { op: "setNotes", slide: 0, notes: "Open with the outcome." },
  insertSlide: { op: "insertSlide", at: 1, body: "## Inserted\n" },
  duplicateSlide: { op: "duplicateSlide", slide: 1 },
  duplicateBlock: { op: "duplicateBlock", slide: 1, block: 1 },
  removeSlide: { op: "removeSlide", slide: 1 },
  moveSlide: { op: "moveSlide", slide: 2, to: 0 },
  addMark: {
    op: "addMark",
    slide: 1,
    range: spanOf(SECOND_BODY, "Why the parser matters"),
    attributes: { key: "parser", classes: ["accent"] },
  },
  setMark: {
    op: "setMark",
    slide: 2,
    mark: "result",
    attributes: { key: "result", classes: ["hero"], properties: { color: "brand" } },
  },
  removeMark: { op: "removeMark", slide: 2, mark: "result" },
  setBlockAttributes: {
    op: "setBlockAttributes",
    slide: 1,
    block: 1,
    attributes: { key: "agenda", classes: ["accent"], properties: { align: "center" } },
  },
  setBlockWidth: { op: "setBlockWidth", slide: 1, block: 1, width: "half" },
  moveBlock: { op: "moveBlock", slide: 1, block: 1, to: 0, region: "right" },
  insertMedia: {
    op: "insertMedia",
    slide: 1,
    at: 2,
    kind: "image",
    src: "assets/chart.png",
    alt: "Performance chart",
    region: "right",
  },
  addStep: {
    op: "addStep",
    slide: 1,
    at: 1,
    action: { emphasize: { target: ".parser", options: { duration: 500 } } },
  },
  removeStep: { op: "removeStep", slide: 1, index: 0 },
  moveStep: { op: "moveStep", slide: 1, from: 0, to: 1 },
  setStep: {
    op: "setStep",
    slide: 1,
    index: 0,
    action: { emphasize: { target: ".parser", options: { preset: "pulse" } } },
  },
  adoptSteps: { op: "adoptSteps", slide: 3 },
} satisfies Record<EditOp["op"], EditOp>;

describe("a shared document under the splice", () => {
  for (const [name, op] of Object.entries(OPERATIONS)) {
    it(`writes the same bytes for ${name} with one editor as it would with none`, async () => {
      // The property in full. Not "an equivalent document" and not "the same
      // rendering" — the same files, byte for byte, including the author's `*`
      // bullets and the three spaces after their `#`.
      const direct = await applyOperation(deck(), SEPARATOR, op);
      const shared = await applyOperation(deck(), SEPARATOR, op, alone().reconciler);

      expect(direct.error).toBeUndefined();
      expect(shared.error).toBeUndefined();
      expect(shared.writes).toEqual(direct.writes);
      expect(shared.source).toBe(direct.source);
    });
  }

  for (const [name, op] of Object.entries(OPERATIONS)) {
    it(`keeps ${name} undoable and restores every file byte through the shared document`, async () => {
      // Every canvas gesture must remain one undoable gesture with collaboration
      // enabled, including operations that delete or rewrite more than one file.
      const before = deck();
      const room = alone();
      const direct = await applyOperation(before, SEPARATOR, op);
      const shared = await applyOperation(before, SEPARATOR, op, room.reconciler);

      expect(shared.error).toBeUndefined();
      expect(shared.undo).toEqual(direct.undo);
      expect(shared.undo.length).toBeGreaterThan(0);

      const changed = applied(before, shared.writes);
      const back = await revertOperation(changed, SEPARATOR, shared.undo, room.reconciler);

      expect(back.error).toBeUndefined();
      expect(applied(changed, back.writes)).toEqual(before);
    });
  }

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

/**
 * The file set with a write plan applied, preserving deleted files as the
 * empty positions an undo can put back.
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

  it("says which block on it they have selected", () => {
    const roster = createRoster();
    roster.seen("a", { local: false, canEdit: true });
    roster.moved("a", { slide: 4, block: 2 });

    expect(roster.viewers()[0]!.block).toBe(2);
  });

  it("says nothing at all about a viewer who has selected nothing", () => {
    // Absent rather than zero, because zero is a block. Anything that draws
    // this has to tell "in the first paragraph" from "nowhere in particular",
    // and a number meaning both is how the second lands on somebody's title.
    const roster = createRoster();
    roster.seen("a", { local: false, canEdit: true });
    roster.moved("a", { slide: 4 });

    expect(roster.viewers()[0]!.block).toBeUndefined();
    expect("block" in roster.viewers()[0]!).toBe(false);
  });

  it("forgets the block when the viewer deselects, rather than leaving the last one", () => {
    const roster = createRoster();
    roster.seen("a", { local: false, canEdit: true });
    roster.moved("a", { slide: 4, block: 2 });
    roster.moved("a", { slide: 4 });

    expect(roster.viewers()[0]!.block).toBeUndefined();
  });

  for (const said of [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
    it(`refuses ${said} as a block, because a share link is held by somebody else`, () => {
      // This is one of the few places in the dev server where the input is not
      // the author's own. Rounding nonsense into a block would put another
      // person's name on a paragraph they have never seen.
      const roster = createRoster();
      roster.seen("a", { local: false, canEdit: true });
      roster.moved("a", { slide: 0, block: said });

      expect(roster.viewers()[0]!.block).toBeUndefined();
    });
  }
});

describe("the stream", () => {
  it("refreshes the roster on a keep-alive that reached the browser", () => {
    // A viewer who never changes slides sends nothing at all, and would age out
    // of the roster while their connection is very much open — the common case
    // while somebody is talking. The heartbeat is what says they are still here,
    // which is what `presence.ts` claims and what this pins.
    const stream = createStream(1);
    const beats: number[] = [];
    const held = { write: () => true, setHeader: () => {}, end: () => {}, on: () => {} };

    stream.join("a", held as never, () => void beats.push(1));

    return new Promise<void>((done) =>
      setTimeout(() => {
        stream.closeAll();
        expect(beats.length).toBeGreaterThan(0);
        done();
      }, 20),
    );
  });

  it("does not call a viewer present on a keep-alive that failed to send", () => {
    // The delivery is the evidence. A write to a browser that has gone away
    // must not be what keeps it in everybody's roster.
    const stream = createStream(1);
    const beats: number[] = [];
    const gone = {
      write: () => {
        throw new Error("socket closed");
      },
      setHeader: () => {},
      end: () => {},
      on: () => {},
    };

    stream.join("a", gone as never, () => void beats.push(1));

    return new Promise<void>((done) =>
      setTimeout(() => {
        stream.closeAll();
        expect(beats).toEqual([]);
        done();
      }, 20),
    );
  });

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

/** Reports a position the way a browser does, over the route it uses. */
async function post(room: Room, said: Record<string, unknown>): Promise<void> {
  const request = Readable.from([Buffer.from(JSON.stringify(said))]) as unknown as Record<
    string,
    unknown
  >;
  request["url"] = "/__slidx/here";
  request["method"] = "POST";

  const answered = await room.handle(request as never, { end: () => {} } as never, {
    grant: Grant.Write,
    local: false,
  });

  expect(answered).toBe(true);
}

describe("what a room does with a reconciler before anybody edits", () => {
  it("adopts the deck the files say before an operation is planned against it", async () => {
    const room = createRoom({ deckState: () => Promise.resolve({}) });

    expect(room.reconciler.begin("# One\n").settle("# One\n", "# Two\n")).toBe("# Two\n");
  });

  it("reports nobody connected until somebody joins the stream", () => {
    expect(createRoom({ deckState: () => Promise.resolve({}) }).viewers()).toEqual([]);
  });

  it("reads the deck before turning the response into a stream", async () => {
    // Joining commits event-stream headers. A deck that cannot be read after
    // that leaves the caller unable to answer at all, because its error reply
    // would be a second set of headers on the same response. So the failure has
    // to arrive while the response is still an ordinary one.
    const room = createRoom({ deckState: () => Promise.reject(new Error("unreadable")) });
    const headers: string[] = [];
    const response = {
      setHeader: (name: string) => void headers.push(name),
      flushHeaders: () => {},
      write: () => true,
      end: () => {},
      on: () => {},
    };

    const request = { url: "/__slidx/live", method: "GET", socket: {} };
    const answering = room.handle(request as never, response as never, {
      grant: Grant.Write,
      local: true,
    });

    await expect(answering).rejects.toThrow("unreadable");
    expect(headers).toEqual([]);
  });

  it("carries the block a viewer reported through to the roster", async () => {
    // The block is what the canvas draws a co-presenter's name on, so a route
    // that dropped it would leave the roster saying "slide 4" forever while
    // every mark on the slide stayed empty.
    const roster = createRoster();
    roster.seen("seat", { local: false, canEdit: true });
    const room = createRoom({ deckState: () => Promise.resolve({}), roster });

    await post(room, { id: "seat", slide: 4, block: 3 });

    expect(roster.viewers()[0]).toMatchObject({ slide: 4, block: 3 });
  });

  it("carries a report with no block as no block", async () => {
    const roster = createRoster();
    roster.seen("seat", { local: false, canEdit: true });
    const room = createRoom({ deckState: () => Promise.resolve({}), roster });

    await post(room, { id: "seat", slide: 4, block: 3 });
    await post(room, { id: "seat", slide: 5 });

    expect(roster.viewers()[0]!.block).toBeUndefined();
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
