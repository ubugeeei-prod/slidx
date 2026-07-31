/**
 * A deliberate tour of the visual editor, recorded by using the real editor.
 *
 * The short arrange recording proves one gesture. This tour shows the whole
 * loop: resizable chrome, keyboard commands, text editing, visual type choices,
 * addressed styles, a layout written to Markdown, slide clipboard and ordering,
 * source mode, undo/redo, and a second editor changing the same file. The video
 * is for the documentation site; the sparse APNG is the same run sampled at its
 * meaningful states so the README can play it as an image.
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

import { slidx } from "@slidxjs/vite-plugin";
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
const TOUR_SCHEME = "dark";
const POINTER_STEPS = 28;
const POINTER_STEP_MS = 16;
const SHARE_EDIT = "0123456789abcdef.00112233445566778899aabbccddeeff";

const OPENING = `---
title: Product tour
duration: 20m
theme: editorial
---

# Ideas become a live deck

Design the moment, keep the source.

<!-- notes: Start with the document and the canvas moving together. -->
`;

const DROPPED_MEDIA = [
  {
    name: "tour-layout.png",
    type: "image/png",
    bytes: readFileSync(resolve("docs/media/editor-storyboard-dark.png")).toString("base64"),
  },
  {
    name: "tour-motion.webm",
    type: "video/webm",
    bytes: readFileSync(resolve("docs/media/deck.webm")).toString("base64"),
  },
];

const scratch = mkdtempSync(join(tmpdir(), "slidx-editor-tour-"));
const root = join(scratch, "deck");
const videoDir = join(scratch, "video");
mkdirSync(OUT, { recursive: true });
cpSync(resolve(DECK), join(root, "slides"), { recursive: true });
writeFileSync(join(root, "slides", FIRST), OPENING);

// The peer enters through the network URL with the same capability a real
// co-presenter receives. That makes the roster say "guest 2", rather than
// presenting two loopback tabs as two copies of "you".
process.env["SLIDX_SHARE_EDIT"] = SHARE_EDIT;

const tokens = execFileSync("cargo", ["run", "-q", "-p", "slidx_docs", "--example", "tokens"], {
  encoding: "utf8",
});
const server = await createServer({
  root,
  logLevel: "silent",
  plugins: [
    {
      name: "slidx-tour-share-capability",
      configureServer(server) {
        server.middlewares.use((request, _response, next) => {
          if (
            request.url?.startsWith("/__slidx/") &&
            request.headers["x-slidx-share"] === undefined
          ) {
            request.headers["x-slidx-share"] = SHARE_EDIT;
          }
          next();
        });
      },
    },
    slidx(),
  ],
  server: { host: "0.0.0.0", port: 0, watch: null, hmr: false },
});
await server.listen();

const browser = await chromium.launch();
const context = await browser.newContext({
  viewport: VIEWPORT,
  colorScheme: TOUR_SCHEME,
  recordVideo: { dir: videoDir, size: VIEWPORT },
});
const peerContext = await browser.newContext({ viewport: EDITOR, colorScheme: TOUR_SCHEME });

try {
  const page = await context.newPage();
  const authorBase = new URL(server.resolvedUrls.local[0]);
  authorBase.hostname = "127.0.0.1";
  const editorUrl = new URL("__slidx/", authorBase).href;
  const peerUrl = `${server.resolvedUrls.network[0] ?? server.resolvedUrls.local[0]}__slidx/#s=${SHARE_EDIT}`;
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
  const pointAt = async (locator, position = { x: 0.5, y: 0.5 }) => {
    // Preview rows are intentionally tall enough to read, so later slides can
    // sit below the outline scrollport. Bring the real target into view before
    // measuring it, just as an author must, or a coordinate-only click would
    // land on whatever happens to be drawn at the clipped point instead.
    await locator.scrollIntoViewIfNeeded();
    const box = await locator.boundingBox();
    if (!box) throw new Error("the tour tried to point at something that was not drawn");

    const to = { x: box.x + box.width * position.x, y: box.y + box.height * position.y };
    const from = await page.evaluate(() => {
      const pointer = document.querySelector(".pointer");
      return !pointer || pointer.hasAttribute("hidden")
        ? { x: 24, y: 24 }
        : { x: Number.parseFloat(pointer.style.left), y: Number.parseFloat(pointer.style.top) };
    });

    for (let step = 1; step <= POINTER_STEPS; step += 1) {
      const along = easeInOut(step / POINTER_STEPS);
      const at = {
        x: from.x + (to.x - from.x) * along,
        y: from.y + (to.y - from.y) * along,
      };
      await page.mouse.move(at.x, at.y);
      await page.evaluate(([x, y]) => window.slidxStage.pointer(x, y), [at.x, at.y]);
      await page.waitForTimeout(POINTER_STEP_MS);
    }

    return to;
  };
  const click = async (locator) => {
    const to = await pointAt(locator);
    await page.mouse.click(to.x, to.y);
  };
  const drag = async (locator, delta, linger = 0, position) => {
    const from = await pointAt(locator, position);
    await page.mouse.down();

    for (let step = 1; step <= POINTER_STEPS; step += 1) {
      const along = easeInOut(step / POINTER_STEPS);
      const at = {
        x: from.x + delta.x * along,
        y: from.y + delta.y * along,
      };
      await page.mouse.move(at.x, at.y);
      await page.evaluate(([x, y]) => window.slidxStage.pointer(x, y), [at.x, at.y]);
      await page.waitForTimeout(POINTER_STEP_MS);
    }

    if (linger > 0) await hold(linger);
    await page.mouse.up();
  };
  const dropMedia = async () => {
    await pointAt(canvas.locator("[data-slidx-region]").first());
    await editor.evaluate((files) => {
      const frame = document.querySelector(".slidx-canvas-frame");
      const preview = frame?.contentDocument;
      const target = preview?.querySelector("[data-slidx-region]");
      if (!preview || !target) throw new Error("the tour has no rendered drop region");

      const rect = target.getBoundingClientRect();
      const transfer = new DataTransfer();
      for (const item of files) {
        const text = atob(item.bytes);
        const bytes = Uint8Array.from(text, (character) => character.charCodeAt(0));
        transfer.items.add(new File([bytes], item.name, { type: item.type }));
      }
      const init = {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + Math.min(12, rect.height / 2),
        dataTransfer: transfer,
      };

      preview.dispatchEvent(new DragEvent("dragenter", init));
      preview.dispatchEvent(new DragEvent("dragover", init));
      window.__slidxTourDrop = { preview, init };
    }, DROPPED_MEDIA);

    await chrome.locator('.slidx-media-drop[data-active="true"]:not([data-target=""])').waitFor();
    await hold(1_200);
    await editor.evaluate(() => {
      const drop = window.__slidxTourDrop;
      if (!drop) throw new Error("the tour did not hold the media drag");

      drop.preview.dispatchEvent(new DragEvent("drop", drop.init));
      delete window.__slidxTourDrop;
    });
    await chrome.locator('.slidx-media-drop[data-active="false"][data-busy="false"]').waitFor();
  };

  await showFile();
  await hold(1_200);

  await click(chrome.locator(".slidx-shortcuts-open"));
  await hold(2_000);
  await page.keyboard.press("Escape");

  // Both side panels are working space, not fixed chrome. Expanding the left
  // makes the live slide previews easier to scan; expanding the right gives
  // the visual choices room without taking either preference into the deck.
  await drag(chrome.getByRole("separator", { name: "Resize slide panel" }), { x: 88, y: 0 }, 800);
  await drag(
    chrome.getByRole("separator", { name: "Resize inspector panel" }),
    { x: -96, y: 0 },
    800,
  );
  await hold(1_200);

  const heading = canvas.locator("h1[contenteditable]").first();
  await click(heading);
  await replaceText(heading, "Ideas become an editable deck");
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

  await replaceText(
    chrome.locator('[data-group="selection"] input[placeholder="accent"]'),
    "accent",
    44,
  );
  await replaceText(
    chrome.locator('[data-group="selection"] input[placeholder="result"]'),
    "moment",
    44,
  );
  await click(chrome.getByRole("button", { name: "Font: Mono" }));
  await click(chrome.getByRole("button", { name: "Size: H2" }));
  await click(chrome.getByRole("button", { name: "Color: Accent" }));
  await hold(1_100);
  await click(chrome.locator('[data-group="selection"] .slidx-add'));
  await waitForFile(
    root,
    FIRST,
    (source) =>
      source.includes("#moment") &&
      source.includes("font=mono") &&
      source.includes("size=heading-2") &&
      source.includes("color=accent"),
  );
  await showFile();
  await hold(1_500);

  await openInspectorTab(chrome, "Slide");
  await click(chrome.locator('[data-group="slide"] [data-layout="aside"]'));
  await waitForFile(root, FIRST, (source) => source.includes("--slidx-layout: aside"));
  await showFile();
  await hold(1_600);

  const transition = chrome.locator('[data-group="slide"] [data-key="transition"]');
  await click(transition);
  await replaceText(transition, "fade", 48);
  await transition.press("Tab");
  await waitForFile(root, FIRST, (source) => source.includes("transition: fade"));
  await showFile();
  await hold(1_100);

  await click(canvas.locator("p[contenteditable]").first());
  await chrome.locator('.slidx-freeform[data-active="true"]').waitFor();
  await chrome.locator(".slidx-freeform-color-input").fill("#f59e0b");
  await waitForFile(root, FIRST, (source) => source.includes("-color: #f59e0b"));
  await showFile();
  await hold(1_200);

  await drag(chrome.locator('.slidx-freeform-handle[data-handle="se"]'), { x: -140, y: 24 }, 900);
  await waitForFile(root, FIRST, (source) => source.includes("-inset:"));
  await showFile();
  await hold(1_200);

  const beforeMove = readFileSync(join(root, "slides", FIRST), "utf8");
  const move = await editor.evaluate(() => {
    const frame = document.querySelector(".slidx-canvas-frame");
    const preview = frame?.contentDocument;
    const safe = preview?.querySelector(".slidx-slide-body")?.getBoundingClientRect();
    const selected = preview
      ?.querySelector("[data-slidx-editor-selected]")
      ?.getBoundingClientRect();
    if (!safe || !selected) throw new Error("the tour cannot measure the selected block");

    return { x: Math.max(32, safe.right - selected.right), y: 0 };
  });
  // The north resize handle sits over the centre of the move strip. Aim away
  // from all three top handles, exactly as a person would, so this records the
  // move hit target rather than dispatching an invented event.
  await drag(chrome.locator(".slidx-freeform-move"), move, 1_000, { x: 0.18, y: 0.5 });
  await waitForFile(root, FIRST, (source) => source !== beforeMove);
  await showFile();
  await hold(1_400);

  await dropMedia();
  await waitForFile(
    root,
    FIRST,
    (source) => source.includes("tour-layout.png") && source.includes("tour-motion.webm"),
  );
  await canvas.locator('img[src*="tour-layout.png"]').waitFor();
  await canvas.locator('video[src*="tour-motion.webm"]').waitFor();
  await showFile();
  await hold(2_000);

  await click(chrome.locator('.slidx-outline-row[data-slide="0"] .slidx-outline-open'));
  await page.keyboard.press(`${PRIMARY}+m`);
  await chrome.locator(".slidx-outline-row").nth(4).waitFor();
  await hold(1_000);

  await click(chrome.locator('.slidx-outline-row[data-slide="1"] .slidx-outline-open'));
  await page.keyboard.press("m");
  const source = chrome.locator(".slidx-canvas-source");
  await source.waitFor();
  await replaceText(source, "## Edited in Markdown\n\nBoth views stay synchronized.", 24);
  await source.press("Tab");
  const added = await waitForSlide(root, "Both views stay synchronized.");
  await showFile(added);
  await hold(1_500);

  await page.keyboard.press("v");
  await canvas.locator("text=Both views stay synchronized.").waitFor();
  await page.keyboard.press(`${PRIMARY}+c`);
  await click(chrome.locator('.slidx-outline-row[data-slide="3"] .slidx-outline-open'));
  await page.keyboard.press(`${PRIMARY}+v`);
  await waitForOutlineCount(chrome, 6);
  // Six preview rows no longer all fit in the outline at once. The selected
  // copy may sit just below its scrollport, so attachment and `aria-current`
  // are the state to wait for; requiring viewport visibility would turn a
  // successful paste into a recorder timeout.
  await waitForSelectedSlide(chrome, 4, "Edited in Markdown");
  await canvas.locator("text=Both views stay synchronized.").waitFor();
  await waitForOutlinePreview(chrome, 4, "Edited in Markdown");
  await hold(1_300);

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
  await peer.goto(peerUrl);
  await peer.locator(".slidx-outline-row").first().waitFor();
  await chrome.locator(".slidx-presence[data-empty='false']").waitFor();
  await hold(1_400);

  const peerHeading = peer
    .frameLocator(".slidx-canvas-frame")
    .locator("h1[contenteditable]")
    .first();
  await replaceText(peerHeading, "Two editors, one Markdown file");
  await peerHeading.press("Tab");
  await waitForFile(root, FIRST, (text) => text.includes("Two editors, one Markdown file"));
  await canvas.locator("text=Two editors, one Markdown file").waitFor();
  await waitForOutlinePreview(chrome, 0, "Two editors, one Markdown file");
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

async function replaceText(locator, text, delay = 36) {
  await locator.fill("");
  await locator.pressSequentially(text, { delay });
}

function easeInOut(progress) {
  return 0.5 - Math.cos(Math.PI * progress) / 2;
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

/** Opens one inspector section when this editor has the tabbed inspector. */
async function openInspectorTab(chrome, name) {
  const tab = chrome.getByRole("tab", { name, exact: true });
  if ((await tab.count()) > 0 && (await tab.getAttribute("aria-selected")) !== "true") {
    await tab.click();
  }
}

async function waitForOutlineCount(chrome, count) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if ((await chrome.locator(".slidx-outline-row").count()) === count) return;
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`the editor tour did not settle at ${count} slides`);
}

async function waitForSelectedSlide(chrome, index, title) {
  let seen = [];
  for (let attempt = 0; attempt < 200; attempt += 1) {
    seen = await chrome.locator(".slidx-outline-row").evaluateAll((rows) =>
      rows.map((row) => ({
        slide: row.getAttribute("data-slide"),
        current: row.getAttribute("aria-current"),
        title: row.querySelector(".slidx-outline-title")?.textContent,
      })),
    );
    if (
      seen.some(
        (row) => row.slide === String(index) && row.current === "true" && row.title === title,
      )
    ) {
      return;
    }
    await new Promise((done) => setTimeout(done, 50));
  }
  throw new Error(`the pasted slide was not selected at ${index}: ${JSON.stringify(seen)}`);
}

async function waitForOutlinePreview(chrome, index, text) {
  const row = chrome.locator(`.slidx-outline-row[data-slide="${index}"]`);
  await row.scrollIntoViewIfNeeded();
  await row
    .locator(".slidx-outline-frame")
    .contentFrame()
    .getByText(text, { exact: true })
    .first()
    .waitFor();
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
