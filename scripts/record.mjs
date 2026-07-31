/**
 * Every picture and every animation the documentation site uses.
 *
 * Nothing here is a file somebody made once. Each image is a real run
 * photographed, and each animation is a real deck driven by real key presses —
 * so an image that stopped being true fails to reproduce rather than quietly
 * showing a product that no longer exists. `scripts/screenshot.mjs` set that
 * standard for the README's slides; this extends it to the terminal and to
 * motion, which are the two things a still of a slide cannot show.
 *
 * ```sh
 * vp run media
 * ```
 *
 * # What is recorded, and why each one
 *
 * **The terminal**, because half of slidx has no browser in it. `slidx doctor`
 * is the command that justifies the project's whole thesis in one screen, and
 * `slidx lint` on a deliberately broken deck is the one that shows what a rule
 * actually says.
 *
 * **The deck**, because steps are the thing a still cannot argue for. The
 * recording drives the built pages with the arrow key a presentation remote
 * sends, so what it shows is the runtime resolving stops, not a mockup of it.
 *
 * # Why webm
 *
 * It is what the browser recording the page produces, it plays in every engine
 * this project already tests against, and it needs no player. A GIF would be
 * larger, worse, and — more to the point — would have to be made by a tool that
 * is not the thing being photographed.
 */

import { execFileSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { chromium } from "playwright";

import { encodeApng } from "./animate.mjs";
import { decodePng } from "./png.mjs";
import { capture, captureUntil, page as terminalPage, toHtml } from "./terminal.mjs";

const OUT = process.argv[2] ?? "docs/media";
const DECK = "examples/deck/dist/slides";

/** Rendered at twice the display size, for the same reason the slides are. */
const SCALE = 2;
const TERMINAL = { width: 900, height: 560 };
const SLIDE = { width: 1280, height: 720 };
const TOUR_SCHEME = "dark";
const TYPE_DELAY_MS = 42;

/**
 * A deck with one thing wrong with it, written here rather than committed.
 *
 * The point of the picture is what the rule says, so the deck that provokes it
 * belongs next to the code that photographs it — a fixture in `examples/` would
 * be a broken deck that everything else in the repository has to keep excluding.
 */
const BROKEN_DECK = `---
title: Making Decks Fast
duration: 5m
---

# Making Decks Fast

![our logo](https://example.com/logo.png)

- one
- two
- three
- four
- five
- six
- seven
- eight
- nine
`;

const scratch = join(OUT, ".scratch");
mkdirSync(join(scratch, "slides"), { recursive: true });
writeFileSync(join(scratch, "slides", "0001.md"), BROKEN_DECK);
const exportRoot = join(scratch, "export");
cpSync(resolve("examples/deck/slides"), join(exportRoot, "slides"), { recursive: true });
cpSync(resolve("examples/deck/dist"), join(exportRoot, "dist"), { recursive: true });

const slidx = binary();
const css = execFileSync("cargo", ["run", "-q", "-p", "slidx_docs", "--example", "tokens"], {
  encoding: "utf8",
});

const captures = [
  {
    name: "doctor",
    title: "slidx doctor",
    // No `--offline`: the network reading is one of the checks a speaker
    // actually cares about, and this is a photograph of a real machine rather
    // than a fixture. The report differs between machines because the whole
    // point of the command is that it reads the one you are standing at.
    text: capture(slidx, ["doctor"]),
  },
  {
    name: "lint",
    title: "slidx lint",
    // Run from inside the deck so the report names `slides` rather than a
    // scratch path that exists for eleven seconds.
    text: capture(slidx, ["lint"], { cwd: scratch }),
  },
];
const cli = [
  {
    command: "slidx dev --no-open --port 41795",
    text: portableTourOutput(
      await captureUntil(slidx, ["dev", "--no-open", "--port", "41795"], {
        cwd: resolve("examples/deck"),
        until: /Editor:\s+http/,
      }),
    ),
  },
  {
    command: "slidx fmt --check",
    text: portableTourOutput(capture(slidx, ["fmt", "--check"], { cwd: resolve("examples/deck") })),
  },
  {
    command: "slidx lint",
    text: portableTourOutput(capture(slidx, ["lint"], { cwd: resolve("examples/deck") })),
  },
  {
    command: "slidx export --target browser --no-build --out exports",
    text: portableTourOutput(
      capture(slidx, ["export", "--target", "browser", "--no-build", "--out", "exports"], {
        cwd: exportRoot,
      }),
    ),
  },
  {
    command: "slidx doctor --offline",
    text: excerpt(
      portableTourOutput(
        capture(slidx, ["doctor", "--offline"], { cwd: resolve("examples/deck") }),
      ),
      18,
    ),
  },
  {
    command: "slidx publish --plan --target blog",
    text: portableTourOutput(
      capture(slidx, ["publish", "--plan", "--target", "blog"], {
        cwd: resolve("examples/deck"),
      }),
    ),
  },
];

const browser = await chromium.launch();

for (const shot of captures) {
  await terminalStills(shot);
}

await cliAnimation(cli);
await deckAnimation();

await browser.close();
rmSync(scratch, { recursive: true, force: true });

process.stdout.write(`\n${captures.length * 2 + 3} file(s) in ${OUT}\n`);

/**
 * The debug binary, built if it is not there.
 *
 * Debug rather than release: this runs the command for its *output*, and a
 * release build would cost minutes to change nothing about what it prints.
 */
function binary() {
  const path = resolve("target/debug/slidx");
  execFileSync("cargo", ["build", "-q", "-p", "slidx_cli", "--bin", "slidx"], {
    stdio: "inherit",
  });
  return path;
}

/** One capture, photographed in both schemes. */
async function terminalStills({ name, title, text }) {
  const html = join(scratch, `${name}.html`);
  writeFileSync(html, terminalPage(title, text, { tokens: css }));

  for (const scheme of ["light", "dark"]) {
    const context = await browser.newContext({
      viewport: TERMINAL,
      deviceScaleFactor: SCALE,
      colorScheme: scheme,
    });
    const page = await context.newPage();
    await page.goto(pathToFileURL(resolve(html)).href);

    const target = page.locator(".terminal");
    await target.waitFor();

    const out = join(OUT, `terminal-${name}-${scheme}.png`);
    await target.screenshot({ path: out });
    process.stdout.write(`  ${out}\n`);

    await context.close();
  }
}

/**
 * The report arriving a line at a time.
 *
 * Not a typing effect over invented text: the lines are the captured ones, in
 * the order the command wrote them, revealed at a speed a reader can follow.
 * What is being animated is the reading rather than the running.
 */
async function cliAnimation(commands) {
  const html = join(scratch, "cli.html");
  writeFileSync(
    html,
    terminalPage("slidx command tour", "", { tokens: css }).replace(
      "</style>",
      `.terminal { height: calc(100vh - 48px); }
.tour-output {
  animation: tour-output-in 240ms cubic-bezier(.22, 1, .36, 1) both;
}
@keyframes tour-output-in {
  from { opacity: 0; transform: translateY(3px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>`,
    ),
  );

  const frames = [];
  await record("cli-tour.webm", TERMINAL, async (page) => {
    await page.goto(pathToFileURL(resolve(html)).href);
    const pre = page.locator("pre");

    for (const command of commands) {
      await pre.evaluate((node) => {
        node.textContent = "$ ";
      });
      await page.waitForTimeout(180);

      for (const character of command.command) {
        await pre.evaluate((node, typed) => {
          node.textContent += typed;
        }, character);
        await page.waitForTimeout(TYPE_DELAY_MS);
      }

      await page.waitForTimeout(480);
      await pre.evaluate((node, output) => {
        node.insertAdjacentHTML("beforeend", `\n<span class="tour-output">${output}</span>`);
      }, toHtml(command.text));
      await page.waitForTimeout(1_800);
      frames.push({ shot: await page.screenshot(), delay: 1_800 });
    }
  });

  const first = decodePng(frames[0].shot);
  const preview = encodeApng(
    frames.map(({ shot, delay }) => ({ pixels: decodePng(shot).pixels, delay })),
    { width: first.width, height: first.height },
  );
  const out = join(OUT, "cli-tour.png");
  writeFileSync(out, preview);
  process.stdout.write(`  ${out} (${(preview.length / 1024).toFixed(0)} kB)\n`);
}

/**
 * The built deck, driven by the key a presentation remote sends.
 *
 * The pages come from `vite build`, so this photographs what a viewer gets
 * rather than a dev server's approximation of it.
 */
async function deckAnimation() {
  const first = resolve(DECK, "index.html");

  await record("deck.webm", SLIDE, async (page) => {
    await page.goto(pathToFileURL(first).href);
    await page.locator(".slidx-slide").waitFor();
    await page.waitForTimeout(900);

    // Right, twelve times. The example deck has four slides and several stops
    // inside them, so this walks stops and slide boundaries alike — which is
    // the thing worth showing, because they are the same key.
    for (let press = 0; press < 12; press += 1) {
      await page.keyboard.press("ArrowRight");
      await page.waitForTimeout(700);
    }
  });
}

/** Runs a scene in its own context and moves the video where it belongs. */
async function record(name, size, scene) {
  const directory = join(scratch, "video");
  const context = await browser.newContext({
    viewport: size,
    colorScheme: TOUR_SCHEME,
    recordVideo: { dir: directory, size },
  });

  const page = await context.newPage();
  await scene(page);

  const video = page.video();
  await context.close();

  const out = join(OUT, name);
  renameSync(await video.path(), out);
  process.stdout.write(`  ${out} (${(readFileSync(out).length / 1024).toFixed(0)} kB)\n`);
}

/** The top of a real report, clipped only so the next command fits the frame. */
function excerpt(text, lines) {
  return `${text.split("\n").slice(0, lines).join("\n")}\n`;
}

/**
 * Keep real command output while removing the recording machine from the shot.
 *
 * macOS resolves `/tmp` to `/private/tmp`, whereas Node keeps the spelling used
 * to enter the worktree. Replace both spellings so regenerated documentation is
 * identical regardless of where its checkout lives.
 */
function portableTourOutput(text) {
  const directories = [resolve("examples/deck"), exportRoot];

  for (const directory of directories) {
    text = text.replaceAll(directory, "~/slides");
    text = text.replaceAll(realpathSync(directory), "~/slides");
  }

  return text;
}
