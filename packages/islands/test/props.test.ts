/**
 * Reading an island's props out of the markup.
 *
 * Props arrive as JSON in an attribute, which means they arrive as text some
 * other program wrote. This is the specification for what happens when that
 * program was wrong, and the answer is always the same: the island loses its
 * props, never the slide.
 *
 * The failure modes guarded here:
 *
 * - A `JSON.parse` that escapes takes down every island after it on the page,
 *   so nothing here may throw for any input.
 * - Valid JSON that is not an object — `null`, an array, a bare number — is a
 *   compiler bug that would otherwise reach a component as props and fail
 *   somewhere unrecognisable.
 * - `__proto__` as an own key of a props object escapes into `Object.prototype`
 *   the moment a framework copies props with `Object.assign`, which both Vue
 *   and React do.
 */

import { describe, expect, it } from "vite-plus/test";

import { parseProps } from "../src/props";

describe("props that are absent", () => {
  it("reads a missing attribute as no props", () => {
    expect(parseProps(null)).toEqual({ props: {} });
  });

  it("reads an undefined attribute as no props", () => {
    expect(parseProps(undefined)).toEqual({ props: {} });
  });

  it("reads an empty attribute as no props without complaining", () => {
    // A compiler is allowed to write the attribute empty rather than omit it.
    // Warning here would fire on the majority of islands.
    expect(parseProps("")).toEqual({ props: {} });
  });

  it("reads a whitespace-only attribute as no props without complaining", () => {
    expect(parseProps("  \n ")).toEqual({ props: {} });
  });

  it("returns a fresh object each time", () => {
    // A shared empty object would let one island's component mutate another's
    // props, and the two islands need never be on the same slide.
    const first = parseProps(null).props;
    const second = parseProps(null).props;

    first["mutated"] = true;

    expect(second).toEqual({});
  });
});

describe("props that parse", () => {
  it("reads a JSON object", () => {
    expect(parseProps('{"title":"Q3","count":4}')).toEqual({
      props: { title: "Q3", count: 4 },
    });
  });

  it("keeps nested structure intact", () => {
    const { props } = parseProps('{"series":[1,2,3],"axis":{"label":"time"}}');

    expect(props["series"]).toEqual([1, 2, 3]);
    expect(props["axis"]).toEqual({ label: "time" });
  });

  it("keeps null and false values rather than dropping them", () => {
    // A component distinguishes "absent" from "explicitly nothing"; collapsing
    // them here would make that distinction unreachable.
    const { props } = parseProps('{"caption":null,"animate":false}');

    expect(props).toEqual({ caption: null, animate: false });
    expect("caption" in props).toBe(true);
  });

  it("keeps keys that are not identifiers", () => {
    const { props } = parseProps('{"data-id":"a","2":"b"}');

    expect(props["data-id"]).toBe("a");
    expect(props["2"]).toBe("b");
  });

  it("reports nothing when the props are fine", () => {
    expect(parseProps('{"ok":true}').problem).toBeUndefined();
  });
});

describe("props that do not parse", () => {
  it("does not throw on malformed JSON", () => {
    expect(() => parseProps("{not json}")).not.toThrow();
  });

  it("mounts with empty props when the JSON is malformed", () => {
    const { props, problem } = parseProps("{not json}");

    expect(props).toEqual({});
    expect(problem).toMatch(/not valid JSON/);
  });

  it("handles JSON truncated mid-value", () => {
    const { props, problem } = parseProps('{"title":"Q');

    expect(props).toEqual({});
    expect(problem).toBeDefined();
  });

  it("quotes the offending text so it can be found in the deck", () => {
    const { problem } = parseProps("{oops}");

    expect(problem).toContain("{oops}");
  });

  it("truncates a very long attribute rather than filling the console", () => {
    const long = `{"a":"${"x".repeat(500)}"`;
    const { problem } = parseProps(long);

    expect(problem).toContain("…");
    expect(problem?.length).toBeLessThan(200);
  });
});

describe("props that parse but are not an object", () => {
  it("rejects a JSON null", () => {
    const { props, problem } = parseProps("null");

    expect(props).toEqual({});
    expect(problem).toContain("got null");
  });

  it("rejects an array", () => {
    const { props, problem } = parseProps("[1,2,3]");

    expect(props).toEqual({});
    expect(problem).toContain("got an array");
  });

  it("rejects a bare string", () => {
    expect(parseProps('"vue"').problem).toContain("got a string");
  });

  it("rejects a bare number", () => {
    expect(parseProps("42").problem).toContain("got a number");
  });

  it("rejects a bare boolean", () => {
    expect(parseProps("true").problem).toContain("got a boolean");
  });

  it("says what it wanted, not only what it got", () => {
    expect(parseProps("[]").problem).toContain("props must be a JSON object");
  });
});

describe("props that would escape", () => {
  it("drops a __proto__ key", () => {
    const { props } = parseProps('{"__proto__":{"polluted":true},"title":"Q3"}');

    expect(Object.keys(props)).toEqual(["title"]);
  });

  it("reports the key it dropped", () => {
    const { problem } = parseProps('{"__proto__":{"polluted":true}}');

    expect(problem).toContain("__proto__");
  });

  it("leaves Object.prototype alone", () => {
    // The reason the key is dropped at all: Vue and React both copy props with
    // `Object.assign`, which assigns rather than defines, so an own
    // `__proto__` on the source replaces the target's prototype.
    parseProps('{"__proto__":{"polluted":true}}');

    const target: Record<string, unknown> = {};
    Object.assign(target, parseProps('{"__proto__":{"polluted":true}}').props);

    expect(({} as Record<string, unknown>)["polluted"]).toBeUndefined();
    expect(Object.getPrototypeOf(target)).toBe(Object.prototype);
  });

  it("keeps the rest of the props when one key is dropped", () => {
    const { props } = parseProps('{"__proto__":{},"count":7}');

    expect(props).toEqual({ count: 7 });
  });
});
