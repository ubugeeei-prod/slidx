---
title: Your editor
summary: Diagnostics, completion, the deck outline, hover and formatting, in VS Code, Zed and Neovim.
section: reference
order: 8
---

# Your editor

slidx has a language server. It gives you, while you type:

- **Diagnostics** — every rule `slidx lint` runs, with its remedy attached. A
  contrast pair a projector will wash out, body text row twelve cannot read, an
  image without alt text, a `steps:` entry naming a mark that is not there.
- **Completion** — frontmatter keys, theme names, transitions, step presets,
  layouts, aspect ratios. All of them read from the Rust that defines them, so a
  preset added to slidx is offered without anybody editing an editor plugin.
- **The deck outline** — one entry per slide, with its stops, its budget and
  whether you marked it optional; the steps nested underneath.
- **Hover** — what a frontmatter key expects, and what a preset will actually do
  on screen, including whether it will stay on the compositor.
- **Formatting** — `slidx fmt` on save, as a handful of small edits rather than a
  rewrite of the file. Your prose, your wrapping and your bullet markers are not
  touched.

It is `slidx lsp`, a subcommand of the one binary. There is no `slidx-lsp` to
install: one binary is one thing on your PATH, one release asset, and one
version that a project's `.slidx-version` pin applies to.

## Which files it is for

**Markdown under a `slides/` directory.** Nothing else.

That is the layout the Vite plugin builds by default and the path `slidx lint`,
`slidx fmt` and `slidx dev` all fall back to. It is also the narrowest rule that
is honest: a deck is Markdown, most Markdown is not a deck, and the only way to
tell a single `talk.md` from a README is to open every Markdown file you have
and read it. An editor plugin that put slide diagnostics on your changelog would
deserve to be uninstalled.

Two consequences worth knowing before you wonder why nothing is happening.

A deck kept as **one file at the top of a project** is not picked up. Move it
under `slides/` — which is what makes it a deck to the plugin that builds it,
too.

A project that pointed the plugin's `srcDir` somewhere else is not picked up
either. The server is handed a file path and knows nothing about the Vite config
it belongs to, and guessing from a directory name would be the same overreach.

The rule lives in the **server**, not in the plugins. Zed can only attach a
language server to a whole language, so its client cannot express a path rule at
all — which means a rule stated in each client would be a rule two of them keep
and one cannot. Editors that _can_ filter do, because it saves sending the file
at all, but nothing depends on their doing so.

## No syntax highlighting, on purpose

None of these plugins contributes a language, a grammar or a file association.
Your decks stay Markdown files with your Markdown tooling on them: your preview,
your table formatter, your folds, your treesitter.

The alternative was a `slidx` language with a highlighter of its own, and it
would go stale. Completion knows the presets, transitions, themes and layouts
because it reads the Rust that defines them; a TextMate grammar or a Vim syntax
file cannot read Rust, so every one of those lists would be typed out a second
time and would be wrong the first time somebody added a variant — silently, with
no test anywhere to notice.

The dialect's own constructs are already ordinary Markdown to a highlighter.
`<!-- step: fade -->` is a comment, `[3.2x faster]{#result}` is a link-like span,
`---` is a rule. They highlight as what they are.

## Finding the binary

Every plugin here looks in the same order.

1. **What you configured**, if you configured anything. Taken as given — a
   setting that quietly fell back to something else is how somebody debugs the
   wrong binary for an hour.
2. **`slidx` on your PATH.** This is what your own terminal runs, and therefore
   what `slidx version use` and a project's `.slidx-version` act on.
3. **The install directory** — `$SLIDX_HOME`, else `$XDG_DATA_HOME/slidx`, else
   `~/.slidx`, and `%LOCALAPPDATA%\slidx` on Windows. The same order
   `install.sh` writes in and `slidx version` manages.

Step 3 exists because an editor is not a login shell: an application started
from a dock has whatever PATH the session manager gave it, which on macOS is
usually not the one in your profile. Zed skips it, and correctly — it resolves
through the project's shell environment already, so its answer is your
terminal's answer.

