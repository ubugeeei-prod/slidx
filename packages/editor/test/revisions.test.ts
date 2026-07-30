/**
 * The history panel, driven without a server.
 *
 * What it has to get right is mostly about absence: a deck outside a
 * repository, a repository with no commits, a commit that touched no slide,
 * and a revision that stopped existing between reading the log and clicking a
 * row. None of those may look like a broken editor.
 */

import { beforeEach, describe, expect, it } from "vite-plus/test";

import {
  ago,
  createRevisions,
  type RestoreAnswer,
  type RevisionChange,
  type RevisionList,
  type RevisionsClient,
} from "../src/revisions";

interface Fake extends RevisionsClient {
  asked: string[];
  restored: string[];
  list_: RevisionList;
  change: RevisionChange | null;
  answer: RestoreAnswer;
}

function fakeHistory(list: Partial<RevisionList> = {}): Fake {
  const fake: Fake = {
    asked: [],
    restored: [],
    list_: { available: true, commits: [], ...list },
    change: { first: false, slides: 3, subject: 'Add "What it cost"', changes: [] },
    answer: { restored: "a".repeat(40), previous: "c".repeat(40) },

    list: async () => fake.list_,
    changeAt: async (rev) => {
      fake.asked.push(rev);
      return fake.change;
    },
    restore: async (rev) => {
      fake.restored.push(rev);
      return fake.answer;
    },
  };

  return fake;
}

const COMMITS = [
  {
    rev: "a".repeat(40),
    author: "The Author",
    date: "2026-07-28T09:00:00Z",
    subject: "rework the middle",
  },
  {
    rev: "b".repeat(40),
    author: "The Author",
    date: "2026-07-01T09:00:00Z",
    subject: "the deck as the author wrote it",
  },
];

/** How many times the editor was told to read the deck again. */
let reloaded = 0;

/** The panel, mounted and rendered once, the way the editor does it. */
function panel(client: RevisionsClient, slide = 0) {
  reloaded = 0;
  const surface = createRevisions(
    { reload: () => (reloaded += 1) },
    { client, deckBase: "slides", now: () => Date.parse("2026-07-30T09:00:00Z") },
  );

  document.body.append(surface.root);
  surface.render({ selection: { slide } } as never);

  return surface.root;
}

function frame(root: HTMLElement): HTMLIFrameElement | null {
  return root.querySelector<HTMLIFrameElement>(".slidx-revision-frame");
}

/** Opens the row for a commit and waits for the answer about it. */
async function openRow(root: HTMLElement, at = 0): Promise<void> {
  rows(root)[at]!.querySelector<HTMLElement>(".slidx-revision-open")!.click();
  await Promise.resolve();
  await Promise.resolve();
}

/** Clicks a button in the preview area and waits for what it asked for. */
async function press(root: HTMLElement, selector: string): Promise<void> {
  root.querySelector<HTMLButtonElement>(selector)!.click();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

function tab(root: HTMLElement): HTMLElement {
  return root.querySelector<HTMLElement>(".slidx-revisions-tab")!;
}

/** Opens the panel and waits for the read it starts. */
async function open(root: HTMLElement): Promise<void> {
  tab(root).click();
  await Promise.resolve();
  await Promise.resolve();
}

function rows(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(".slidx-revision")];
}

function text(root: HTMLElement, selector: string): string {
  return root.querySelector(selector)?.textContent ?? "";
}

