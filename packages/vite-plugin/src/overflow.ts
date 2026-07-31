/**
 * Finding out whether a slide's content actually fits its box.
 *
 * The design box has `overflow: hidden`, so a slide with more on it than the
 * box holds loses the difference with no error anywhere — not in the terminal,
 * not in the console, and not on the author's screen, which shows the same
 * clipped slide the projector will. It is the one failure in the linted set
 * that nothing at build time can reason its way to: whether the content fits
 * depends on where the lines break, and where the lines break depends on font
 * metrics only a browser has.
 *
 * So this measures rather than estimates. It opens the *emitted* print shell —
 * the one artefact with a page per stop, already laid out at the deck's own
 * size — and reads `scrollHeight` against `clientHeight` for each. The numbers
 * go back to the Rust linter, which decides what they mean, so a clipped slide
 * is worded the same way whether it was caught here or by a rule that never
 * needed a browser.
 *
 * Playwright is an optional peer dependency, and this returns `null` when it is
 * absent rather than throwing. The honest consequence is that a build with no
 * browser has no overflow check at all — which is why it says so instead of
 * quietly reporting nothing.
 */

import { pathToFileURL } from "node:url";

import { READY_ATTRIBUTE } from "./pdf";

/** One rendered stop, as the Rust linter's `Measurement` expects it. */
export interface Measurement {
  slideIndex: number;
  stop: number;
  /** How far the content exceeded its box downwards, as a share of the box. */
  overHeight: number;
  /** The same across. */
  overWidth: number;
  /**
   * The layout region this measures, when it is one rather than the whole slide.
   *
   * A region is its own grid track, so it can lose content while the slide as a
   * whole fits — which is exactly what happens to a block moved into a narrower
   * column. Measuring only the body would report that slide as clean.
   */
  region?: string;
}

export interface MeasureOptions {
  /** How long to wait for the shell to expand its stops. */
  timeoutMs?: number;
}

/**
 * Measures every stop in an emitted print shell.
 *
 * Returns `null` when no browser is available — the caller has to be able to
 * tell "nothing overflowed" apart from "nothing was checked", because those
 * are opposite answers and only one of them means the deck is fine.
 */
export async function measureOverflow(
  printHtmlPath: string,
  options: MeasureOptions = {},
): Promise<Measurement[] | null> {
  // Both halves of "there is no browser here", because they fail differently
  // and only one of them was caught. A missing *package* throws on the import;
  // a missing *browser binary* throws on the launch, and that is the ordinary
  // state of a machine where somebody ran `pnpm install` and not `playwright
  // install` — every non-Linux CI runner in this repository, and every
  // contributor's first checkout.
  //
  // Answering `null` either way is what makes the caller's warn-and-continue
  // path reachable. It was written and could not be reached.
  let browser;
  try {
    const { chromium } = await import("playwright");
    browser = await chromium.launch();
  } catch {
    return null;
  }
  try {
    // A viewport wide enough that the pages lay out at their design size. The
    // shell's `.slidx-page` is `width: 100%` of the document, so a narrow
    // window would measure a smaller slide — and the whole layout is a size
    // container, so a smaller slide is not the same slide with smaller text.
    const page = await browser.newPage({ viewport: { width: 1600, height: 1200 } });

    // `file://` for the same reason the PDF exporter uses it: the shell inlines
    // everything, so measuring the artefact is measuring what a person opens.
    await page.goto(pathToFileURL(printHtmlPath).href, { waitUntil: "load" });
    await page.waitForSelector(`html[${READY_ATTRIBUTE}]`, {
      timeout: options.timeoutMs ?? 30_000,
    });

    return await page.evaluate(measureInPage);
  } finally {
    await browser.close();
  }
}

/**
 * Runs inside the page. Must be self-contained — it is serialised across.
 *
 * Three things are measured, and all of them are content the audience does not
 * get. The body against the box the padding leaves it. Each **region**, because
 * a layout gives every region its own grid track: a block moved into a narrower
 * column can lose its last lines while the body's own scroll height stays
 * exactly what it was. And any element that clips its own overflow — a code
 * block in practice, which scrolls on a laptop, indistinguishable from fitting,
 * and on a wall is simply missing its right-hand side.
 */
function measureInPage(): Measurement[] {
  const stops = new Map<number, number>();
  const measured: Measurement[] = [];

  const over = (scroll: number, client: number) =>
    client > 0 ? Math.max(0, (scroll - client) / client) : 0;

  for (const page of document.querySelectorAll(".slidx-page")) {
    const slideIndex = Number((page as HTMLElement).dataset["slidxSlide"] ?? 0);
    const stop = stops.get(slideIndex) ?? 0;
    stops.set(slideIndex, stop + 1);

    const body = page.querySelector(".slidx-slide-body");
    if (body === null) {
      measured.push({ slideIndex, stop, overHeight: 0, overWidth: 0 });
      continue;
    }

    let overWidth = over(body.scrollWidth, body.clientWidth);

    // An element that hides its own overflow keeps its container's numbers clean
    // while losing content all the same. Counted against the slide rather than
    // against a region: a `<pre>` too wide for its column is too wide for the
    // slide, and the region measurement below says which column it was in.
    for (const clipped of page.querySelectorAll("pre, table, .slidx-block")) {
      overWidth = Math.max(overWidth, over(clipped.scrollWidth, clipped.clientWidth));
    }

    measured.push({
      slideIndex,
      stop,
      overHeight: over(body.scrollHeight, body.clientHeight),
      overWidth,
    });

    for (const region of page.querySelectorAll("[data-slidx-region]")) {
      const name = (region as HTMLElement).dataset["slidxRegion"];
      if (name === undefined) continue;

      measured.push({
        slideIndex,
        stop,
        overHeight: over(region.scrollHeight, region.clientHeight),
        overWidth: over(region.scrollWidth, region.clientWidth),
        region: name,
      });
    }
  }

  return measured;
}
