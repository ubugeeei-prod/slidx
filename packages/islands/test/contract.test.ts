/**
 * The markup half of the island contract.
 *
 * These names are a wire format between two languages: the Rust compiler
 * writes them into the slide and this package reads them back in the browser.
 * Nothing type-checks across that boundary, so renaming one silently produces
 * a deck where every island is inert and nothing is reported — the failure
 * mode where a slide simply has a hole in it and no console output.
 *
 * Pinning the strings is the only check there is. A change here is a change to
 * a published format and has to be made on both sides at once.
 */

import { describe, expect, it } from "vite-plus/test";

import { ISLAND_ATTRIBUTE, PROPS_ATTRIBUTE, STATE_ATTRIBUTE } from "../src/contract";

describe("the attributes the compiler writes", () => {
  it("marks an island with data-slidx-island", () => {
    expect(ISLAND_ATTRIBUTE).toBe("data-slidx-island");
  });

  it("carries props in data-slidx-island-props", () => {
    expect(PROPS_ATTRIBUTE).toBe("data-slidx-island-props");
  });

  it("reflects the lifecycle in data-slidx-island-state", () => {
    expect(STATE_ATTRIBUTE).toBe("data-slidx-island-state");
  });

  it("keeps every attribute under the slidx namespace", () => {
    // A deck's slides are also somebody's HTML. Nothing here may collide with
    // an attribute an author wrote.
    for (const attribute of [ISLAND_ATTRIBUTE, PROPS_ATTRIBUTE, STATE_ATTRIBUTE]) {
      expect(attribute.startsWith("data-slidx-")).toBe(true);
    }
  });

  it("keeps the three attributes distinct", () => {
    expect(new Set([ISLAND_ATTRIBUTE, PROPS_ATTRIBUTE, STATE_ATTRIBUTE]).size).toBe(3);
  });
});