describe("the history panel", () => {
  beforeEach(() => {
    document.body.replaceChildren();
  });

  it("stays out of the way until it is asked for", async () => {
    // It is a thing an author consults and closes. A fourth permanent column
    // would take width from the canvas, which is showing the actual talk.
    const client = fakeHistory({ commits: COMMITS });
    const root = panel(client);

    expect(root.getAttribute("data-open")).toBe("false");
    expect(rows(root)).toHaveLength(0);

    await open(root);
    expect(root.getAttribute("data-open")).toBe("true");
    expect(rows(root)).toHaveLength(2);
  });

  it("says who changed the deck and when, newest first", async () => {
    const root = panel(fakeHistory({ commits: COMMITS }));
    await open(root);

    expect(text(rows(root)[0]!, ".slidx-revision-subject")).toBe("rework the middle");
    expect(text(rows(root)[0]!, ".slidx-revision-meta")).toBe("The Author · 2 days ago");
    expect(text(rows(root)[1]!, ".slidx-revision-subject")).toBe("the deck as the author wrote it");
  });

  it("says a deck outside a repository has no history, in a sentence", async () => {
    // A deck in a directory nobody ran `git init` in is ordinary. The panel
    // says so and stops; it never becomes an error state.
    const client = fakeHistory();
    client.list_ = {
      available: false,
      reason: "This deck is not in a git repository, so there is no history to read.",
      commits: [],
    };

    const root = panel(client);
    await open(root);

    expect(text(root, ".slidx-revisions-note")).toContain("not in a git repository");
    expect(rows(root)).toHaveLength(0);
  });

  it("says a repository with no deck commits has nothing yet", async () => {
    const root = panel(fakeHistory({ available: true, commits: [] }));
    await open(root);

    expect(text(root, ".slidx-revisions-note")).toBe("No commit has touched this deck yet.");
  });

  it("says what a commit did to the deck when a row is opened", async () => {
    const client = fakeHistory({ commits: COMMITS });
    client.change = {
      first: false,
      slides: 4,
      subject: 'Retitle "What goes wrong" and retime a slide',
      changes: ['retitled "What goes wrong" to "What actually goes wrong"', "budget: 1m30s to 2m"],
    };

    const root = panel(client);
    await open(root);
    rows(root)[0]!.querySelector<HTMLElement>(".slidx-revision-open")!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(text(root, ".slidx-revision-headline")).toBe(
      'Retitle "What goes wrong" and retime a slide',
    );
    expect(
      [...root.querySelectorAll(".slidx-revision-lines li")].map((li) => li.textContent),
    ).toEqual(['retitled "What goes wrong" to "What actually goes wrong"', "budget: 1m30s to 2m"]);
    expect(client.asked).toEqual([COMMITS[0]!.rev]);
  });

  it("asks about a commit once and not again on the second look", async () => {
    // What a commit did to the deck cannot change. Re-reading it would be two
    // parses of the whole deck for an answer already on the screen.
    const client = fakeHistory({ commits: COMMITS });
    const root = panel(client);
    await open(root);

    const openRow = () => {
      rows(root)[0]!.querySelector<HTMLElement>(".slidx-revision-open")!.click();
      return Promise.resolve().then(() => Promise.resolve());
    };

    await openRow();
    await openRow(); // closes
    await openRow(); // opens again

    expect(client.asked).toEqual([COMMITS[0]!.rev]);
  });

  it("says nothing changed for a commit that touched no slide", async () => {
    // A commit under the deck directory that the parser reads as identical —
    // whitespace, a file that is not a slide. Inventing a change would be
    // worse than saying there was none.
    const client = fakeHistory({ commits: COMMITS });
    client.change = { first: false, slides: 3, subject: "", changes: [] };

    const root = panel(client);
    await open(root);
    rows(root)[0]!.querySelector<HTMLElement>(".slidx-revision-open")!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(text(root, ".slidx-revision-quiet")).toBe("Nothing in the deck itself changed.");
  });

  it("keeps its list when a revision stopped existing since the log was read", async () => {
    // A rebase while the editor was open. The row stays and simply has nothing
    // to say about the deck.
    const client = fakeHistory({ commits: COMMITS });
    client.change = null;

    const root = panel(client);
    await open(root);
    rows(root)[0]!.querySelector<HTMLElement>(".slidx-revision-open")!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(rows(root)).toHaveLength(2);
    expect(text(root, ".slidx-revision-quiet")).toBe("Nothing in the deck itself changed.");
  });

  it("re-reads the log every time it is opened", async () => {
    // An author commits in a terminal with the editor still running, so a list
    // fetched once at startup is a list that is wrong by lunchtime.
    const client = fakeHistory({ commits: [COMMITS[0]!] });
    const root = panel(client);

    await open(root);
    expect(rows(root)).toHaveLength(1);

    tab(root).click(); // closed
    client.list_ = { available: true, commits: COMMITS };
    await open(root);

    expect(rows(root)).toHaveLength(2);
  });
});

