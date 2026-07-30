/**
 * Real terminal output, captured and drawn.
 *
 * The images of the terminal on the documentation site are not typed into a
 * mockup. Each one is `slidx` run for real, its bytes captured, and those bytes
 * drawn — so an image whose command changed its mind fails to reproduce rather
 * than quietly showing a report the tool no longer writes. That is the standard
 * `scripts/screenshot.mjs` set for the slide images, applied to the half of the
 * product that has no browser in it.
 *
 * # Why a pty
 *
 * `slidx` colours its output when stdout is a terminal and not otherwise, which
 * is the right behaviour and makes a plain pipe capture a report with the
 * emphasis stripped out. So the command runs under `script`, which allocates a
 * pty, and what is captured is the same byte stream a person sees.
 *
 * Colour is *converted*, never decided: an SGR code becomes a class, and the
 * class takes a token from the theme already on the page. Nothing here has an
 * opinion about which finding is serious — that opinion is the CLI's, and it
 * already expressed it in the bytes.
 *
 * # What this cannot capture
 *
 * A full-screen program. `slidx tui` paints with cursor addressing and a capture
 * of it is a screen's worth of escape sequences describing motion, not a
 * picture. Nothing here pretends otherwise; the commands below all write a
 * report and exit.
 */

import { execFileSync, spawn } from "node:child_process";

/**
 * Runs one command under a pty and returns what it wrote.
 *
 * `script` is the portable-enough way to get one, and the two spellings are not
 * compatible: BSD takes the command as arguments after the file, util-linux
 * takes it after `-c`. Both are asked to write the typescript to `/dev/null`,
 * because the capture we want is the one on stdout.
 */
