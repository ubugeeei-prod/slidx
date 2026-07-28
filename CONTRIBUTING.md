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

**Files stay under about 250 lines.** When one grows past that, it is usually
holding two ideas. Split by responsibility, not by line count.

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

Module-level docs say what the module is *for* and what would break without
it. That context is what makes the file navigable six months later.

## Checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test
pnpm typecheck
```

All five run in CI on Linux, macOS, and Windows. Warnings are errors.

## Commits

Conventional commits, scoped to the crate or package:

```
feat(core): declarative step pipeline
fix(lint): measure font size after theme scaling
```

The body explains the decision, not the diff. A reviewer can read the diff.
