/**
 * What the editor does with an answer from the dev server.
 *
 * The rules here are the ones an author feels: a press of undo takes back
 * something visible, a gesture that changed nothing does not cost one, and an
 * operation the deck refuses is a line of text rather than a crash.
 */

import { describe, expect, it } from "vite-plus/test";

import { createHistory } from "../src/history";
import { createSession } from "../src/session";
import { deckOf, fakeServer } from "./support";

describe("an editing session", () => {
  it("reads the deck it is opened on", async () => {
    const session = createSession(fakeServer());
    await session.open();

    expect(session.state().slides.map((slide) => slide.title)).toEqual(["One", "Two", "Three"]);
  });

  it("sends every change as an operation and never as text", async () => {
    const server = fakeServer();
    const session = createSession(server);
    await session.open();

    await session.run({ op: "setHeading", slide: 1, text: "Retitled" });

    expect(server.ops).toEqual([{ op: "setHeading", slide: 1, text: "Retitled" }]);
  });

  it("never sends a change through a view-only capability", async () => {
    const deck = deckOf("One", "Two");
    deck.access = { canEdit: false };
    const server = fakeServer(deck);
    const session = createSession(server);
    await session.open();

    await session.run({ op: "setHeading", slide: 0, text: "Retitled" });
    await session.undo();
    await session.redo();

    expect(session.state().canEdit).toBe(false);
    expect(server.ops).toEqual([]);
    expect(server.reverted).toEqual([]);
    expect(session.state().writing).toBe(false);
  });

  it("says when a change is still being written and when disk has answered", async () => {
    const server = fakeServer();
    let release: (() => void) | undefined;
    server.apply = () =>
      new Promise((resolve) => {
        release = () => resolve({ ...deckOf("One", "Two", "Three"), undo: [{ splice: 1 }] });
      });
    const session = createSession(server);
    await session.open();

    const request = session.run({ op: "setHeading", slide: 1, text: "Retitled" });
    expect(session.state().writing).toBe(true);

    release!();
    await request;
    expect(session.state().writing).toBe(false);
    expect(session.state().canUndo).toBe(true);
  });

  it("gives back the slide's Markdown, counted in bytes", async () => {
    // Byte offsets, in a language that counts in UTF-16 code units. A deck
    // written in Japanese diverges on the first character, and the slice that
    // gets it wrong lands an edit in the middle of a word.
    const session = createSession(fakeServer(deckOf("日本語のタイトル", "Two")));
    await session.open();

    expect(session.bodyOf(0)).toBe("# 日本語のタイトル");
    expect(session.bodyOf(1)).toBe("# Two");
    expect(session.contentOf(0)).toBe("# 日本語のタイトル");
    expect(session.contentOf(1)).toBe("# Two");
  });

  it("lands on an operation's result only after the operation succeeds", async () => {
    const server = fakeServer();
    server.answer = deckOf("One", "Two", "Two copy", "Three");
    const session = createSession(server);
    await session.open();
    session.select({ slide: 1, block: 2 });

    await session.run(
      { op: "duplicateSlide", slide: 1, after: 1 },
      { slide: 2, block: undefined, range: undefined, text: undefined },
    );

    expect(session.state().selection).toEqual({ slide: 2 });
  });

  it("tells its listeners once per change", async () => {
    const session = createSession(fakeServer());
    const seen: number[] = [];
    session.subscribe((state) => seen.push(state.slides.length));

    await session.open();

    expect(seen).toEqual([0, 3]);
  });

  it("keeps a whole block selected across the operation that styles it", async () => {
    const deck = deckOf("One");
    deck.spans[0]!.blocks = [{ span: { start: 0, end: 5 } }];
    const session = createSession(fakeServer(deck));
    await session.open();
    session.select({ block: 0 });

    await session.run({
      op: "setBlockStyle",
      slide: 0,
      block: 0,
      property: "color",
      value: "#ff3366",
    });

    expect(session.state().selection).toEqual({ slide: 0, block: 0 });
  });

  it("keeps the same occurrence of selected words across a style rewrite", async () => {
    const initial = deckOf("fast then fast");
    const styled = deckOf("fast then [fast]{.accent}");
    const server = fakeServer(initial);
    server.answer = styled;
    const session = createSession(server);
    await session.open();
    session.select({ text: "fast", range: { start: 12, end: 16 } });

    await session.run({
      op: "addMark",
      slide: 0,
      range: { start: 12, end: 16 },
      attributes: { classes: ["accent"] },
    });

    expect(session.state().selection).toEqual({
      slide: 0,
      text: "fast",
      range: { start: 13, end: 17 },
    });
  });
});

