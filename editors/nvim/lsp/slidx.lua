-- The slidx language server, as Neovim's own `lsp/` mechanism wants it.
--
-- This is a configuration, not a plugin, and that is the whole point. Neovim
-- 0.11 reads `lsp/<name>.lua` off the runtimepath and `nvim-lspconfig` is a
-- directory of exactly these files, so a plugin of our own would be a wrapper
-- around a table anybody can write — one more thing to install, version and
-- keep in step, buying nothing.
--
-- Put it on the runtimepath and switch it on:
--
--     vim.opt.runtimepath:append("/path/to/slidx/editors/nvim")
--     vim.lsp.enable("slidx")
--
-- Or copy this file to `~/.config/nvim/lsp/slidx.lua`, which is the same thing
-- without the clone. It is written to be submitted upstream unchanged.
--
-- ## Why `markdown` and not a filetype of our own
--
-- A deck is Markdown. A `slidx` filetype would take these buffers away from
-- every Markdown thing already set up for them — the syntax, the folds, the
-- conceal rules, treesitter — in exchange for a highlighter slidx would then
-- have to write and keep in step with Rust enums no syntax file can read.
--
-- So the client attaches to Markdown, and the *server* decides which buffers
-- are decks: Markdown under a `slides/` directory, and nothing else. A README
-- is opened and never answered for. That rule lives in the server because Zed
-- cannot express it in a client at all, and one rule with a test beats three
-- restatements.
--
-- To scope it here as well, replace `vim.lsp.enable` with an autocommand on
-- `*/slides/*.md`. It saves the traffic and changes no answer.

return {
  -- One binary, one subcommand. slidx ships `slidx` and nothing beside it, so
  -- there is no `slidx-lsp` to look for and no second thing to put on a PATH.
  cmd = { "slidx", "lsp" },
  filetypes = { "markdown" },

  -- The deck's project, in the order a deck is usually found in one: the slide
  -- directory itself, then the pin that says which slidx this deck was written
  -- against, then the build that turns it into a talk.
  root_markers = { "slides", ".slidx-version", "vite.config.ts", "vite.config.js", ".git" },
}
