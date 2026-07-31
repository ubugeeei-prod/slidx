/**
 * The claims check, tested by writing the wrong sentences down.
 *
 * Which is why this file is excluded from the scan it tests: proving that a
 * phrase is caught means putting the phrase in a file, and a checker that
 * failed on its own test fixtures would be a checker nobody could develop.
 */

import { describe, expect, it } from "vite-plus/test";

import { CLAIMS, EXEMPT, overstatements, readableFiles } from "../claims.mjs";

describe("catching a claim that is wider than what proves it", () => {
  it("finds the phrase whatever case it was written in", () => {
    const found = overstatements("The deck makes Zero Network Requests.");

    expect(found).toHaveLength(1);
    expect(found[0].claim.phrase).toBe("zero network requests");
  });

  it("says which line, so a long document is actionable", () => {
    expect(overstatements("one\ntwo\nzero network requests\n")[0].line).toBe(3);
  });

  it("carries the true sentence to write instead, not only the objection", () => {
    // A rule that says no and stops is a rule somebody works around.
    expect(overstatements("zero network requests")[0].claim.instead).toContain("another origin");
  });

  it("says nothing about the sentence that is actually true", () => {
    expect(overstatements("A built deck asks nothing of anywhere but itself.")).toEqual([]);
  });

  it("reports every line rather than the first", () => {
    expect(overstatements("zero network requests\nfine\nno network requests\n")).toHaveLength(2);
  });
});

describe("what the check reads", () => {
  it("reads the README, which is where this one came back", () => {
    expect(readableFiles()).toContain("README.md");
  });

  it("reads a crate's doc comments, where three copies of it lived", () => {
    expect(readableFiles()).toContain("crates/slidx_lint/src/rules/offline.rs");
  });

  it("skips tests, which have to write the wrong version down", () => {
    expect(readableFiles()).not.toContain("scripts/test/claims.test.mjs");
  });

  it("skips the one file that names the phrases", () => {
    expect(readableFiles()).not.toContain(EXEMPT[0]);
  });

  it("skips what nobody reads as prose", () => {
    expect(readableFiles().some((file) => file.endsWith(".png"))).toBe(false);
    expect(readableFiles().some((file) => file.endsWith(".lock"))).toBe(false);
  });
});

describe("what is on the list", () => {
  it("gives every claim a reason and a replacement", () => {
    // A bare list of forbidden phrases is how a rule outlives the reason for
    // it, and how the next person deletes it as arbitrary.
    for (const claim of CLAIMS) {
      expect(claim.wrong.length).toBeGreaterThan(0);
      expect(claim.instead.length).toBeGreaterThan(0);
    }
  });

  it("writes every phrase in the case it is matched in", () => {
    for (const claim of CLAIMS) expect(claim.phrase).toBe(claim.phrase.toLowerCase());
  });
});
