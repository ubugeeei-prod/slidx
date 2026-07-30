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
  type RevisionChange,
  type RevisionList,
  type RevisionsClient,
} from "../src/revisions";

interface Fake extends RevisionsClient {
  asked: string[];
  list_: RevisionList;
  change: RevisionChange | null;
}

function fakeHistory(list: Partial<RevisionList> = {}): Fake {
  const fake: Fake = {
    asked: [],
    list_: { available: true, commits: [], ...list },
    change: { first: false, slides: 3, subject: 'Add "What it cost"', changes: [] },

    list: async () => fake.list_,
    changeAt: async (rev) => {
      fake.asked.push(rev);
      return fake.change;
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

/** The panel, mounted and rendered once, the way the editor does it. */
function panel(client: RevisionsClient) {
  const surface = createRevisions({ client, now: () => Date.parse("2026-07-30T09:00:00Z") });
  document.body.append(surface.root);
  surface.render({} as never);

  return surface.root;
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
