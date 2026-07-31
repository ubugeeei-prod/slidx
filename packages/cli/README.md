# slidx

The [slidx](https://github.com/ubugeeei-prod/slidx) command line: the pre-flight
a speaker runs in the room, and the linter their CI runs.

```bash
npm i -g slidx
```

A prebuilt binary for your platform, no toolchain required. The installer at
[install.sh](https://github.com/ubugeeei-prod/slidx/blob/main/install.sh) and
`cargo install slidx_cli` produce the same binary.

`slidx self-update` downloads the newest stable release for this target,
verifies it against the release's `SHA256SUMS`, and starts it once before
installing it. A binary that npm, cargo or a system package manager owns stays
theirs: it refuses and prints that channel's command rather than putting a
second `slidx` later on `PATH`.

## The commands, in the order a talk needs them

```text
slidx dev                  # the deck and the visual editor, from a slides directory
slidx fmt                  # normalise what slidx owns, and nothing you wrote
slidx lint                 # every rule the build runs, non-zero on anything blocking
slidx export --target pdf  # browser | pdf | pdf-zip | png | pptx
slidx doctor               # power, clock, fonts, screen capture, mirroring, Do Not Disturb
slidx publish              # all that needs no account, and the payload for what does
```

A speaker keeps several decks in several repositories, so slidx indexes them:

```text
slidx list                 # every deck this machine has seen
slidx grep "venue wifi"    # searches them all, and answers in slides
slidx cd vueconf           # with `slidx shell` loaded, takes you there
```

`slidx shell <name>` prints the function that makes `cd` move your shell —
a child process cannot move the shell that started it, so the command resolves
and the function enters. `slidx completions <name>` prints completions. Both
know sh, bash, zsh, fish, nushell, PowerShell and ush; plain `sh` gets the
function and says why it cannot have completions.

## What it is not

It is not the build. A deck is built by
[`@slidxjs/vite-plugin`](https://www.npmjs.com/package/@slidxjs/vite-plugin) in
the project that owns it, and `slidx export` runs that build rather than
rendering a deck a second way — so what you hand over is what your CI produced.

There is no credential store and no HTTP client under `slidx publish`. It
composes what a platform wants, writes what belongs on disk, and names the page
to paste the rest into.

## Documentation

https://github.com/ubugeeei-prod/slidx#readme

## License

MIT. The notice is in this package, and at
https://github.com/ubugeeei-prod/slidx/blob/main/LICENSE.