describe("undo", () => {
  it("takes back the operation that was just applied", async () => {
    const server = fakeServer();
    const session = createSession(server);
    await session.open();

    await session.run({ op: "setHeading", slide: 0, text: "Retitled" });
    await session.undo();

    expect(server.reverted).toEqual([[{ splice: 1 }]]);
    expect(session.state().canUndo).toBe(false);
    expect(session.state().canRedo).toBe(true);
  });

  it("puts back what it took away", async () => {
    const server = fakeServer();
    const session = createSession(server);
    await session.open();

    await session.run({ op: "removeSlide", slide: 1 });
    await session.undo();
    await session.redo();

    expect(server.reverted).toEqual([[{ splice: 1 }], [{ splice: -1 }]]);
    expect(session.state().canRedo).toBe(false);
  });

  it("spends no step on a gesture that changed nothing", async () => {
    // Editors emit these constantly: a drag that ends where it started, a
    // field committed without being touched. Each would otherwise cost a press.
    const server = fakeServer();
    server.answer = { undo: [] };
    const session = createSession(server);
    await session.open();

    await session.run({ op: "moveSlide", slide: 1, to: 1 });

    expect(session.state().canUndo).toBe(false);
  });

  it("drops what was ahead once the author edits again", async () => {
    const server = fakeServer();
    const session = createSession(server);
    await session.open();

    await session.run({ op: "setHeading", slide: 0, text: "One more" });
    await session.undo();
    await session.run({ op: "setHeading", slide: 0, text: "Somewhere else" });

    expect(session.state().canRedo).toBe(false);
  });

  it("stops growing once the stack is deep enough to be useless", async () => {
    const history = createHistory(2);
    const session = createSession(fakeServer(), history);
    await session.open();

    for (let index = 0; index < 5; index += 1) {
      await session.run({ op: "setHeading", slide: 0, text: `Take ${index}` });
    }

    await session.undo();
    await session.undo();

    expect(session.state().canUndo).toBe(false);
  });
});

describe("an operation the deck refuses", () => {
  it("is a line of text rather than a thrown error", async () => {
    const server = fakeServer();
    server.answer = { error: { error: "noSuchSlide", slide: 9 } };
    const session = createSession(server);
    await session.open();

    await session.run({ op: "removeSlide", slide: 9 });

    expect(session.state().refusal).toEqual({ error: "noSuchSlide", slide: 9 });
    expect(session.state().canUndo).toBe(false);
  });

  it("does not move to a proposed result", async () => {
    const server = fakeServer();
    server.answer = { error: { error: "noSuchSlide", slide: 9 } };
    const session = createSession(server);
    await session.open();
    session.select({ slide: 1 });

    await session.run(
      { op: "duplicateSlide", slide: 9, after: 1 },
      { slide: 2, block: undefined, range: undefined, text: undefined },
    );

    expect(session.state().selection).toEqual({ slide: 1 });
  });

  it("does not put a refusal on the undo stack", async () => {
    const server = fakeServer();
    server.answer = { error: { error: "noSuchSlide", slide: 9 }, undo: [{ splice: 1 }] };
    const session = createSession(server);
    await session.open();

    await session.run({ op: "removeSlide", slide: 9 });

    expect(session.state().canUndo).toBe(false);
  });
});

