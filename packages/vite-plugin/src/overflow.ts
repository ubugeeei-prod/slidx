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
  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    return null;
  }

  const browser = await chromium.launch();
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
 * Two things are measured, and both are content the audience does not get:
 * the body against the box the padding leaves it, and any block that clips its
 * own overflow. A code block is the second case in practice — it scrolls on a
 * laptop, which is indistinguishable from fitting, and on a wall it is simply
 * missing its right-hand side.
 */
function measureInPage(): Measurement[] {
  const stops = new Map<number, number>();

  return [...document.querySelectorAll(".slidx-page")].map((page) => {
    const slideIndex = Number((page as HTMLElement).dataset["slidxSlide"] ?? 0);
    const stop = stops.get(slideIndex) ?? 0;
    stops.set(slideIndex, stop + 1);

    const body = page.querySelector(".slidx-slide-body");
    if (body === null) return { slideIndex, stop, overHeight: 0, overWidth: 0 };

    const over = (scroll: number, client: number) =>
      client > 0 ? Math.max(0, (scroll - client) / client) : 0;

    let overWidth = over(body.scrollWidth, body.clientWidth);

    // An element that hides its own overflow keeps the body's numbers clean
    // while losing content all the same.
    for (const clipped of page.querySelectorAll("pre, table, .slidx-slide-body > *")) {
      overWidth = Math.max(overWidth, over(clipped.scrollWidth, clipped.clientWidth));
    }

    return {
      slideIndex,
      stop,
      overHeight: over(body.scrollHeight, body.clientHeight),
      overWidth,
    };
  });
}
