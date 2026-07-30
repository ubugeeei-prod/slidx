/**
 * The frame a recording of the editor is taken through.
 *
 * A recording of a canvas is a recording of a slide: it shows something moving
 * and says nothing the words did not. What cannot be claimed in prose is the
 * canvas and the **Markdown moving together** — a block crossing a region
 * boundary while the file gains the one line that says so. So the picture has to
 * hold two things at once, and the editor is not a two-pane view of itself: its
 * canvas and its Markdown are the same panel, and only one of them is up.
 *
 * This is that frame, and everything in it is real:
 *
 * **The editor** is the dev server's own `/__slidx/` in a frame, at a real size,
 * so the canvas inside it is the deck's page rendered by the same WebAssembly
 * module a build uses. Nothing is drawn here that the editor draws.
 *
 * **The file** is bytes read off disk between one gesture and the next, by the
 * driver, from the deck the editor is writing to. Not a rendering of a panel
 * that does not exist — the file, as `git diff` would find it a moment later.
 *
 * The two are labelled, because a reader has to be able to tell what they are
 * looking at. This is documentation's own frame around two real things, in the
 * way `scripts/terminal.mjs` draws a real byte stream: what is captured decides
 * the content, and the page only decides where it sits.
 *
 * # Why the pointer is drawn
 *
 * A screenshot does not contain one. The operating system draws the cursor
 * outside the page, so a frame captured mid-drag has the ghost, the guides and
 * the drop line in it and nothing to say what moved them — the gesture happens
 * and reads as the interface twitching on its own.
 *
 * So the stage draws a pointer at the coordinate the driver drove the mouse to.
 * It is a record of the gesture rather than an illustration of one: the position
 * is the position, and it is set from the same numbers Playwright was given. In
 * the brand's own ink, because the alternative is a screenshot of one particular
 * operating system's cursor.
 *
 * # Why a window onto the editor rather than the whole of it
 *
 * A gesture happens in one or two panels, and the rest of the chrome is 300
 * pixels of stillness that every clone of this repository pays for forever. So a
 * scene names the panels it is about and the stage is a window onto exactly
 * those, at the size the editor really lays them out — the same crop
 * `scripts/screenshot.mjs` takes when it photographs the slide rather than the
 * letterboxing around it.
 */

/** The file beside the editor, wide enough for a deck line and no wider. */
const FILE_WIDTH = 320;

/**
 * The page, with a window onto the editor and room for the file.
 *
 * The window's rectangle is not known here: it is measured inside the editor
 * once it has laid out, and the driver sets it. Guessing it from the chrome's
 * grid would be a second copy of the editor's layout, kept in a script that has
 * no way to notice when it stops being true.
 *
 * `tokens` comes from the crates that generate the brand, the same way
 * `terminal.mjs` is handed them: a second reader of those files would be a
 * second place that knows where they live.
 */