export function capture(command, args, options = {}) {
  const [program, argv] = pty(command, args);

  let output;
  try {
    output = execFileSync(program, argv, {
      encoding: "utf8",
      cwd: options.cwd,
      // A non-zero exit is expected: `slidx lint` exits non-zero on a blocking
      // finding, and that run is exactly the one worth a picture.
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    if (error.stdout === undefined) throw error;
    output = error.stdout;
  }

  return clean(output);
}

/**
 * Captures a command that stays alive once it is ready, then stops its process
 * group cleanly. Used for the dev server in the CLI tour: its startup is real,
 * but a recording task cannot leave the server behind.
 */
export function captureUntil(command, args, options) {
  const [program, argv] = pty(command, args);

  return new Promise((resolve, reject) => {
    const child = spawn(program, argv, {
      cwd: options.cwd,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    let ready = false;
    let failure;

    const stop = () => {
      try {
        if (process.platform === "win32") child.kill("SIGINT");
        else process.kill(-child.pid, "SIGINT");
      } catch {
        child.kill("SIGINT");
      }
    };

    // The deadline bounds waiting for readiness, and nothing after it: `script`
    // takes a couple of seconds to reap the pty once it is signalled, so a timer
    // left running through shutdown would fail a capture that already succeeded.
    const timeout = setTimeout(() => {
      failure = new Error(`the command did not print ${options.until} before the timeout`);
      stop();
    }, options.timeout ?? 20_000);

    const read = (chunk) => {
      output += chunk.toString("utf8");
      if (ready || !options.until.test(output)) return;
      ready = true;
      clearTimeout(timeout);
      stop();
    };
    child.stdout.on("data", read);
    child.stderr.on("data", read);

    child.on("error", (error) => {
      failure = error;
      clearTimeout(timeout);
      reject(error);
    });
    child.on("close", () => {
      clearTimeout(timeout);
      if (failure) reject(failure);
      else if (!ready) reject(new Error("the command ended before it was ready"));
      else resolve(cleanLive(output));
    });
  });
}

function pty(command, args) {
  const shell = [command, ...args].map(quote).join(" ");
  return process.platform === "darwin"
    ? ["script", ["-q", "/dev/null", command, ...args]]
    : ["script", ["-qec", shell, "/dev/null"]];
}

/**
 * Removes what the pty added and the command did not write.
 *
 * BSD `script` echoes the end-of-file character before the first line, and a
 * pty ends every line with CRLF. Neither is output; both would be drawn.
 */
function clean(text) {
  return text
    .replace(/^\^D\x08\x08/, "")
    .replaceAll("\r\n", "\n")
    .replace(/\n+$/, "\n");
}

/**
 * A full-screen dev tool clears and redraws its startup report. Keep the final
 * bytes and remove only cursor-addressing instructions; colour remains SGR and
 * is converted by the same renderer as every other terminal capture.
 */
function cleanLive(text) {
  const escape = String.fromCharCode(27);
  const cursor = new RegExp(`${escape}\\[[0-9;?]*[A-HJKSTf]`, "g");

  return clean(text)
    .replaceAll(cursor, "")
    .replaceAll(/\n{3,}/g, "\n\n")
    .trimStart();
}

function quote(argument) {
  return /^[\w./=-]+$/.test(argument) ? argument : `'${argument.replaceAll("'", "'\\''")}'`;
}

/**
 * Every SGR parameter this turns into a class.
 *
 * Only the ones `slidx_cli::style` emits. An unrecognised code is dropped
 * rather than guessed at, which keeps a future style change visible as missing
 * emphasis instead of as a wrong colour.
 */
const SGR = new Map([
  [1, "bold"],
  [2, "dim"],
  [3, "italic"],
  [4, "underline"],
  [31, "red"],
  [32, "green"],
  [33, "yellow"],
  [34, "blue"],
  [35, "magenta"],
  [36, "cyan"],
  [90, "dim"],
]);

/**
 * Captured bytes as HTML.
 *
 * A `<pre>` of spans. No cursor addressing is handled — see the header for why
 * that is a bound rather than an omission.
 */
export function toHtml(captured) {
  const parts = [];
  let classes = [];
  let open = false;

  const close = () => {
    if (open) parts.push("</span>");
    open = false;
  };

  const pattern = /\x1b\[([0-9;]*)m/g;
  let last = 0;
  let match;

  while ((match = pattern.exec(captured)) !== null) {
    parts.push(escape(captured.slice(last, match.index)));
    last = pattern.lastIndex;

    close();
    classes = codesToClasses(match[1], classes);

    if (classes.length > 0) {
      parts.push(`<span class="${classes.map((name) => `t-${name}`).join(" ")}">`);
      open = true;
    }
  }

  parts.push(escape(captured.slice(last)));
  close();

  return parts.join("");
}

function codesToClasses(parameters, current) {
  const codes = parameters === "" ? [0] : parameters.split(";").map(Number);
  let classes = [...current];

  for (const code of codes) {
    if (code === 0) classes = [];
    else if (SGR.has(code)) classes.push(SGR.get(code));
  }

  return [...new Set(classes)];
}

function escape(text) {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/**
 * The stylesheet the captured output is drawn with.
 *
 * Every value is a token: the brand's mono face and spacing, and the deck
 * theme's code colours, which are the palette a slide's code gets. Flat, like
 * everything else slidx draws — the window has a hairline and no shadow, and
 * `scripts/check-flat.mjs` reads this file.
 */
export const STYLESHEET = `
* { box-sizing: border-box; }

body {
  margin: 0;
  padding: 24px;
  background: var(--slidx-brand-paper);
  font-family: var(--slidx-brand-font-mono);
}

.terminal {
  border: var(--slidx-brand-hairline) solid var(--slidx-brand-line);
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
}

.terminal pre {
  margin: 0;
  padding: 16px;
  overflow: hidden;
  font-size: 14px;
  line-height: 1.5;
  white-space: pre;
  tab-size: 2;
}

.t-bold { font-weight: 700; }
.t-dim { color: var(--slidx-color-code-comment); }
.t-italic { font-style: italic; }
.t-underline { text-decoration: underline; }
.t-red { color: var(--slidx-color-code-keyword); }
.t-green { color: var(--slidx-color-code-string); }
.t-yellow { color: var(--slidx-color-code-number); }
.t-blue { color: var(--slidx-color-code-type); }
.t-magenta { color: var(--slidx-color-code-keyword); }
.t-cyan { color: var(--slidx-color-code-type); }
`;

/**
 * One capture as a whole page, ready to be photographed.
 *
 * There is no title bar, and that is deliberate: every one of these commands
 * echoes its own invocation as its first line, so a bar would be the same words
 * twice and the only invented element on the picture.
 *
 * `tokens` is passed in rather than read here. It comes from the crates that
 * generate it, and a second reader of those would be a second place that knows
 * where they live.
 */
export function page(title, captured, { tokens }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${escape(title)}</title>
<style>
${tokens}
${STYLESHEET}
</style>
</head>
<body>
<div class="terminal">
  <pre>${toHtml(captured)}</pre>
</div>
</body>
</html>
`;
}
