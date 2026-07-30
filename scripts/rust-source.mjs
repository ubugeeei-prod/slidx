/**
 * Which part of a Rust file is implementation.
 *
 * Two checks need this and both had their own answer: find the first
 * `#[cfg(test)]` and treat everything after it as tests. That is right for the
 * common shape — a `mod tests` at the bottom — and wrong for the five files in
 * this workspace that put a test-only helper somewhere in the middle and carry
 * on implementing below it. `slidx_lint/src/lib.rs` declares
 * `#[cfg(test)] mod test_support;` on line 52 of 319, so the size guideline was
 * measuring a 229-line file as 51 lines, and the dead-config check was reading
 * a fifth of the file it thought it had read.
 *
 * So an attribute suppresses one item, not the rest of the file. Nothing here
 * parses Rust: it counts braces, which is enough for the shapes a `#[cfg(test)]`
 * ever attaches to and is honest about being a heuristic. A brace inside a
 * string literal would fool it, and the failure would be to drop too much —
 * which shows up as a check complaining, never as one going quiet.
 */

const CFG_TEST = /^#\[cfg\(test\)\]/;
const ATTRIBUTE = /^#[!\[]/;

/**
 * `source` with every `#[cfg(test)]` item removed, and how many lines are left.
 *
 * Lines are blanked rather than dropped so a caller can still report a line
 * number that matches the file someone opens in an editor. `lines` therefore
 * counts what survived rather than what the text now measures, and it counts
 * blank lines the way the size guideline always has — this is a fix to which
 * lines are tests, not a re-definition of the number.
 */
export function implementation(source) {
  const lines = source.split("\n");
  const kept = [...lines];
  const removed = new Set();

  for (let index = 0; index < lines.length; index += 1) {
    if (!CFG_TEST.test(lines[index].trimStart())) continue;

    let cursor = index;
    // Other attributes may sit between the `cfg` and the item it guards.
    while (cursor < lines.length && ATTRIBUTE.test(lines[cursor].trimStart())) cursor += 1;

    let depth = 0;
    let opened = false;

    for (; cursor < lines.length; cursor += 1) {
      for (const character of lines[cursor]) {
        if (character === "{") {
          depth += 1;
          opened = true;
        } else if (character === "}") {
          depth -= 1;
        }
      }

      // A declaration with no block — `mod test_support;` — ends on its own
      // line, and a block ends when its braces balance.
      if (opened ? depth <= 0 : lines[cursor].trimEnd().endsWith(";")) break;
    }

    for (let blank = index; blank <= Math.min(cursor, lines.length - 1); blank += 1) {
      kept[blank] = "";
      removed.add(blank);
    }

    index = cursor;
  }

  return { text: kept.join("\n"), lines: lines.length - removed.size };
}
