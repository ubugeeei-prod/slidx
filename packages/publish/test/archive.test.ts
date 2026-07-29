/**
 * The archive record, and the index built out of many of them.
 *
 * This is the only target whose input is not finished when it first runs, and
 * the whole specification follows from that. The slides go up the evening of
 * the talk; the recording appears when the conference gets round to it, which
 * is weeks and sometimes never. A target that blocked until everything existed
 * would produce nothing on the one day the author is still thinking about the
 * talk, and would be run again by nobody.
 *
 * So the failure modes guarded here are about time rather than about fields:
 *
 * - Refusing to record a talk that happened, because a URL that does not exist
 *   yet does not exist yet.
 * - Confusing *missing* with *not yet*. The author can add `url:` right now;
 *   they cannot make the conference publish the video.
 * - Changing the record when nothing changed, so that re-running months later
 *   produces a diff of exactly the recording and nothing else.
 * - Dropping an undated talk out of the index because it did not sort.
 */

import { describe, expect, it } from "vite-plus/test";

import { composeArchive, describeArchive } from "../src/targets/archive";
import type { ArchiveRecord } from "../src/targets/archive";
import { buildTalkIndex } from "../src/talks";
import { deck, TALK, without } from "./support";

function record(meta = TALK): ArchiveRecord {
  const result = composeArchive(deck(meta));
  if (!result.ok) throw new Error(`expected a record, got: ${result.reasons[0]?.message ?? ""}`);
  return result.value;
}

function pendingFields(meta = TALK): string[] {
  return record(meta).pending.map((entry) => entry.field);
}

describe("recording a talk that happened", () => {
  it("keeps the fields the author already wrote", () => {
    const entry = record();

    expect(entry.title).toBe("Zero-JavaScript Slides");
    expect(entry.event).toBe("SlidxConf 2026");
    expect(entry.date).toBe("2026-07-29");
    expect(entry.venue).toBe("Kyoto");
    expect(entry.deck).toBe("https://slidx.dev/talks/zero-js");
  });

  it("files the record under a slug, so re-running overwrites rather than piles up", () => {
    expect(record().path).toBe("talks/zero-javascript-slides.md");
  });

  it("honours a pinned slug, because the path is an address someone has bookmarked", () => {
    expect(record({ ...TALK, slug: "zero-js" }).path).toBe("talks/zero-js.md");
  });

  it("keeps a non-Latin title in the file name, because the file is the author's", () => {
    // Unlike a slide-host URL, this path never leaves the author's disk, so
    // there is no reason to reduce a Japanese talk to nothing.
    expect(record({ ...TALK, title: "プレーンな HTML の話" }).path).toBe(
      "talks/プレーンな-html-の話.md",
    );
  });

  it("falls back to the date when a title yields no file name at all", () => {
    const entry = record({ ...TALK, title: "!!! ???" });

    expect(entry.path).toBe("talks/talk-2026-07-29.md");
    expect(entry.pending.map((item) => item.field)).toContain("slug");
  });

  it("records a talk with nothing but a title, because that talk still happened", () => {
    const result = composeArchive(deck({ title: "Lightning talk" }));

    expect(result.ok).toBe(true);
  });

  it("records a talk named only by its event", () => {
    // A deck whose title lives on the first slide rather than in frontmatter.
    // The event and date are enough to find it again.
    const result = composeArchive(deck({ event: "SlidxConf 2026", date: "2026-07-29" }));

    expect(result.ok).toBe(true);
  });

  it("refuses only when there is nothing to name the talk by", () => {
    const result = composeArchive(deck({ author: "ubugeeei" }));

    expect(result.ok).toBe(false);
    expect(result.ok ? [] : result.reasons.map((entry) => entry.field)).toContain("title");
  });
});

describe("what is missing, and what is merely not here yet", () => {
  it("waits for the recording instead of blocking on it", () => {
    // The distinction this target exists for. The author cannot make the
    // conference publish the video, so this is a return-later, not a fix-now.
    const entry = record();

    expect(entry.recording).toBeUndefined();
    expect(pendingFields()).toContain("recording");
    expect(entry.pending.find((item) => item.field === "recording")?.message).toMatch(
      /when the (conference|recording)/i,
    );
  });

  it("stops asking once the recording is attached", () => {
    const entry = record({ ...TALK, recording: "https://youtu.be/abc123" });

    expect(entry.recording).toBe("https://youtu.be/abc123");
    expect(pendingFields({ ...TALK, recording: "https://youtu.be/abc123" })).not.toContain(
      "recording",
    );
  });

  it("waits for the deck URL too, since it is usually published the same evening", () => {
    expect(pendingFields(without(TALK, "url"))).toContain("url");
  });

  it("reports a date it cannot order by, rather than silently mis-sorting it", () => {
    // `2026-7-9` sorts before `2026-11-01` as text. A talk index that puts
    // November before July is wrong in a way nobody reads carefully enough to
    // notice, so the shape is checked rather than trusted.
    expect(pendingFields({ ...TALK, date: "2026-7-9" })).toContain("date");
  });

  it("says nothing is pending when the record is complete", () => {
    const complete = { ...TALK, recording: "https://youtu.be/abc123" };

    expect(record(complete).pending).toEqual([]);
  });
});

