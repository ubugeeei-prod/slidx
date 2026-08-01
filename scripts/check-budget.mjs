/**
 * Builds the fixture deck and holds what comes out to its budget.
 *
 * `scripts/budget.mjs` holds the figures and why each exists. This is the part
 * that has to run a real build, because the only honest answer to "how big is a
 * deck" is one taken off a deck that was built.
 *
 * The same build answers the claim slidx repeats most often — that a slide
 * without steps fetches no JavaScript — by reading the page rather than the
 * template that produced it. A page can reference a module the shell did not
 * mean to emit, and only the emitted page knows.
 */

import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

import { build } from "vite";

// The published package rather than the source, so this measures what somebody
// installs — the library build included.
import { slidx } from "@slidxjs/vite-plugin";

import {
  BUDGETS,
  executableScripts,
  FIXTURE,
  NOT_THE_AUDIENCE,
  overBudget,
  referencesIn,
  splitPages,
  unmeasured,
} from "./budget.mjs";

/** A one-pixel PNG, so the picture slide has something real to fetch. */
const SQUARE = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
  "base64",
);

const root = await mkdtemp(join(tmpdir(), "slidx-budget-"));
const out = join(root, "dist");

await mkdir(join(root, "slides"), { recursive: true });
await writeFile(join(root, "slides", "square.png"), SQUARE);
for (const [name, body] of Object.entries(FIXTURE)) {
  await writeFile(join(root, "slides", name), body);
}

await build({
  root,
  logLevel: "silent",
  // The two steps that launch a browser, and only those. A default build starts
  // one for the PDF and for the overflow measurement, so this check failed on
  // every machine without Playwright browsers installed — both CI runners that
  // are not Linux, and every contributor who has not run `playwright install`.
  // A size budget that needs a browser to measure a byte is one most people
  // cannot run.
  //
  // What each costs the figures, measured rather than assumed:
  //
  // - `print` and `overflow` cost nothing. The print shell is outside all five
  //   by name and the overflow pass emits nothing at all.
  // - `og` used to be off here too, and is back on: it was only off because a
  //   card could not be rasterised without a browser *and threw* instead of
  //   saying so, which is now fixed. The figures include the real `og:image`
  //   again.
  // - `presenter` was tried and put back: switching it off stops the step
  //   runtime being emitted at all, and the "nothing measured this" guard is
  //   what said so before the number could quietly become four figures.
  plugins: [slidx({ print: false, overflow: false })],
  build: { outDir: out },
});

const files = await walk(out);
const read = async (path) => readFile(join(out, path));
const gzip = (source) => gzipSync(source).byteLength;

/** Whether a path is something a room fetches. See `NOT_THE_AUDIENCE`. */
const forTheRoom = (path) => !NOT_THE_AUDIENCE.some((part) => path.includes(part));

/** Every page a room loads, from the projector or from a phone. */
const pages = files.filter((path) => path.endsWith(".html") && forTheRoom(path));

const { slides: audience, snippets } = splitPages(pages);

/**
 * The page for the first slide, which is the one with nothing to reveal.
 *
 * It is the deck's index rather than `slides/1/`: the first slide is the deck's
 * own address, which is the whole reason a deck has one URL per slide and not
 * one URL plus a fragment. Reading the wrong page here is not a small mistake —
 * the slide *with* steps is exactly the one allowed to load a runtime, so
 * measuring it reports the claim broken when it is kept.
 */
const still = audience.find((path) => /^slides\/index\.html$/.test(path));

if (still === undefined) {
  process.stderr.write(`error: no page for the first slide among ${audience.join(", ")}\n`);
  process.exit(1);
}

const stillPage = (await read(still)).toString("utf8");

/**
 * Script bytes the page would run, counted as the page presents them.
 *
 * Both halves matter and they fail differently, so they are counted apart. An
 * inline `<script>` is weight in the document and has a budget. A `src` is a
 * *request*, which is the thing a room on venue wifi actually pays for, and a
 * slide with nothing to reveal is allowed none.
 */
