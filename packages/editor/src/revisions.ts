/**
 * The deck's history, beside the deck.
 *
 * A deck is plain Markdown in a git repository, so its past is already
 * recorded — and reading it should not mean leaving the editor for a terminal
 * and translating `+34 −6` back into slides. This panel lists the commits that
 * touched the deck and says what each one did in the deck's own vocabulary:
 * the slide that was retitled, the step that was added, the budget that grew.
 *
 * # Nothing here decides what a change is called
 *
 * The sentences come from the pipeline, composed by the same code that writes
 * a commit message from a deck-level summary. A panel that worded a change for
 * itself would let a talk's history and its own record of itself disagree about
 * what an author did, which is the kind of drift this project has one parser to
 * prevent.
 *
 * # A deck with no repository is an ordinary deck
 *
 * Not every deck is in git, and not every machine has git on it. Neither may
 * stop the editor loading, so the panel asks, is told which situation it is in,
 * and says that in a sentence — it never becomes an error state.
 */

import { EDITOR_BASE } from "./client";
import { element, fill } from "./dom";
import type { Surface } from "./outline";
import { applyRevisionsStyles } from "./revisions-styles";

/** One commit that touched the deck. */
export interface Revision {
  rev: string;
  author: string;
  /** ISO-8601, so this side decides how a date is spelled. */
  date: string;
  subject: string;
}

/** What there is to show, or why there is nothing. */
export interface RevisionList {
  available: boolean;
  reason?: string;
  commits: Revision[];
}

/** What one commit did to the deck, said in slides. */
export interface RevisionChange {
  first: boolean;
  slides: number;
  /** Empty when the commit did not touch the deck. */
  subject: string;
  changes: string[];
}

/**
 * The dev server, as this panel talks to it.
 *
 * Its own rather than a pair of methods on the editor's client, because the
 * subject here is the repository rather than the deck being edited: the panel
 * reads it when it is opened and nothing else in the editor waits for it.
 * Injected so the panel can be driven in a test without a server.
 */
export interface RevisionsClient {
  list(): Promise<RevisionList>;
  changeAt(rev: string): Promise<RevisionChange | null>;
}

export interface RevisionsOptions {
  client?: RevisionsClient;
  /** Substituted in tests, where "3 days ago" has to be a fixed sentence. */
  now?: () => number;
}

