import { describe, expect, it } from "vite-plus/test";

import { classify, keyOf, WRITE_ONLY } from "./write-only.mjs";

describe("classify", () => {
  it("reports a field nothing reads and nobody has justified", () => {
    const { unexplained } = classify(["a.rs:S.one"], new Set());

    expect(unexplained).toEqual(["a.rs:S.one"]);
  });

  it("stays quiet about a field whose reason is recorded", () => {
    const { unexplained, stale } = classify(["a.rs:S.one"], new Set(["a.rs:S.one"]));

    expect(unexplained).toEqual([]);
    expect(stale).toEqual([]);
  });

  it("reports an exemption for a field that is read again", () => {
    // The inversion that is easy to get backwards: an exemption goes stale when
    // its field stops appearing in the unread list, not when it appears in it.
    const { stale } = classify([], new Set(["a.rs:S.gone"]));

    expect(stale).toEqual(["a.rs:S.gone"]);
  });

  it("separates the two kinds in one pass", () => {
    const { unexplained, stale } = classify(
      ["a.rs:S.new", "b.rs:T.known"],
      new Set(["b.rs:T.known", "c.rs:U.read"]),
    );

    expect(unexplained).toEqual(["a.rs:S.new"]);
    expect(stale).toEqual(["c.rs:U.read"]);
  });
});

describe("the recorded exemptions", () => {
  it("name a file, a struct and a field, so a stale one can be found", () => {
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
