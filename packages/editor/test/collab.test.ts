/**
 * The presence surface, and the one thing it must not become.
 *
 * `history.ts` refuses to hold a second copy of the document, and collaboration
 * is the feature that would justify one. So the interesting assertions here are
 * negative: nothing is merged in the browser, an announcement carrying bytes the
 * editor already has is ignored, and the editor's model of the deck still comes
 * from the one route that has always supplied it.
 */

import { describe, expect, it } from "vite-plus/test";

import { createPresence, readCredential, readFrames, CREDENTIAL_HEADER } from "../src/collab";
import type { EditorState } from "../src/session";

const SESSION = "0123456789abcdef";
const SECRET = "00112233445566778899aabbccddeeff";

/** An editor state with one field that matters here. */
function state(source: string, slide = 0): EditorState {
  return {
    source,
    spans: [],
    slides: [],
    diagnostics: [],
    selection: { slide },
    canUndo: false,
    canRedo: false,
  };
}

/** A `fetch` that records what it was asked and never answers a stream. */
function recording() {
  const calls: { url: string; init: RequestInit | undefined }[] = [];

  const send = ((url: string, init?: RequestInit) => {
    calls.push({ url, init });
    // No body, so the surface stops listening rather than looping in a test.
    return Promise.resolve({ body: undefined } as unknown as Response);
  }) as unknown as typeof globalThis.fetch;

  return { calls, send };
}

function presence(options: { href?: string; reload?: () => void } = {}) {
  const { calls, send } = recording();
  const reloads: number[] = [];

  const surface = createPresence({
    reload: options.reload ?? (() => void reloads.push(1)),
    fetch: send,
    href: options.href ?? "http://localhost:5173/__slidx/",
    retry: 10_000,
  });

  return { surface, calls, reloads };
}

describe("reading a share credential", () => {
  it("takes it from the fragment", () => {
    expect(readCredential(`http://host/__slidx/#s=${SESSION}.${SECRET}`)).toBe(
      `${SESSION}.${SECRET}`,
    );
  });

  it("finds nothing in a URL that put it in the query", () => {
    // A URL carrying the secret in its query has already been written to a log,
    // and honouring it would make replaying that log enough to join.
    expect(readCredential(`http://host/__slidx/?s=${SESSION}.${SECRET}`)).toBeUndefined();
  });

  it("finds nothing when there is nothing to find", () => {
    expect(readCredential("http://host/__slidx/")).toBeUndefined();
    expect(readCredential("http://host/__slidx/#other=1")).toBeUndefined();
  });

  it("coexists with other fragment parameters", () => {
    expect(readCredential(`http://host/#slide=3&s=${SESSION}.${SECRET}`)).toBe(
      `${SESSION}.${SECRET}`,
    );
  });
});

describe("reading frames off a stream", () => {
  it("keeps a half-arrived frame in the buffer rather than acting on it", () => {
    // A read lands wherever the network split it. Acting on half a frame is how
    // a deck announcement becomes a parse error.
    const read = readFrames('event: state\ndata: {"source":"# On');

    expect(read.frames).toEqual([]);
    expect(read.rest).toBe('event: state\ndata: {"source":"# On');
  });

  it("reads two frames that arrived in one chunk", () => {
    const read = readFrames(
      'event: hello\ndata: {"id":"a"}\n\nevent: presence\ndata: {"viewers":[]}\n\n',
    );

    expect(read.frames.map((frame) => frame.event)).toEqual(["hello", "presence"]);
    expect(read.rest).toBe("");
  });

  it("ignores a keep-alive comment, which carries no data at all", () => {
    expect(readFrames(": keep-alive\n\n").frames).toEqual([]);
  });

  it("carries a deck source through unharmed, newlines and all", () => {
    const read = readFrames(
      `event: state\ndata: ${JSON.stringify({ source: "# A\n\n# B\n" })}\n\n`,
    );

    expect(read.frames[0]!.data["source"]).toBe("# A\n\n# B\n");
  });
});

describe("the presence surface", () => {
  it("presents the share credential in a header rather than a query", () => {
    const { calls } = presence({ href: `http://host/__slidx/#s=${SESSION}.${SECRET}` });

    expect(calls[0]!.url).toBe("/__slidx/live");
    expect(calls[0]!.url).not.toContain(SECRET);
    expect((calls[0]!.init?.headers as Record<string, string>)[CREDENTIAL_HEADER]).toBe(
      `${SESSION}.${SECRET}`,
    );
  });

  it("sends no credential when the page was opened without one", () => {
    const { calls } = presence();

    expect(calls[0]!.init?.headers).toEqual({});
  });

  it("shows nothing at all while the author is alone", () => {
    // An author working alone should not have to look at a panel telling them
    // they are alone.
    const { surface } = presence();

    expect(surface.root.getAttribute("data-empty")).toBe("true");
  });

  it("stops listening when the editor that mounted it is destroyed", async () => {
    // The reconnect is deliberately unbounded, so that a dev server the author
    // restarted is picked back up without a reload. That is exactly why a
    // destroyed editor has to say so: otherwise a remounted editor leaves the
    // previous one's loop reconnecting behind it, asking a session that no
    // longer exists to re-read the deck.
    // A dev server that is not answering, which is the state the loop was
    // written for: it fails, waits, and tries again until somebody stops it.
    const attempts: string[] = [];
    const send = ((url: string) => {
      attempts.push(url);
      return Promise.reject(new Error("no dev server"));
    }) as unknown as typeof globalThis.fetch;

    const surface = createPresence({
      reload: () => {},
      fetch: send,
      href: "http://localhost:5173/__slidx/",
      retry: 1,
    });

    surface.destroy?.();
    const seen = attempts.length;
    await new Promise((wake) => setTimeout(wake, 25));

    expect(attempts.length).toBe(seen);
  });

  it("says nothing about where it is until the server has given it a seat", () => {
    // The seat id is issued on the stream. Posting without one would be a
    // position nobody can attribute.
    const { surface, calls } = presence();
    surface.render(state("# One\n", 2));

    expect(calls.filter((call) => call.url.endsWith("here"))).toEqual([]);
  });
});
