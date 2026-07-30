import { describe, expect, it } from "vite-plus/test";

import { implementation } from "./rust-source.mjs";

describe("implementation", () => {
  it("drops a test module at the bottom of a file", () => {
    const source = ["pub fn one() {}", "", "#[cfg(test)]", "mod tests {", "    fn t() {}", "}", ""];

    const { text, lines } = implementation(source.join("\n"));

    expect(text).not.toContain("mod tests");
    expect(text).toContain("pub fn one()");
    expect(lines).toBe(3);
  });

  it("keeps the implementation that follows a test-only helper", () => {
    // The regression this module exists for. `slidx_lint/src/lib.rs` declares
    // `#[cfg(test)] mod test_support;` on line 52 of 319, and cutting the file
    // at the first attribute measured a 229-line file as 51.
    const source = ["#[cfg(test)]", "mod test_support;", "", "pub fn after() -> u8 { 1 }"];

    const { text, lines } = implementation(source.join("\n"));

    expect(text).not.toContain("test_support");
    expect(text).toContain("pub fn after()");
    expect(lines).toBe(2);
  });

  it("does not end a test module at a brace nested inside it", () => {
    const source = [
      "#[cfg(test)]",
      "mod tests {",
      "    fn t() {",
      "        if true {}",
      "    }",
      "}",
      "pub fn survives() {}",
    ];

    const { text } = implementation(source.join("\n"));

    expect(text).not.toContain("if true");
    expect(text).toContain("pub fn survives()");
  });

  it("skips attributes sitting between the cfg and the item it guards", () => {
    const source = [
      "#[cfg(test)]",
      "#[allow(clippy::unwrap_used)]",
      "mod tests {",
      "    fn t() {}",
      "}",
      "pub fn survives() {}",
    ];

    const { text } = implementation(source.join("\n"));

    expect(text).not.toContain("mod tests");
    expect(text).toContain("pub fn survives()");
  });

  it("blanks rather than deletes, so a line number still points where it did", () => {
    const source = ["#[cfg(test)]", "mod tests {}", "pub fn third() {}"];

    const { text } = implementation(source.join("\n"));

    expect(text.split("\n")[2]).toBe("pub fn third() {}");
  });

  it("leaves a file with no tests exactly as it was", () => {
    const source = "pub fn only() -> u8 {\n    1\n}\n";

    expect(implementation(source)).toEqual({ text: source, lines: 4 });
  });
});
