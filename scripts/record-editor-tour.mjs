/**
 * A deliberate tour of the visual editor, recorded by using the real editor.
 *
 * The short arrange recording proves one gesture. This tour shows the whole
 * loop: keyboard commands, text editing, addressed styles, a layout written to
 * Markdown, slide creation and ordering, source mode, undo/redo, and a second
 * editor changing the same file. The video is for the documentation site; the
 * sparse APNG is the same run sampled at its meaningful states so the README
 * can play it as an image.
 */

import { execFileSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { slidx } from "@ubugeeei/slidx-vite-plugin";
import { chromium } from "playwright";
import { createServer } from "vite";

import { encodeApng } from "./animate.mjs";
import { decodePng } from "./png.mjs";
import { FILE_WIDTH, mark, stagePage } from "./stage.mjs";

const OUT = process.argv[2] ?? "docs/media";
const DECK = "examples/deck/slides";
const EDITOR = { width: 1400, height: 760 };
const VIEWPORT = { width: EDITOR.width + FILE_WIDTH, height: EDITOR.height };
const FIRST = "0001.md";
const PRIMARY = process.platform === "darwin" ? "Meta" : "Control";

const OPENING = `---
title: Product tour
duration: 20m
theme: editorial
---

# Ideas become a live deck

Design the moment, keep the source.

<!-- notes: Start with the document and the canvas moving together. -->
`;

const scratch = mkdtempSync(join(tmpdir(), "slidx-editor-tour-"));
const root = join(scratch, "deck");
const videoDir = join(scratch, "video");
mkdirSync(OUT, { recursive: true });
cpSync(resolve(DECK), join(root, "slides"), { recursive: true });
writeFileSync(join(root, "slides", FIRST), OPENING);

const tokens = execFileSync("cargo", ["run", "-q", "-p", "slidx_docs", "--example", "tokens"], {
  encoding: "utf8",
});
const server = await createServer({
  root,
  logLevel: "silent",
  plugins: [slidx()],
  server: { port: 0, watch: null, hmr: false },
});
await server.listen();

const browser = await chromium.launch();
const context = await browser.newContext({
  viewport: VIEWPORT,
  colorScheme: "light",
  recordVideo: { dir: videoDir, size: VIEWPORT },
});
const peerContext = await browser.newContext({ viewport: EDITOR, colorScheme: "light" });

try {
  const page = await context.newPage();
  const editorUrl = `${server.resolvedUrls.local[0]}__slidx/`;
  const stage = join(scratch, "tour.html");
  writeFileSync(
    stage,
    stagePage(editorUrl, {
      tokens,
      editorWidth: EDITOR.width,
      editorHeight: EDITOR.height,
      withFile: true,
    }),
  );
  await page.goto(pathToFileURL(stage).href);

  const editor = page.frames().find((frame) => frame.url().includes("/__slidx/"));
  if (!editor) throw new Error("the tour stage has no editor frame");
  await editor.waitForSelector(".slidx-arrange-grip");

  const chrome = page.frameLocator(".editor");
  const canvas = chrome.frameLocator(".slidx-canvas-frame");
  const frames = [];
  const shown = new Map();

  const showFile = async (name = FIRST) => {
    const text = readFileSync(join(root, "slides", name), "utf8").replace(/\n$/, "");
    const rows = mark(shown.get(name) ?? "", text);
    shown.set(name, text);
    await page.evaluate(
      ([path, lines]) => window.slidxStage.file(path, lines),
      [`slides/${name}`, rows],
    );
  };
  const hold = async (delay) => {
    frames.push({
      shot: await page.screenshot({ animations: "disabled", caret: "hide" }),
      delay,
    });
    await page.waitForTimeout(delay);
  };
  const click = async (locator) => {
    const box = await locator.boundingBox();
    if (!box) throw new Error("the tour tried to point at something that was not drawn");

    const to = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    const from = await page.evaluate(() => {
      const pointer = document.querySelector(".pointer");
      return !pointer || pointer.hasAttribute("hidden")
        ? { x: 24, y: 24 }
        : { x: Number.parseFloat(pointer.style.left), y: Number.parseFloat(pointer.style.top) };
    });

    for (let step = 1; step <= 12; step += 1) {
      const along = step / 12;
      const at = {
        x: from.x + (to.x - from.x) * along,
        y: from.y + (to.y - from.y) * along,
      };
      await page.mouse.move(at.x, at.y);
      await page.evaluate(([x, y]) => window.slidxStage.pointer(x, y), [at.x, at.y]);
      await page.waitForTimeout(18);
    }
    await page.mouse.click(to.x, to.y);
  };

  await showFile();
  await hold(900);

  await click(chrome.locator(".slidx-shortcuts-open"));
  await hold(1_800);
  await page.keyboard.press("Escape");

  const heading = canvas.locator("h1[contenteditable]").first();
  await click(heading);
  await heading.fill("Ideas become an editable deck");
  await heading.press("Tab");
  await waitForFile(root, FIRST, (source) => source.includes("editable deck"));
  await showFile();
  await hold(1_400);

  const paragraph = canvas.locator("p[contenteditable]").first();
  await click(paragraph);
  await paragraph.evaluate((line) => {
    const range = line.ownerDocument.createRange();
    range.selectNodeContents(line);
    const selection = line.ownerDocument.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
    line.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
  });
  await chrome.locator('[data-group="selection"] input[placeholder="accent"]').waitFor();
  await hold(900);

  await chrome.locator('[data-group="selection"] input[placeholder="accent"]').fill("accent");
  await chrome.locator('[data-group="selection"] input[placeholder="result"]').fill("moment");
  await chrome
    .locator('[data-group="selection"] [aria-label="Style properties"]')
    .fill("color=signal\nfont=mono");
  await click(chrome.locator('[data-group="selection"] .slidx-add'));
  await waitForFile(root, FIRST, (source) => source.includes("#moment"));
  await showFile();
  await hold(1_500);

  await click(chrome.locator('[data-group="slide"] [data-layout="aside"]'));
  await waitForFile(root, FIRST, (source) => source.includes("--slidx-layout: aside"));
  await showFile();
  await hold(1_600);

  const transition = chrome.locator('[data-group="slide"] [data-key="transition"]');
  await click(transition);
  await transition.fill("fade");
  await transition.press("Tab");
  await waitForFile(root, FIRST, (source) => source.includes("transition: fade"));
  await showFile();
  await hold(1_100);

  await click(chrome.locator('.slidx-outline-row[data-slide="0"] .slidx-outline-open'));
  await page.keyboard.press(`${PRIMARY}+m`);
  await chrome.locator(".slidx-outline-row").nth(4).waitFor();
  await hold(1_000);

  await click(chrome.locator('.slidx-outline-row[data-slide="1"] .slidx-outline-open'));
  await page.keyboard.press("m");
  const source = chrome.locator(".slidx-canvas-source");
  await source.waitFor();
  await source.fill("## Edited in Markdown\n\nBoth views stay synchronized.");
  await source.press("Tab");
  const added = await waitForSlide(root, "Both views stay synchronized.");
  await showFile(added);
  await hold(1_500);

  await page.keyboard.press("v");
  await canvas.locator("text=Both views stay synchronized.").waitFor();
  await page.keyboard.press(`${PRIMARY}+d`);
  await waitForOutlineCount(chrome, 6);
  await hold(1_100);

  const order = await outlineTitles(chrome);
  const moving = chrome.locator('.slidx-outline-row[data-slide="3"] .slidx-outline-open');
  await click(moving);
  await moving.focus();
  await page.keyboard.press("Alt+ArrowUp");
  await waitForOutlineOrder(chrome, order, "changed");
  await hold(900);

  const moved = await outlineTitles(chrome);
  await page.keyboard.press(`${PRIMARY}+z`);
  await waitForOutlineOrder(chrome, order, "equal");
  await hold(700);

  await page.keyboard.press(`Shift+${PRIMARY}+z`);
  await waitForOutlineOrder(chrome, moved, "equal");
  await waitForOutlineCount(chrome, 6);
  await hold(1_000);

  await click(chrome.locator('.slidx-outline-row[data-slide="0"] .slidx-outline-open'));
  const peer = await peerContext.newPage();
  await peer.goto(editorUrl);
  await peer.locator(".slidx-outline-row").first().waitFor();
  await chrome.locator(".slidx-presence[data-empty='false']").waitFor();
  await hold(1_400);

  const peerHeading = peer
    .frameLocator(".slidx-canvas-frame")
    .locator("h1[contenteditable]")
    .first();
  await peerHeading.fill("Two editors, one Markdown file");
  await peerHeading.press("Tab");
  await waitForFile(root, FIRST, (text) => text.includes("Two editors, one Markdown file"));
  await canvas.locator("text=Two editors, one Markdown file").waitFor();
  await showFile(FIRST);
  await hold(2_200);

  const decoded = decodePng(frames[0].shot);
  const preview = encodeApng(
    frames.map(({ shot, delay }) => ({ pixels: decodePng(shot).pixels, delay })),
    { width: decoded.width, height: decoded.height },
  );
  const previewOut = join(OUT, "editor-tour.png");
  writeFileSync(previewOut, preview);

  const video = page.video();
  await peerContext.close();
  await context.close();
  const videoOut = join(OUT, "editor-tour.webm");
  renameSync(await video.path(), videoOut);

  process.stdout.write(
    `  ${previewOut} (${(preview.length / 1024).toFixed(0)} kB)\n` +
      `  ${videoOut} (${(readFileSync(videoOut).length / 1024).toFixed(0)} kB)\n`,
  );
} finally {
  await peerContext.close().catch(() => undefined);
  await context.close().catch(() => undefined);
  await browser.close();
  await server.close();
  rmSync(scratch, { recursive: true, force: true });
}

async function waitForFile(root, name, accepts) {
  const file = join(root, "slides", name);
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const source = readFileSync(file, "utf8");
    if (accepts(source)) return source;
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`${name} never received the edit the tour performed`);
}

async function waitForSlide(root, phrase) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    for (const name of readdirSync(join(root, "slides")).filter((file) => file.endsWith(".md"))) {
      if (readFileSync(join(root, "slides", name), "utf8").includes(phrase)) return name;
    }
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`no slide received “${phrase}”`);
}

async function outlineTitles(chrome) {
  return chrome.locator(".slidx-outline-title").allTextContents();
}

async function waitForOutlineCount(chrome, count) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if ((await chrome.locator(".slidx-outline-row").count()) === count) return;
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`the editor tour did not settle at ${count} slides`);
}

async function waitForOutlineOrder(chrome, expected, relation) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const current = await outlineTitles(chrome);
    const equal = JSON.stringify(current) === JSON.stringify(expected);
    if ((relation === "equal" && equal) || (relation === "changed" && !equal)) return;
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`the editor tour outline never ${relation === "equal" ? "returned" : "moved"}`);
}
