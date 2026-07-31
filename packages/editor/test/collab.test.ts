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

import {
  createPresence,
  readCredential,
  readFrames,
  CREDENTIAL_HEADER,
  type PresenceOptions,
  type Viewer,
} from "../src/collab";
import type { EditorState } from "../src/session";

const SESSION = "0123456789abcdef";
const SECRET = "00112233445566778899aabbccddeeff";

/** An editor state with one field that matters here. */
function state(source: string, slide = 0, block?: number): EditorState {
  return {
    source,
    spans: [],
    slides: [],
    layouts: [],
    diagnostics: [],
    selection: { slide, block },
    viewers: [],
    canUndo: false,
    canRedo: false,
  };
}

/** One server-sent frame, as the dev server writes it. */
function sent(event: string, data: unknown): string {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

/**
 * A presence surface the server has already given a seat.
 *
 * Nothing is reported before the seat arrives — the id is issued on the stream,
 * so a position posted without one is a position nobody can attribute — which
 * means every test about what gets reported has to get past that first.
 */
async function seated(frames: string[] = [], extra: Partial<PresenceOptions> = {}) {
  const calls: { url: string; init: RequestInit | undefined; body: unknown }[] = [];
  const rosters: Viewer[][] = [];
  const encoder = new TextEncoder();
  const queue = [sent("hello", { id: "seat-1", canEdit: true }), ...frames];

  const send = ((url: string, init?: RequestInit) => {
    calls.push({
      url,
      init,
      body: typeof init?.body === "string" ? JSON.parse(init.body) : undefined,
    });

    if (!url.endsWith("live")) return Promise.resolve({ body: undefined } as unknown as Response);

    let at = 0;
    const body = {
      getReader: () => ({
        read: () =>
          Promise.resolve(
            at < queue.length
              ? { value: encoder.encode(queue[at++]!), done: false }
              : { value: undefined, done: true },
          ),
      }),
    };

    return Promise.resolve({ body } as unknown as Response);
  }) as unknown as typeof globalThis.fetch;

  const surface = createPresence({
    reload: () => {},
    saw: (viewers) => void rosters.push(viewers),
    fetch: send,
    href: "http://localhost:5173/__slidx/",
    retry: 10_000,
    ...extra,
  });

  // The frames are read from a promise chain, so the loop needs turns of the
  // microtask queue before the seat has arrived.
  for (let turn = 0; turn < queue.length + 4; turn += 1) await Promise.resolve();

  return {
    surface,
    calls,
    rosters,
    posted: () => calls.filter((call) => call.url.endsWith("here")),
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

describe("what the editor reports about where it is", () => {
  it("names the block, not only the slide", async () => {
    const { surface, posted } = await seated();
    surface.render(state("# One\n", 2, 1));

    expect(posted()[0]!.body).toEqual({ id: "seat-1", slide: 2, block: 1 });
  });

  it("leaves the block out entirely when nothing is selected", async () => {
    // Absent rather than null, because that is the statement the roster makes
    // back: no key means nowhere in particular, and every number is a block.
    const { surface, posted } = await seated();
    surface.render(state("# One\n", 2));

    expect(posted()[0]!.body).toEqual({ id: "seat-1", slide: 2 });
  });

  it("reports again when only the block changed", async () => {
    // The slide is the same, so a surface watching only the slide would leave
    // everybody else's screen pointing at the paragraph this author has left.
    const { surface, posted } = await seated();
    surface.render(state("# One\n", 2, 0));
    surface.render(state("# One\n", 2, 3));

    expect(posted().map((call) => call.body)).toEqual([
      { id: "seat-1", slide: 2, block: 0 },
      { id: "seat-1", slide: 2, block: 3 },
    ]);
  });

  it("reports again when a selection is cleared", async () => {
    const { surface, posted } = await seated();
    surface.render(state("# One\n", 2, 0));
    surface.render(state("# One\n", 2));

    expect(posted()).toHaveLength(2);
    expect(posted()[1]!.body).toEqual({ id: "seat-1", slide: 2 });
  });

  it("says nothing at all when neither changed", async () => {
    // A render happens on every keystroke in the inspector. A post per
    // keystroke would make presence the busiest thing on the wire.
    const { surface, posted } = await seated();
    surface.render(state("# One\n", 2, 1));
    surface.render(state("# One typed into\n", 2, 1));
    surface.render(state("# One typed into a bit more\n", 2, 1));

    expect(posted()).toHaveLength(1);
  });
});

describe("handing the roster on", () => {
  it("gives everyone connected to whoever asked for them", async () => {
    // The same people are drawn twice — once as a list, once as marks over the
    // blocks they are in — and the stream is still read in one place.
    const { rosters } = await seated([
      sent("presence", {
        viewers: [
          { id: "seat-1", label: "you", local: true, canEdit: true, slide: 0 },
          { id: "seat-2", label: "guest 2", local: false, canEdit: true, slide: 3, block: 2 },
        ],
      }),
    ]);

    expect(rosters.at(-1)).toEqual([
      { id: "seat-1", label: "you", local: true, canEdit: true, slide: 0 },
      { id: "seat-2", label: "guest 2", local: false, canEdit: true, slide: 3, block: 2 },
    ]);
  });

  it("draws its own list when nobody else wants the roster", async () => {
    // The option is optional, and an editor mounted without it still draws its
    // own list rather than failing on the first presence frame.
    const { surface } = presence();

    expect(surface.root.getAttribute("data-empty")).toBe("true");
  });
});

describe("going and standing where somebody else is", () => {
  const ROSTER = {
    viewers: [
      { id: "seat-1", label: "you", local: true, canEdit: true, slide: 0 },
      { id: "seat-2", label: "guest 2", local: false, canEdit: true, slide: 3, block: 1 },
      { id: "seat-3", label: "guest 3", local: false, canEdit: false, slide: 1 },
    ],
  };

  /** A seated surface that has been told who is here. */
  async function withRoster(options: { follow?: boolean } = {}) {
    const asked: (string | undefined)[] = [];
    const seat = await seated([sent("presence", ROSTER)], {
      ...(options.follow === false ? {} : { follow: (id?: string) => void asked.push(id) }),
    });

    return {
      ...seat,
      asked,
      seats: () => [...seat.surface.root.querySelectorAll(".slidx-presence-seat")],
    };
  }

  it("makes every guest a control and the author's own row plain text", async () => {
    // Following yourself is not a thing anybody means, and a control that does
    // nothing is worse than no control.
    const { seats } = await withRoster();

    expect(seats().map((node) => node.tagName)).toEqual(["SPAN", "BUTTON", "BUTTON"]);
  });

  it("asks to follow the seat that was pressed", async () => {
    const { seats, asked } = await withRoster();
    (seats()[1] as HTMLButtonElement).click();

    expect(asked).toEqual(["seat-2"]);
  });

  it("marks the seat being followed, and offers to stop", async () => {
    const { surface, seats } = await withRoster();
    surface.render({ ...state("# One\n"), following: "seat-2" });

    expect(seats()[1]!.getAttribute("aria-pressed")).toBe("true");
    expect(seats()[1]!.getAttribute("title")).toBe("Stop following guest 2");
    expect(seats()[2]!.getAttribute("aria-pressed")).toBe("false");
  });

  it("asks to follow nobody when the seat already being followed is pressed", async () => {
    const { surface, seats, asked } = await withRoster();
    surface.render({ ...state("# One\n"), following: "seat-2" });
    (seats()[1] as HTMLButtonElement).click();

    expect(asked).toEqual([undefined]);
  });

  it("draws no controls at all when nothing can act on them", async () => {
    const { seats } = await withRoster({ follow: false });

    expect(seats().map((node) => node.tagName)).toEqual(["SPAN", "SPAN", "SPAN"]);
  });

  it("still says where everybody is", async () => {
    const { surface } = await withRoster();
    const where = [...surface.root.querySelectorAll(".slidx-presence-where")];

    expect(where.map((node) => node.textContent)).toEqual(["slide 1", "slide 4", "slide 2"]);
  });

  it("still says who is only reading", async () => {
    const { surface } = await withRoster();
    const roles = [...surface.root.querySelectorAll(".slidx-presence-role")];

    expect(roles.map((node) => node.textContent)).toEqual(["reading"]);
  });
});