export function stagePage(editorUrl, { tokens, editorWidth, editorHeight, withFile }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>slidx editor</title>
<style>
${tokens}
${STYLESHEET}
</style>
</head>
<body>
<div class="stage" data-file="${withFile ? "true" : "false"}">
  <!--
    The window starts the size of the editor, so the whole of it is on screen and
    answering a pointer before a scene has said which panels it wants. A window
    with no size clips an absolutely positioned frame to nothing, and an element
    clipped to nothing cannot be clicked.
  -->
  <div class="window" style="width: ${editorWidth}px; height: ${editorHeight}px">
    <iframe
      class="editor"
      title="The slidx editor"
      style="width: ${editorWidth}px; height: ${editorHeight}px"
      src="${editorUrl}"
    ></iframe>
  </div>
  <aside class="file">
    <p class="file-name"></p>
    <ol class="file-lines"></ol>
  </aside>
</div>
<svg class="pointer" viewBox="0 0 12 19" width="12" height="19" aria-hidden="true" hidden>
  <path d="M1 1 L1 15 L4.6 11.6 L7 17.4 L9.6 16.3 L7.2 10.7 L11.6 10.4 Z" />
</svg>
<script type="module">
${SCRIPT}
</script>
</body>
</html>
`;
}

export { FILE_WIDTH };

/**
 * A file's lines, with the ones that were not there before marked.
 *
 * One gesture on the canvas is one operation and one splice, so what a reader
 * has to be able to see is which line it wrote. Matched from both ends rather
 * than diffed: what these recordings show is an insertion, and the shortest
 * honest description of one is everything the two texts do not already agree
 * about.
 */
export function mark(before, after) {
  const now = after.split("\n");
  // Nothing is a change against a file nobody has been shown yet: the first
  // frame is the deck as the author saved it.
  if (before === "") return now.map((text) => ({ text, added: false }));

  const was = before.split("\n");

  let head = 0;
  while (head < was.length && head < now.length && was[head] === now[head]) head += 1;

  let tail = 0;
  while (
    tail < was.length - head &&
    tail < now.length - head &&
    was[was.length - 1 - tail] === now[now.length - 1 - tail]
  ) {
    tail += 1;
  }

  return now.map((text, at) => ({ text, added: at >= head && at < now.length - tail }));
}

/**
 * Flat, hairlines, and every colour a brand token.
 *
 * The stage sits around the editor's own chrome, so anything it draws with an
 * opinion of its own would read as part of the tool. `check-flat.mjs` and
 * `check-borrowed.mjs` both scan this file, which is the reason there is not a
 * hex literal in it.
 */
const STYLESHEET = `
* { box-sizing: border-box; }

body {
  margin: 0;
  overflow: hidden;
  background: var(--slidx-brand-paper);
  color: var(--slidx-brand-ink);
  font-family: var(--slidx-brand-font-sans);
}

.stage { display: flex; }

/*
 * A window onto the editor, which is laid out at its own size behind it.
 *
 * The frame is not resized to the crop: the editor's panels are what is being
 * photographed, and a narrower viewport would photograph a narrower editor.
 */
.window { overflow: hidden; position: relative; }
.editor { position: absolute; border: 0; }

.file {
  width: ${FILE_WIDTH}px;
  flex: none;
  padding: 10px 0 0 0;
  border-left: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
  overflow: hidden;
  font-family: var(--slidx-brand-font-mono);
  font-size: 12.5px;
  line-height: 1.6;
}

.stage[data-file="false"] .file { display: none; }

/*
 * The file's own path, in the file's own case.
 *
 * The panel headings in the editor's chrome are set in small capitals, and this
 * is deliberately not: it is a path somebody could type, and 0005.MD is not one.
 */
.file-name {
  margin: 0 0 8px 0;
  padding: 0 12px;
  color: var(--slidx-brand-muted);
  font-size: 11.5px;
}

.file-lines { margin: 0; padding: 0; list-style: none; white-space: pre-wrap; }

.file-lines li { padding: 0 12px 0 0; }

/*
 * The line the last gesture wrote, marked the way a diff marks one.
 *
 * The mark is the point of the whole picture: one gesture on the canvas is one
 * line in the file, and a reader has to be able to see which line without being
 * told. The tint is the signal at low alpha rather than a fill, so the text on
 * it stays as legible as the text around it.
 */
.file-lines li[data-added="true"] {
  background: color-mix(in srgb, var(--slidx-brand-signal) 16%, transparent);
}

.file-lines .mark {
  display: inline-block;
  width: 12px;
  padding-left: 4px;
  color: var(--slidx-brand-muted);
}

.file-lines li[data-added="true"] .mark { color: var(--slidx-brand-signal); }

/*
 * The pointer, outlined so it is legible over a slide of any colour.
 *
 * Fixed, because the coordinate it is given is the viewport coordinate the mouse
 * was driven to. It is above everything for the same reason a real cursor is.
 */
.pointer {
  position: fixed;
  z-index: 10;
  pointer-events: none;
  fill: var(--slidx-brand-ink);
  stroke: var(--slidx-brand-paper);
  stroke-width: 1.4;
}
`;

/**
 * The two things the driver sets, and nothing else.
 *
 * No state and no timers: every frame of a recording is taken after a deliberate
 * call, so anything here that moved on its own would be motion nobody authored
 * and a file that changes on every regeneration.
 */
const SCRIPT = `
const frame = document.querySelector(".editor");
const name = document.querySelector(".file-name");
const lines = document.querySelector(".file-lines");
const pointer = document.querySelector(".pointer");

window.slidxStage = {
  /** Where the mouse is. Hidden until a gesture has moved it somewhere. */
  pointer(x, y) {
    pointer.hidden = x === undefined;
    if (x === undefined) return;

    // The tip of the arrow is the hot spot, which is what the coordinate means.
    pointer.style.left = x + "px";
    pointer.style.top = y + "px";
  },

  /** Puts the window over one rectangle of the editor's own layout. */
  crop({ x, y, width, height }) {
    const pane = document.querySelector(".window");
    pane.style.width = width + "px";
    pane.style.height = height + "px";
    frame.style.left = -x + "px";
    frame.style.top = -y + "px";
    document.querySelector(".file").style.height = height + "px";
  },

  /** The deck file as it is on disk, with the lines a gesture just wrote marked. */
  file(path, rows) {
    name.textContent = path;
    lines.replaceChildren(
      ...rows.map((row) => {
        const item = document.createElement("li");
        item.dataset.added = String(row.added === true);

        const mark = document.createElement("span");
        mark.className = "mark";
        mark.textContent = row.added === true ? "+" : " ";

        item.append(mark, document.createTextNode(row.text));
        return item;
      }),
    );
  },
};
`;