describe("a deck the server cannot write", () => {
  it("is reported and does not take the editor down with it", async () => {
    const server = fakeServer();
    server.apply = async () => {
      throw new Error("A slide runs past the end of slides/0001.md.");
    };
    const session = createSession(server);
    await session.open();

    await session.run({ op: "setHeading", slide: 0, text: "x" });

    expect(session.state().problem).toContain("slides/0001.md");
    expect(session.state().writing).toBe(false);
  });
});

describe("a change that has not been made yet", () => {
  it("is held next to the deck's own findings and cleared by the change itself", async () => {
    // A block being dragged has a landing before it has a line in the file.
    // Once it lands, what was foreseen is either in `diagnostics` or was never
    // true, so it cannot survive the operation.
    const session = createSession(fakeServer());
    await session.open();

    session.foresee([{ severity: "error", code: "overflow/clipped", message: "loses its edge" }]);
    expect(session.state().foreseen).toHaveLength(1);

    await session.run({ op: "moveBlock", slide: 0, block: 0, to: 1, region: "right" });
    expect(session.state().foreseen).toBeUndefined();
  });

  it("does not wake every surface up to say nothing is wrong twice", async () => {
    // A drag calls this whenever the pointer leaves a region, and a render of
    // the whole editor per event is a drag that stutters.
    const session = createSession(fakeServer());
    await session.open();

    let renders = 0;
    session.subscribe(() => (renders += 1));
    session.foresee([]);
    session.foresee([]);

    expect(renders).toBe(1);
  });
});

describe("moving with somebody else", () => {
  /** One connected viewer, as the dev server reports them. */
  const guest = (id: string, slide: number) => ({
    id,
    label: id,
    local: false,
    canEdit: true,
    slide,
  });

  it("follows nobody until it is asked to", async () => {
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 2)]);

    expect(session.state().following).toBeUndefined();
    expect(session.state().selection.slide).toBe(0);
  });

  it("arrives where they already are rather than waiting for their next move", async () => {
    // Minutes can pass between somebody moving. An editor that only caught up
    // on their next step would look broken for all of them.
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 2)]);
    session.follow("b");

    expect(session.state().selection.slide).toBe(2);
  });

  it("moves when they move", async () => {
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 0)]);
    session.follow("b");
    session.saw([guest("b", 2)]);

    expect(session.state().selection.slide).toBe(2);
  });

  it("stops the moment the author selects anything themselves", async () => {
    // Following that survived a deliberate click would drag the author off the
    // slide they just chose, at whatever moment somebody else happened to move.
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 0)]);
    session.follow("b");
    session.select({ slide: 1 });
    session.saw([guest("b", 2)]);

    expect(session.state().following).toBeUndefined();
    expect(session.state().selection.slide).toBe(1);
  });

  it("stops when the person being followed closes their tab", async () => {
    // Otherwise the editor simply stops moving, with nothing on screen saying
    // why it ever was.
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 1)]);
    session.follow("b");
    session.saw([]);

    expect(session.state().following).toBeUndefined();
    expect(session.state().selection.slide).toBe(1);
  });

  it("follows nobody when asked for a seat that is not in the roster", async () => {
    const session = createSession(fakeServer());
    await session.open();
    session.follow("nobody");

    expect(session.state().following).toBeUndefined();
    expect(session.state().selection.slide).toBe(0);
  });

  it("stops when asked to follow nobody", async () => {
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 1)]);
    session.follow("b");
    session.follow(undefined);
    session.saw([guest("b", 2)]);

    expect(session.state().following).toBeUndefined();
    expect(session.state().selection.slide).toBe(1);
  });

  it("leaves the selection alone when the roster changes around somebody who has not moved", async () => {
    const session = createSession(fakeServer());
    await session.open();
    session.saw([guest("b", 1)]);
    session.follow("b");
    session.select({ block: 2 });
    session.follow("b");
    session.saw([guest("b", 1), guest("c", 0)]);

    expect(session.state().selection.block).toBe(2);
  });
});