export function createRevisions(options: RevisionsOptions = {}): Surface {
  const client = options.client ?? createRevisionsClient();
  const now = options.now ?? Date.now;

  const tab = element(
    "button",
    { type: "button", class: "slidx-revisions-tab", "aria-expanded": false },
    ["History"],
  );
  const body = element("div", { class: "slidx-revisions-body" });
  const panel = element("div", { class: "slidx-revisions-panel" }, [
    element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["History"])]),
    body,
  ]);
  const root = element("section", { class: "slidx-revisions", "aria-label": "History" }, [
    tab,
    panel,
  ]);

  root.setAttribute("data-open", "false");
  applyRevisionsStyles(root.ownerDocument);

  let open = false;
  let commits: Revision[] = [];
  let selected: string | undefined;
  const changes = new Map<string, RevisionChange>();

  function note(text: string): void {
    fill(body, [element("p", { class: "slidx-revisions-note" }, [text])]);
  }

  function draw(): void {
    if (commits.length === 0) {
      // Reached only once the list came back empty from a repository that
      // could be read: a deck committed by nobody yet.
      note("No commit has touched this deck yet.");
      return;
    }

    fill(body, [
      element(
        "ol",
        { class: "slidx-revisions-list" },
        commits.map((commit) => row(commit)),
      ),
    ]);
  }

  function row(commit: Revision): HTMLElement {
    const open = element("button", { type: "button", class: "slidx-revision-open" }, [
      element("span", { class: "slidx-revision-subject" }, [commit.subject || "(no message)"]),
      element("span", { class: "slidx-revision-meta" }, [
        `${commit.author} · ${ago(commit.date, now())}`,
      ]),
    ]);

    const item = element(
      "li",
      {
        class: "slidx-revision",
        "data-rev": commit.rev,
        "aria-current": commit.rev === selected,
      },
      [open],
    );

    if (commit.rev === selected) item.append(changeFor(commit.rev));
    open.addEventListener("click", () => void choose(commit.rev));

    return item;
  }

  /** What the deck says a commit did, once it has been asked for. */
  function changeFor(rev: string): HTMLElement {
    const change = changes.get(rev);
    const block = element("div", { class: "slidx-revision-change" });

    if (!change) {
      block.append(element("p", { class: "slidx-revision-quiet" }, ["Reading the deck…"]));
      return block;
    }

    if (change.subject.length === 0) {
      // The commit is in the log because it touched a file under the deck
      // directory, and nothing the parser reads came out different.
      block.append(
        element("p", { class: "slidx-revision-quiet" }, ["Nothing in the deck itself changed."]),
      );
      return block;
    }

    block.append(element("p", { class: "slidx-revision-headline" }, [change.subject]));

    if (change.changes.length > 0) {
      block.append(
        element(
          "ul",
          { class: "slidx-revision-lines" },
          change.changes.map((line) => element("li", {}, [line])),
        ),
      );
    }

    return block;
  }

  async function choose(rev: string): Promise<void> {
    // Clicking the open row closes it, so a reader can get the list back
    // without hunting for a control that only exists once something is open.
    selected = selected === rev ? undefined : rev;
    draw();
    if (selected === undefined || changes.has(rev)) return;

    const change = await client.changeAt(rev);
    // A revision the repository no longer has — a rebase since the log was
    // read. The row stays; it simply has nothing to say about the deck.
    changes.set(rev, change ?? { first: false, slides: 0, subject: "", changes: [] });
    if (selected === rev) draw();
  }

  /**
   * Reads the log, every time the panel is opened.
   *
   * An author commits in a terminal with the editor still running, so a list
   * fetched once at startup is a list that is wrong by lunchtime. Opening the
   * panel is the moment they are asking, and one git call is cheap enough to
   * make it the moment it is answered.
   */
  async function load(): Promise<void> {
    note("Reading the deck's history…");

    const answer = await client.list();
    if (!answer.available) {
      note(answer.reason ?? "There is no history to read.");
      return;
    }

    commits = answer.commits;
    changes.clear();
    selected = undefined;
    draw();
  }

  tab.addEventListener("click", () => {
    open = !open;
    root.setAttribute("data-open", String(open));
    tab.setAttribute("aria-expanded", String(open));
    if (open) void load();
  });

  return {
    root,
    // Deliberately nothing. This panel's subject is the repository, not what
    // is selected in the editor, and its list is re-read when it is opened —
    // which is also the only moment an author is asking about it.
    render() {},
  };
}

/** The panel's half of the dev server's history routes. */
export function createRevisionsClient(
  send: typeof globalThis.fetch = globalThis.fetch.bind(globalThis),
): RevisionsClient {
  return {
    async list() {
      try {
        const response = await send(`${EDITOR_BASE}history`);
        return (await response.json()) as RevisionList;
      } catch {
        // A dev server that went away while the editor stayed open. The panel
        // says so rather than throwing into a click handler.
        return { available: false, reason: "The dev server did not answer.", commits: [] };
      }
    },

    async changeAt(rev) {
      const response = await send(`${EDITOR_BASE}history/change?rev=${encodeURIComponent(rev)}`);
      if (!response.ok) return null;

      return (await response.json()) as RevisionChange;
    },
  };
}

/**
 * How long ago something happened, in whole units.
 *
 * `Intl` rather than a table of thresholds and plurals, so a deck opened by
 * somebody who does not read English gets their own language for free — the
 * one string in this panel that is not written by the pipeline.
 */
export function ago(iso: string, now: number): string {
  const at = Date.parse(iso);
  if (Number.isNaN(at)) return iso;

  const seconds = Math.round((at - now) / 1000);
  const format = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31_536_000],
    ["month", 2_592_000],
    ["week", 604_800],
    ["day", 86_400],
    ["hour", 3600],
    ["minute", 60],
  ];

  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return format.format(Math.trunc(seconds / size), unit);
  }

  return format.format(seconds, "second");
}
