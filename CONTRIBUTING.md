# Contributing

## Method

Test-first. Write the test that states the behaviour, watch it fail for the
right reason, then make it pass. Every behavioural claim in this repository is
a test — that is what makes the maintenance automatable.

A test name is a sentence about behaviour, not a label for a function:

```rust
#[test]
fn a_separator_inside_a_fenced_code_block_is_not_a_slide_break() { … }
```

Prefer a test that would catch a real stage failure over one that raises a
coverage number.

## Layout conventions

**No `mod.rs`.** Use the 2018 path style: a module lives in `foo.rs` and its
children in `foo/`. A directory full of files called `mod.rs` is unnavigable in
an editor's file switcher.

```
src/parser.rs
src/parser/segment.rs
```

**Files stay under about 400 lines of implementation.** A guideline rather
than a rule — `vp check` warns and never fails on it. Past that a module is
usually holding two ideas, so treat the warning as a prompt to look: split by
responsibility, and leave a long file that does one thing well alone.

Tests do not count. A test module is a list, not an abstraction, and splitting
one to hit a number makes it harder to read. Every `#[cfg(test)]` item is
discounted, not just a `mod tests` at the bottom — four files here declare a
test-only helper partway down and carry on implementing below it.

**One reason to exist per module.** `scanner.rs` is the only place that knows
what a fenced code block is. `markers.rs` is the only place that knows the
anchor contract. When a rule appears in two files, one of them is wrong.

## Comments

Document the constraint, not the mechanism. A comment earns its place by
explaining something the code cannot show: why a default is what it is, which
failure a branch exists to prevent, what an external contract requires.

```rust
// An element whose first mention is a reveal starts off screen. Anything
// else — hidden later, emphasised — was authored into the slide body and
// starts visible.
```

Not `// set visibility to hidden`.

Module-level docs say what the module is _for_ and what would break without
it. That context is what makes the file navigable six months later.

## Checks

[Vite+](https://voidzero.dev) is the task runner for the whole repository —
Rust and TypeScript alike. One command runs exactly what CI runs:

```bash
vp run workspace:ci
```

The pieces, when you want one of them:

```bash
vp check
```

```bash
vp test
```

```bash
vp fmt
```

Each of those delegates into the task graph, so `vp check` covers `cargo fmt`,
`clippy`, `oxfmt`, type-aware `oxlint`, and the layout conventions — not just
the TypeScript half. `vp run` with no argument lists every task.

Narrow a run with a filter or a path:

```bash
vp run --filter @slidx/runtime test:ts
```

CI runs `vp run workspace:ci` on Linux, macOS, and Windows and nothing else, so
a check that exists locally cannot be missing from CI. Warnings are errors.
The task graph lives in [vite.config.ts](./vite.config.ts).

## Commits

Conventional commits, scoped to the crate or package:

```
feat(core): declarative step pipeline
fix(lint): measure font size after theme scaling
```

The body explains the decision, not the diff. A reviewer can read the diff.
