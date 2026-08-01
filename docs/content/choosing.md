---
title: Choosing slidx for a talk
summary: What it covers, what it refuses to do, and where it actually is today.
section: choosing
order: 1
---

# Choosing slidx for a talk

You have a slot in a few weeks and you are deciding what to write it in. This
page is the argument and the caveats, in that order, and it does not hide the
second half.

If you would rather see it than read about it, [Start](index.md) gets you to a
built deck in twenty minutes.

## What it is for

Making the slides is the short part of giving a talk. slidx is organised around
the long part — the week before, the room, the twenty minutes on stage, and the
fortnight afterwards when publishing never quite happens.

| When             | What slidx does about it                                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Writing          | Markdown, and a visual editor over the same file. An edit is a byte-range splice, so the diff stays reviewable.                                                                |
| Writing          | A language server: diagnostics as you type, completion for frontmatter keys and step presets, the deck outline.                                                                |
| Before you leave | A linter that models the room — projector washout, legibility from the back row, overflow measured in a real browser, time against the slot.                                   |
| The morning of   | `slidx doctor`: power, disk, clock skew, the fonts your theme names, whether anything is recording the screen, whether the network works.                                      |
| On stage         | A presenter view with the clock as the largest thing on it, a behind-or-ahead reading against your per-slide budgets, and the optional slides named when you need to drop one. |
| On stage         | A declared demo fallback — the live target and a recording of it working, both already in the markup, switched by one key.                                                     |
| On stage         | A shared code fence published as its own page in your deck's own output, with a QR on the slide pointing at it.                                                                |
| Afterwards       | `slidx publish` plans every destination from the frontmatter you wrote at proposal time and performs everything that needs no account.                                         |
| Months later     | `slidx list`, `grep`, `cd` and `open` over the decks this machine has seen, because you will not remember which repository the talk was in.                                    |

The [README](../../README.md) has the long version of that list, and
[ROADMAP.md](../../ROADMAP.md) has the honest one: every unchecked line there
says _why_ it is not done, and it opens by defining what a checked box is allowed
to mean.

## What you are signing up for

These are properties, not settings. If one of them is wrong for your talk, it is
wrong all the way down and no configuration will move it.

**Your source is Markdown in your repository.** There is no binary deck format
and there is no hosted editor. The visual editor runs on your machine and writes
to the file you have open.

**The output is pages, not an application.** One HTML document per slide.
Navigation is the browser following a link, so a slide can be shared,
bookmarked, indexed and printed — and it renders before any script runs.

**A remote asset is a build error.** Fonts, images, styles and scripts are
inlined or bundled. A deck that reaches for a CDN does not build. This is the
single most opinionated thing in the project and it is not adjustable, because
the failure it prevents happens in a room where you cannot fix it.

**The build is Vite.** `@slidxjs/vite-plugin` is the whole configuration and there
is deliberately no `slidx build` — one pipeline is the point. The `slidx` binary
is separate and optional, and does the things a build cannot.

**Nothing publishes as you.** There is no HTTP client and no token store under
`slidx publish`. It composes the payload, writes what belongs on disk, and names
the page you paste the rest into. That is a property rather than an omission: a
tool that can post as you is a tool that has to be trusted with a credential.

## Where it actually is

**Pre-alpha, and unreleased.** Nothing is on npm or crates.io. There is no
tagged release and no published binary. Running it means cloning the repository,
which [Start](index.md) walks through.

Everything the README marks as shipped is built and tested and reachable by a
person — that last clause is doing real work, and
[ROADMAP.md](../../ROADMAP.md#what-a-checked-box-means-here) explains why it had
to be spelled out. The parts that are not done are open issues with a stated
shape rather than a wish list.

What is verified rather than assumed: the deck renders in Chromium, Firefox and
WebKit; the pipeline runs on Node, Bun and Deno, and on macOS, Linux and Windows.
Each of those is a job in CI, not a claim.

## Reasons to use something else

**You need to install it today.** There is no registry package. If you cannot
build from a clone, this is not usable for your talk in three weeks.

**You cannot run a local toolchain.** slidx is a build, on your machine or in
your CI. Nothing here is hosted.

**Your deck has to be one scrolling document.** slidx compiles to one page per
slide and the whole design leans on that.

**Your slides are your design tool's output.** slidx renders Markdown through a
theme. Arbitrary free-form layout is not what it is for, and a deck that is
really a set of drawings will fight it the whole way.

**You need it to control the machine.** A deck is a page, and the page does not
yet reach for fullscreen or hold a wake lock — the code for both is written and
nothing on a slide calls it, which is a gap rather than a decision. Use your
browser's own fullscreen for now.

It will not turn on Do Not Disturb or set your volume even then, because no
browser API does and none should — a page that could mute your machine could
hide a phishing alert. Those are a checklist naming the setting and where it
lives on your platform.

## Three weeks out, concretely

**Today, twenty minutes.** Work through [Start](index.md). You will know by the
end whether the format suits how you write.

**While you write.** Keep `vite dev` running. The linter is in the build, so the
things that are invisible on a laptop are reported while you can still act on
them, not the night before.

**A week out.** Run `slidx doctor` on the machine you will actually speak from —
not the one you wrote the talk on, if they are different. Rehearse once with the
presenter view open: the clock runs against the slot your frontmatter declared,
and the pace reading compares where you are against the per-slide budgets, which
is the difference between "you ran over" and "slide 7 took four minutes and was
budgeted one".

**The night before.** [The night before](tonight.md) is indexed by symptom.
Bookmark it now, while you are calm.