let inlineBytes = 0;
let fetchedBytes = 0;
for (const { attributes, body } of executableScripts(stillPage)) {
  const src = /\bsrc="([^"]+)"/.exec(attributes);

  if (src === null) {
    inlineBytes += Buffer.byteLength(body);
    continue;
  }

  const path = resolve(still.slice(0, still.lastIndexOf("/")), src[1]);
  // A reference to something the build did not emit still costs a request, so
  // it is not nothing.
  fetchedBytes += files.includes(path) ? (await read(path)).byteLength : 1;
}

/** What the pages themselves ask a browser to fetch. */
const wanted = new Set();
for (const path of audience) {
  const page = (await read(path)).toString("utf8");
  const directory = path.slice(0, path.lastIndexOf("/"));

  for (const reference of referencesIn(page)) wanted.add(resolve(directory, reference));
}

let audienceBytes = 0;
for (const path of audience) audienceBytes += gzip(await read(path));

let downloaded = audienceBytes;
for (const path of snippets) downloaded += gzip(await read(path));
for (const path of wanted) {
  if (files.includes(path)) downloaded += gzip(await read(path));
}

const runtime = files.find((path) => path.endsWith(".js") && path.includes("runtime"));

/**
 * The page a QR code on a slide points at.
 *
 * Its reader is standing in a room, on a phone, on whatever signal the venue
 * has, in the ninety seconds before the speaker moves on — which is the
 * tightest budget in this file and the one nothing was holding until now.
 */
const [snippet] = snippets;

const measured = {
  "javascript a slide with no steps fetches": fetchedBytes,
  "javascript inlined into a slide with no steps": inlineBytes,
  "an audience slide, gzipped": Math.round(audienceBytes / audience.length),
  "everything an audience downloads, gzipped": downloaded,
  ...(snippet === undefined ? {} : { "a shared snippet page, gzipped": gzip(await read(snippet)) }),
  ...(runtime === undefined ? {} : { "the step runtime, gzipped": gzip(await read(runtime)) }),
};

await rm(root, { recursive: true, force: true });

for (const budget of BUDGETS) {
  const value = measured[budget.name];
  process.stdout.write(
    value === undefined
      ? `  ${budget.name}: not measured\n`
      : `  ${budget.name}: ${value} / ${budget.limit}\n`,
  );
}

const missing = unmeasured(measured);
const over = overBudget(measured);

for (const name of missing) {
  process.stderr.write(
    `error: nothing measured "${name}" — a budget nobody measures is not a budget\n`,
  );
}

for (const { name, measured: value, limit, over: excess, protects } of over) {
  process.stderr.write(
    `error: ${name} is ${value}, over ${limit} by ${excess}\n  it protects ${protects}\n`,
  );
}

if (over.length > 0 || missing.length > 0) {
  process.stderr.write(
    `\nA budget is moved deliberately, in review, with the reason in the commit —\n` +
      `scripts/budget.mjs. A build that grew has either earned it or nobody noticed,\n` +
      `and those look identical until somebody has to say which.\n`,
  );
  process.exit(1);
}

process.stdout.write(`budget: ${BUDGETS.length} figures checked, every one inside\n`);

/** A reference on a page, as a path from the output root. */
function resolve(directory, reference) {
  if (reference.startsWith("/")) return reference.slice(1);

  const parts = directory.split("/").filter(Boolean);
  for (const step of reference.split("/")) {
    if (step === "." || step === "") continue;
    if (step === "..") parts.pop();
    else parts.push(step);
  }

  return parts.join("/");
}

async function walk(directory, prefix = "") {
  const found = [];

  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...(await walk(join(directory, entry.name), path)));
    else found.push(path);
  }

  return found.sort((a, b) => a.localeCompare(b, "en"));
}
