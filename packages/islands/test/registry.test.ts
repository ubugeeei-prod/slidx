/**
 * Which frameworks a deck opted into.
 *
 * The registry is a map, so the behaviour worth specifying is not lookup — it
 * is what happens when lookup *fails*. `vue` in the frontmatter with
 * `vueIsland` never registered is the first mistake almost everyone makes, and
 * the message is the only thing standing between that and an afternoon spent
 * reading the compiler.
 *
 * The failure modes guarded here:
 *
 * - A message that says what is missing without saying what is present sends
 *   an author to the wrong file.
 * - Whitespace out of a template must not read as a different framework.
 * - A definition with no name can never be selected, so registering one is a
 *   mistake worth catching where it is made rather than on slide 40.
 * - Hot module replacement re-runs a deck's setup module on every edit, so a
 *   second registration of the same name is normal and must not throw.
 */

import { describe, expect, it } from "vitest";

import type { IslandDefinition, IslandHandle } from "../src/contract";
import { createRegistry, unknownIslandMessage } from "../src/registry";

function definition(name: string): IslandDefinition {
  return {
    name,
    mount: (): Promise<IslandHandle> => Promise.resolve({ unmount: () => {} }),
  };
}

describe("looking an island up", () => {
  it("finds what was registered", () => {
    const registry = createRegistry();
    const vue = definition("vue");
    registry.register(vue);

    expect(registry.lookup("vue")).toBe(vue);
  });

  it("has nothing before anything is registered", () => {
    const registry = createRegistry();

    expect(registry.lookup("vue")).toBeUndefined();
    expect(registry.has("vue")).toBe(false);
    expect(registry.names()).toEqual([]);
  });

  it("reports presence the same way it resolves", () => {
    const registry = createRegistry([definition("react")]);

    expect(registry.has("react")).toBe(true);
    expect(registry.has("vue")).toBe(false);
  });

  it("ignores whitespace around the name", () => {
    // Attributes pick up whitespace from templating and from hand-written
    // markup. `"vue "` is not a different framework.
    const registry = createRegistry([definition("vue")]);

    expect(registry.lookup(" vue")).toBeDefined();
    expect(registry.lookup("vue\n")).toBeDefined();
  });

  it("treats a differently-cased name as unknown", () => {
    // The token is a lowercase identifier, not prose. Matching loosely here
    // would hide the mistake instead of reporting it.
    const registry = createRegistry([definition("vue")]);

    expect(registry.lookup("Vue")).toBeUndefined();
  });

  it("seeds from an iterable so a deck registers in one expression", () => {
    const registry = createRegistry([definition("vue"), definition("react")]);

    expect(registry.names()).toEqual(["react", "vue"]);
  });

  it("keeps registries independent", () => {
    const one = createRegistry([definition("vue")]);
    const other = createRegistry();

    expect(other.lookup("vue")).toBeUndefined();
    expect(one.lookup("vue")).toBeDefined();
  });
});

describe("registering", () => {
  it("replaces a name rather than rejecting it", () => {
    // Hot module replacement re-runs the deck's setup module on every edit.
    // Throwing on the second pass would make islands unusable in dev for a
    // mistake nobody made.
    const registry = createRegistry();
    const first = definition("vue");
    const second = definition("vue");

    registry.register(first);
    registry.register(second);

    expect(registry.lookup("vue")).toBe(second);
    expect(registry.names()).toEqual(["vue"]);
  });

  it("rejects a definition with no name", () => {
    const registry = createRegistry();

    expect(() => registry.register(definition(""))).toThrow(TypeError);
  });

  it("rejects a name that is only whitespace", () => {
    const registry = createRegistry();

    expect(() => registry.register(definition("   "))).toThrow(/needs a name/);
  });

  it("rejects a definition with no mount function", () => {
    // Caught at registration, in the deck's setup module, rather than deep in
    // a mount on the slide that happens to use it.
    const registry = createRegistry();
    const broken = { name: "vue" } as unknown as IslandDefinition;

    expect(() => registry.register(broken)).toThrow(/no mount function/);
  });

  it("names the island in the mount-function error", () => {
    const registry = createRegistry();
    const broken = { name: "three" } as unknown as IslandDefinition;

    expect(() => registry.register(broken)).toThrow(/"three"/);
  });
});

describe("listing names", () => {
  it("sorts them, so a message reads the same run to run", () => {
    const registry = createRegistry([definition("vue"), definition("three"), definition("react")]);

    expect(registry.names()).toEqual(["react", "three", "vue"]);
  });

  it("lists a replaced name once", () => {
    const registry = createRegistry([definition("vue"), definition("vue")]);

    expect(registry.names()).toEqual(["vue"]);
  });
});

describe("the unknown-island message", () => {
  it("names what is registered", () => {
    // The whole point: "unknown island vue" sends an author to their
    // frontmatter. "registered: react, three" tells them the frontmatter is
    // fine and the setup module is not.
    const message = unknownIslandMessage("vue", ["react", "three"]);

    expect(message).toContain('unknown island "vue"');
    expect(message).toContain("registered: react, three");
  });

  it("says so plainly when nothing is registered at all", () => {
    const message = unknownIslandMessage("vue", []);

    expect(message).toContain("no islands are registered");
    expect(message).not.toContain("registered:");
  });

  it("points at a near miss in a different case", () => {
    // `Vue` is what a person writes when they are thinking about the framework
    // rather than about a token.
    const message = unknownIslandMessage("Vue", ["react", "vue"]);

    expect(message).toContain('did you mean "vue"?');
  });

  it("offers no suggestion when nothing is close", () => {
    const message = unknownIslandMessage("angular", ["react", "vue"]);

    expect(message).not.toContain("did you mean");
  });

  it("quotes the name exactly as the markup wrote it", () => {
    // Trimmed for matching, not for reporting: an author searching their deck
    // for the string in the message has to be able to find it.
    const message = unknownIslandMessage(" vue ", ["react"]);

    expect(message).toContain('unknown island " vue "');
  });

  it("is what a registry reports for a name it does not have", () => {
    const registry = createRegistry([definition("react")]);

    expect(registry.lookup("vue")).toBeUndefined();
    expect(unknownIslandMessage("vue", registry.names())).toContain("registered: react");
  });
});
