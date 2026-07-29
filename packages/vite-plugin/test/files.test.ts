/**
 * Joining a directory of slide files into the source the parser reads.
 *
 * The join is where a multi-file deck stops being files and starts being a
 * document, so it is also where a file can quietly stop being its own slide.
 */

import { describe, expect, it } from "vite-plus/test";

import { joinDeck, type DeckFile } from "../src/files";

function files(...sources: string[]): DeckFile[] {
  return sources.map((source, index) => ({
    path: `/slides/${index}.md`,
    label: `slides/${index}.md`,
    source,
  }));
}

describe("joining slide files", () => {
  it("puts each file's own bytes where its span says they are", () => {
    const joined = joinDeck(files("# One\n", "# Two\n"), "---");

    expect(joined.spans.map((span) => joined.source.slice(span.start, span.end))).toEqual([
      "# One",
      "# Two",
    ]);
  });

  it("keeps the first file's frontmatter at the very top, where the deck's belongs", () => {
    const joined = joinDeck(files("---\ntitle: T\n---\n\n# One\n", "# Two\n"), "---");

    expect(joined.source).toMatch(/^---\ntitle: T\n---/);
  });

  it("keeps a file whose first line reads as YAML as its own slide", () => {
    // `## Heading` is a YAML comment and `<!-- notes: x -->` is a YAML key, so
    // a separator with no blank line under it turns the next file's opening
    // slide into frontmatter for the one after it. The blank line is what says
    // "this is a slide break", not "this is a block of keys".
    const joined = joinDeck(
      files("# One\n", "## The problem\n\n<!-- notes: said -->\n\n---\n\n# Three\n"),
      "---",
    );

    expect(joined.source).toContain("---\n\n## The problem");
  });

  it("lets a file that opens with a separator be its own slide break", () => {
    // That line is the opening delimiter of the slide's frontmatter *and* the
    // break between the two slides. Writing another above it would leave an
    // empty slide between them, and the pipeline follows the same rule when it
    // moves a slide — which is what makes reading a deck the exact inverse of
    // writing one.
    const joined = joinDeck(files("# One\n", "---\nlayout: split\n---\n\n# Two\n"), "---");

    expect(joined.source).toBe("# One\n\n---\nlayout: split\n---\n\n# Two");
    expect(joined.source.slice(joined.spans[1]!.start)).toBe("---\nlayout: split\n---\n\n# Two");
  });

  it("gives a file with nothing in it a place but no separator", () => {
    // A file this session emptied would otherwise join the deck as a blank
    // slide. It keeps its position so an undo can put its slides back.
    const joined = joinDeck(files("# One\n", "", "# Three\n"), "---");

    expect(joined.source).toBe("# One\n\n---\n\n# Three");
    expect(joined.spans[1]).toEqual({ start: 5, end: 5 });
  });

  it("measures a file in bytes, because that is what a splice is measured in", () => {
    // One em dash is one JavaScript character and three bytes. Counting the
    // wrong one shifts every later span by two and lands the next write in the
    // middle of a word — silently, and only on a deck that is not pure ASCII.
    const joined = joinDeck(files("# One — really\n", "# Two\n"), "---");

    expect(joined.spans[0]).toEqual({ start: 0, end: 16 });
    expect(joined.spans[1]!.start).toBe(16 + Buffer.byteLength("\n\n---\n\n"));
    expect(Buffer.from(joined.source, "utf8").toString("utf8", 0, 16)).toBe("# One — really");
  });

  it("writes the separator the deck was configured with", () => {
    expect(joinDeck(files("# One\n", "# Two\n"), "===").source).toBe("# One\n\n===\n\n# Two");
  });
});