describe("the deck as a commit had it", () => {
  beforeEach(() => {
    document.body.replaceChildren();
  });

  it("shows the deck's own page for that commit rather than drawing one", async () => {
    // The preview is an iframe on the real route with a `?rev=`, which the dev
    // server answers through the renderer it answers the working copy with.
    // Nothing in this panel draws a slide, and that is the point: a second
    // renderer would be a second answer about layout.
    const root = panel(fakeHistory({ commits: COMMITS }), 2);
    await open(root);
    await openRow(root);

    expect(frame(root)!.getAttribute("src")).toBe(`/slides/3/?rev=${COMMITS[0]!.rev}`);
  });

  it("keeps up with the slide the author moved to", async () => {
    const surface = createRevisions(
      { reload: () => {} },
      { client: fakeHistory({ commits: COMMITS }), deckBase: "slides" },
    );
    document.body.append(surface.root);
    surface.render({ selection: { slide: 0 } } as never);

    await open(surface.root);
    await openRow(surface.root);
    surface.render({ selection: { slide: 3 } } as never);

    expect(frame(surface.root)!.getAttribute("src")).toBe(`/slides/4/?rev=${COMMITS[0]!.rev}`);
  });

  it("shows nothing at all rather than a stale slide when the row is closed", async () => {
    // A frame left pointing at the last commit looked at would be a page
    // claiming to be something nobody asked about.
    const root = panel(fakeHistory({ commits: COMMITS }));
    await open(root);
    await openRow(root);
    expect(frame(root)!.getAttribute("src")).toBeTruthy();

    await openRow(root);

    expect(frame(root)!.getAttribute("src")).toBeNull();
    expect(root.querySelector(".slidx-revisions-preview")!.getAttribute("data-showing")).toBe(
      "false",
    );
  });
});

describe("putting the deck back", () => {
  beforeEach(() => {
    document.body.replaceChildren();
  });

  it("asks the server to restore, and never writes a file itself", async () => {
    const client = fakeHistory({ commits: COMMITS });
    const root = panel(client);
    await open(root);
    await openRow(root);
    await press(root, ".slidx-revision-restore");

    expect(client.restored).toEqual([COMMITS[0]!.rev]);
  });

  it("reads the deck again once it has been put back", async () => {
    // A restore is a git operation rather than an edit, so nothing in the
    // editor's own undo stack knows about it and the session starts from disk.
    const root = panel(fakeHistory({ commits: COMMITS }));
    await open(root);
    await openRow(root);
    await press(root, ".slidx-revision-restore");

    expect(reloaded).toBeGreaterThan(0);
    expect(text(root, ".slidx-revision-said")).toContain("staged");
  });

  it("offers to undo, and undoing is one more restore", async () => {
    // What makes looking at history safe to act on: going back is not a
    // one-way door, and the way back is the same operation.
    const client = fakeHistory({ commits: COMMITS });
    const root = panel(client);
    await open(root);
    await openRow(root);

    const undo = () => root.querySelector<HTMLButtonElement>(".slidx-revision-undo")!;
    expect(undo().hidden).toBe(true);

    await press(root, ".slidx-revision-restore");
    expect(undo().hidden).toBe(false);

    client.answer = { restored: "c".repeat(40), previous: COMMITS[0]!.rev };
    await press(root, ".slidx-revision-undo");

    expect(client.restored).toEqual([COMMITS[0]!.rev, "c".repeat(40)]);
    // The offer ends when it is taken: undoing an undo would be a redo
    // wearing the wrong label.
    expect(undo().hidden).toBe(true);
    expect(text(root, ".slidx-revision-said")).toBe("Put back the way it was.");
  });

  it("says what is unsaved when the server refuses to write over it", async () => {
    // Going back must never be the thing that loses an afternoon, and an
    // author told which of their slides is at risk can go and deal with it.
    const client = fakeHistory({ commits: COMMITS });
    client.answer = {
      refused: "The deck has changes that are not committed.",
      changed: ["talks/deck/slides/0003.md"],
    };

    const root = panel(client);
    await open(root);
    await openRow(root);
    await press(root, ".slidx-revision-restore");

    expect(text(root, ".slidx-revision-said")).toBe(
      "The deck has changes that are not committed. (talks/deck/slides/0003.md)",
    );
    expect(reloaded).toBe(0);
    expect(root.querySelector<HTMLButtonElement>(".slidx-revision-undo")!.hidden).toBe(true);
  });
});

describe("how long ago a commit was", () => {
  const now = Date.parse("2026-07-30T12:00:00Z");

  it("counts in whole units a person would say out loud", () => {
    expect(ago("2026-07-30T11:59:30Z", now)).toBe("30 seconds ago");
    expect(ago("2026-07-30T09:00:00Z", now)).toBe("3 hours ago");
    expect(ago("2026-07-29T12:00:00Z", now)).toBe("yesterday");
    expect(ago("2026-07-16T12:00:00Z", now)).toBe("2 weeks ago");
    expect(ago("2025-07-30T12:00:00Z", now)).toBe("last year");
  });

  it("shows a date it cannot read rather than a wrong answer about it", () => {
    // git writes ISO-8601 and this reads it, but a panel that printed
    // "NaN days ago" would be worse than printing what it was given.
    expect(ago("not a date", now)).toBe("not a date");
  });
});
