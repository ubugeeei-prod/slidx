---
title: The slidx command
summary: Every command the binary has, and the three it deliberately refuses.
section: reference
order: 6
---

# The slidx command

The binary is separate from the build and optional. It does three things, and
none of them is building a deck: **writing** one when you would rather not leave
the terminal, **the room** — what is about to happen to your talk that no editor
can see — and **the decks you already have**, because a speaker who gives four
talks a year has four repositories and remembers where none of them are.

Everything on this page comes from
[the table the binary itself reads](../../crates/slidx_cli/src/command/table.rs).
The argument parser, the help text and six shell completion scripts read the
same one, and a test upstream fails when the table and the dispatcher disagree —
so this page cannot describe a flag the parser does not accept, which is the
usual way a command-line reference goes wrong.

`-h` is accepted everywhere and is left off the tables below for that reason.

## Installing it

There is no published binary yet. Until the first release:

```bash
cargo build --release -p slidx_cli
./target/release/slidx --help
```

Both install channels named in the [README](../../README.md) — the shell script
and the npm wrapper — hand over the same prebuilt binary and neither has
anything to hand over yet.

## The commands

<!-- slidx-docs: commands -->

## The commands slidx does not have

Each of these is a reasonable thing to type. Answering with "unknown command"
would leave you believing the tool cannot do a thing it does — so the refusal
says what to type instead, and why.

<!-- slidx-docs: declined -->

There is deliberately no `slidx build`. That is `@slidx/vite-plugin`'s job, and
one pipeline is the whole point: the deck you check is the deck you hand over,
because nothing else can render one.

## Two that need a shell to help

`slidx cd` prints a path rather than changing directory, and that is not a
limitation waiting to be fixed: **a child process cannot change the working
directory of the shell that started it.** So it resolves, and a shell function
enters — the same pair every directory jumper is built out of. `slidx shell`
writes that function for your shell.
