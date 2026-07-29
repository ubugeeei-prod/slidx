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

  it("gives back the slide's Markdown, counted in bytes", async () => {
    // Byte offsets, in a language that counts in UTF-16 code units. A deck
    // written in Japanese diverges on the first character, and the slice that
    // gets it wrong lands an edit in the middle of a word.
    const session = createSession(fakeServer(deckOf("日本語のタイトル", "Two")));
    await session.open();

    expect(session.bodyOf(0)).toBe("# 日本語のタイトル");
    expect(session.bodyOf(1)).toBe("# Two");
  });

  it("tells its listeners once per change", async () => {
    const session = createSession(fakeServer());
    const seen: number[] = [];
    session.subscribe((state) => seen.push(state.slides.length));

    await session.open();

    expect(seen).toEqual([0, 3]);
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
  });
});