describe("the record as a file", () => {
  it("writes frontmatter a static site can read", () => {
    const { markdown } = record({ ...TALK, recording: "https://youtu.be/abc123" });

    expect(markdown.startsWith("---\n")).toBe(true);
    expect(markdown).toContain('title: "Zero-JavaScript Slides"');
    expect(markdown).toContain('date: "2026-07-29"');
    expect(markdown).toContain('recording: "https://youtu.be/abc123"');
  });

  it("omits a field it does not have rather than writing an empty one", () => {
    // An empty `recording: ""` reads as "there is no recording" to a site
    // template, which is a different claim from "not yet".
    expect(record().markdown).not.toContain("recording:");
  });

  it("quotes values, so a title with a colon does not become two keys", () => {
    const { markdown } = record({ ...TALK, title: 'Slides: a "talk" about talks' });

    expect(markdown).toContain('title: "Slides: a \\"talk\\" about talks"');
  });

  it("changes only the recording when only the recording changed", () => {
    // The property that makes re-running months later safe: the diff an author
    // sees is the thing they actually did.
    const before = record().markdown.split("\n");
    const after = record({ ...TALK, recording: "https://youtu.be/abc123" }).markdown.split("\n");

    expect(after.filter((line) => !before.includes(line))).toEqual([
      'recording: "https://youtu.be/abc123"',
    ]);
  });

  it("is unchanged when nothing changed", () => {
    expect(record().markdown).toBe(record().markdown);
  });
});

describe("the index across every talk", () => {
  const kyoto = record({ ...TALK, title: "Kyoto talk", date: "2026-07-29" });
  const tokyo = record({ ...TALK, title: "Tokyo talk", date: "2025-11-02" });
  const osaka = record({ ...TALK, title: "Osaka talk", date: "2026-02-14" });
  const undated = record(without({ ...TALK, title: "Undated talk" }, "date"));

  function titles(records: ArchiveRecord[]): string[] {
    return buildTalkIndex(records).talks.map((entry) => entry.title);
  }

  it("leads with the most recent talk, which is what a speaking page is for", () => {
    expect(titles([tokyo, kyoto, osaka])).toEqual(["Kyoto talk", "Osaka talk", "Tokyo talk"]);
  });

  it("keeps an undated talk, after the dated ones and in the order given", () => {
    // Dropping it would lose a talk. Guessing a date would invent one.
    expect(titles([undated, tokyo, kyoto])).toEqual(["Kyoto talk", "Tokyo talk", "Undated talk"]);
  });

  it("does not reorder two talks given the same date", () => {
    const first = record({ ...TALK, title: "Morning", date: "2026-07-29" });
    const second = record({ ...TALK, title: "Afternoon", date: "2026-07-29" });

    expect(titles([first, second])).toEqual(["Morning", "Afternoon"]);
  });

  it("groups by year, because that is how a speaking page reads", () => {
    const { markdown } = buildTalkIndex([tokyo, kyoto, osaka]);

    expect(markdown.indexOf("## 2026")).toBeLessThan(markdown.indexOf("## 2025"));
    expect(markdown).toContain("## 2025");
  });

  it("counts the recordings still outstanding, since that is the chase", () => {
    const withVideo = record({ ...TALK, title: "Done", recording: "https://youtu.be/abc123" });

    expect(buildTalkIndex([withVideo, kyoto, tokyo]).awaitingRecording).toBe(2);
  });

  it("links only what exists", () => {
    const { markdown } = buildTalkIndex([kyoto]);

    expect(markdown).toContain("https://slidx.dev/talks/zero-js");
    expect(markdown).not.toContain("()");
  });

  it("produces a page for an author who has given no talks yet", () => {
    const index = buildTalkIndex([]);

    expect(index.talks).toEqual([]);
    expect(index.markdown).toContain("#");
  });
});

describe("one line for a printed plan", () => {
  it("names what is still outstanding", () => {
    expect(describeArchive(record())).toMatch(/talks\/zero-javascript-slides\.md/);
    expect(describeArchive(record())).toMatch(/recording/);
  });

  it("says so when the record is complete", () => {
    const complete = record({ ...TALK, recording: "https://youtu.be/abc123" });

    expect(describeArchive(complete)).not.toMatch(/awaiting/);
  });
});
