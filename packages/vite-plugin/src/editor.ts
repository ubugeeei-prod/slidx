/**
 * The visual editor, served by the dev server that already owns the deck.
 *
 * `vite dev` gives the author their deck *and* the editor, with no extra
 * install and no second process. That is a deliberate constraint rather than a
 * convenience: an editor that needed its own server would be a second thing to
 * start, a second port to remember, and a second copy of the deck to disagree
 * with the first.
 *
 * Two things are served, and only in dev — the page, and the module it loads.
 * A build never registers either, which is what keeps something that writes to
 * the author's files off a web server.
 */

import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";

/** The editor's own page, under the same prefix as the routes it talks to. */
export const EDITOR_PAGE = "/__slidx/";
export const EDITOR_MODULE = "/__slidx/editor.js";

/**
 * The page the editor mounts into.
 *
 * Almost nothing: a root element and one module import. The chrome, including
 * its stylesheet, comes from the package, so the plugin never holds an opinion
 * about how the editor looks.
 */
export function editorPage(deckBase: string, title: string | undefined): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${escape(title ?? "slidx")} — editor</title>
</head>
<body>
<div id="slidx-editor"></div>
<script type="module">
import { mount } from "${EDITOR_MODULE}";
mount(document.getElementById("slidx-editor"), { deckBase: ${JSON.stringify(deckBase)} });
</script>
</body>
</html>
`;
}

/** The built editor module, read once. */
let module: Promise<string> | undefined;

export function readEditor(): Promise<string> {
  module ??= (async () => {
    const require = createRequire(import.meta.url);
    return readFile(require.resolve("@slidx/editor"), "utf8");
  })();

  return module;
}

function escape(text: string): string {
  return text.replace(/[&<>]/g, (character) =>
    character === "&" ? "&amp;" : character === "<" ? "&lt;" : "&gt;",
  );
}
