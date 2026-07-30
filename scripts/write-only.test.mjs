import { describe, expect, it } from "vite-plus/test";

import { classify, keyOf, WRITE_ONLY } from "./write-only.mjs";

describe("classify", () => {
  it("reports a field nothing reads and nobody has justified", () => {
    const { unexplained } = classify(["a.rs:S.one"], ["a.rs:S.one"], new Set());

    expect(unexplained).toEqual(["a.rs:S.one"]);
  });

  it("stays quiet about a field whose reason is recorded", () => {
    const { unexplained, orphaned } = classify(
      ["a.rs:S.one"],
      ["a.rs:S.one"],
      new Set(["a.rs:S.one"]),
    );

    expect(unexplained).toEqual([]);
    expect(orphaned).toEqual([]);
  });

  it("says nothing about an exemption whose field looks read again", () => {
    // Deliberate silence. A read is found by name, so an unrelated struct with
    // a field of the same name is enough to make one look read — the tool
    // cannot tell an exemption that is no longer needed from a collision, so it
    // does not claim to. Reporting it anyway called all five entries stale the
    // day an unrelated command added a `.result`.
    const { unexplained, orphaned } = classify([], ["a.rs:S.read"], new Set(["a.rs:S.read"]));

    expect(unexplained).toEqual([]);
    expect(orphaned).toEqual([]);
  });

  it("reports an exemption for a field the workspace no longer declares", () => {
    // A rename is the rot that actually happens, and unlike a read it is
    // checkable: the entry names something that is not there.
    const { orphaned } = classify([], ["a.rs:S.renamed"], new Set(["a.rs:S.gone"]));

    expect(orphaned).toEqual(["a.rs:S.gone"]);
  });

  it("separates the two kinds in one pass", () => {
    const { unexplained, orphaned } = classify(
      ["a.rs:S.new", "b.rs:T.known"],
      ["a.rs:S.new", "b.rs:T.known"],
      new Set(["b.rs:T.known", "c.rs:U.deleted"]),
    );

    expect(unexplained).toEqual(["a.rs:S.new"]);
    expect(orphaned).toEqual(["c.rs:U.deleted"]);
  });
});

describe("the recorded exemptions", () => {
  it("name a file, a struct and a field, so an orphaned one can be found", () => {
    for (const entry of WRITE_ONLY.keys()) {
      expect(entry).toMatch(/^[\w./-]+\.rs:\w+\.\w+$/);
    }
  });

  it("each carry a reason long enough to be one", () => {
    for (const [entry, reason] of WRITE_ONLY) {
      expect(reason.length, entry).toBeGreaterThan(30);
    }
  });

  it("is what keyOf produces, so the two cannot drift apart", () => {
    expect(
      keyOf({ file: "crates/slidx_lsp/src/hover.rs", struct: "Hover", field: "contents" }),
    ).toBe("crates/slidx_lsp/src/hover.rs:Hover.contents");
    expect(WRITE_ONLY.has("crates/slidx_lsp/src/hover.rs:Hover.contents")).toBe(true);
  });
});