**When nothing is found**, the plugin says so, once, naming the places it looked
and how to fix it. It does not start a server and it does not fail silently: a
language server that never starts is indistinguishable from one that has nothing
to say, which is the one thing this must never look like. To find out what your
machine thinks is going on:

```bash
slidx version current
```

That prints the file that is actually running, which install channel put it
there, and whether anything else on your PATH is shadowing it.

## VS Code

The extension is in [packages/vscode](../../packages/vscode). It is not on the
Marketplace yet — slidx has no release — so today it is installed from a build:

```bash
vp run build:vscode
npx @vscode/vsce package --out slidx.vsix
code --install-extension slidx.vsix
```

It wakes up when a workspace contains `slides/*.md` and not before, so a window
with no deck in it starts nothing.

One setting, for the one thing nobody can guess for you:

```json
{ "slidx.path": "/opt/built/slidx" }
```

Leave it empty and the order above applies.

To make slidx the formatter for your decks without making it the formatter for
every Markdown file you own:

```json
{
  "[markdown]": { "editor.formatOnSave": true },
  "editor.defaultFormatter": "esbenp.prettier-vscode"
}
```

VS Code asks which formatter to use the first time two of them offer, and slidx
only ever offers for a file under `slides/`.

## Zed

The extension is in [editors/zed](../../editors/zed). Zed extensions are
compiled by Zed itself, so there is nothing to build first:

1. **Extensions → Install Dev Extension**
2. Choose `editors/zed` in your clone.

It attaches to Markdown, because that is the finest granularity Zed offers, and
the server declines everything that is not a deck.

To point it at a particular binary, in your Zed settings:

```json
{ "lsp": { "slidx": { "binary": { "path": "/opt/built/slidx" } } } }
```

## Neovim

There is no slidx plugin for Neovim, and there should not be. Neovim 0.11 reads
`lsp/<name>.lua` off the runtimepath and `nvim-lspconfig` is a directory of
exactly those files, so a plugin here would be a wrapper around a table you can
write yourself — one more thing to install, version and keep in step, buying
nothing.

The table is [editors/nvim/lsp/slidx.lua](../../editors/nvim/lsp/slidx.lua), and
it is short enough to read here:

```lua
return {
  cmd = { "slidx", "lsp" },
  filetypes = { "markdown" },
  root_markers = { "slides", ".slidx-version", "vite.config.ts", "vite.config.js", ".git" },
}
```

Copy it to `~/.config/nvim/lsp/slidx.lua` and switch it on:

```lua
vim.lsp.enable("slidx")
```

Or put the clone on the runtimepath instead of copying, which is the same thing
with an update path:

```lua
vim.opt.runtimepath:append("/path/to/slidx/editors/nvim")
vim.lsp.enable("slidx")
```

**No new filetype.** A deck is a `markdown` buffer and stays one, for the reason
above: a `slidx` filetype would cost you every Markdown thing already set up for
it. The client attaches to Markdown and the server decides which buffers are
decks.

If you would rather it never attached to a README at all, use an autocommand in
place of `vim.lsp.enable` — it saves the traffic and changes no answer:

```lua
vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
  pattern = "*/slides/*.md",
  callback = function(event)
    vim.lsp.start(vim.lsp.config.slidx, { bufnr = event.buf })
  end,
})
```

Formatting on save, for decks only:

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = "*/slides/*.md",
  callback = function() vim.lsp.buf.format({ name = "slidx" }) end,
})
```

## What it is not

It is not a second implementation of anything. Every diagnostic carries the code
and the remedy `slidx lint` prints, every formatting edit is a splice
`slidx fmt` would have written, and every completion list is read out of the Rust
that defines the thing being completed. An editor showing you something
`slidx lint` disagrees with would be a bug, not a difference of opinion.

And it says nothing about how your deck _looks_. Whether type is legible from row
fifteen and whether a colour pair survives a projector are questions about a
rendered slide; the linter answers them, and it runs here — but nothing in an
editor pane is evidence about a room. `slidx lint` and a real browser are still
what settle that.
