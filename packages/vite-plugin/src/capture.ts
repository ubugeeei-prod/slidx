/**
 * Taking a screenshot, allowing for the browser losing its nerve.
 *
 * Two places here ask a browser for a PNG: the social cards, and the frames an
 * export asks for. Both hit the same failure on macOS CI —
 *
 *     page.screenshot: Protocol error (Page.captureScreenshot):
 *     Unable to capture screenshot
 *
 * — which ejected three merge-queue batches in one afternoon. It is not a
 * failure of the page: the same commit passes on a rerun, and it passes on
 * every Linux and Windows runner every time. Chromium's capture is asynchronous
 * with the compositor, and on a loaded machine the frame can be gone by the
 * time the protocol call reaches it.
 *
 * So it is retried, and the retry is bounded and loud. A test that goes green
 * by being run enough times is worthless, so a genuine failure has to still
 * fail: three attempts, and the last error is thrown with its own message
 * rather than a summary, so a real breakage reads exactly as it did before.
 *
 * Nothing else here is retried. This is not a policy about flakiness — it is
 * one browser call with one documented transient fault.
 */

/** How many times to ask before believing the answer. */
const ATTEMPTS = 3;

/**
 * The message Chromium produces when the frame is gone before the capture
 * lands. Matched rather than retrying every error: a page that failed to load,
 * a selector that found nothing, or a closed browser are all permanent, and
 * retrying them turns a clear failure into a slow one.
 */
const TRANSIENT = /Unable to capture screenshot|Target closed|Session closed/i;

export async function capture(take: () => Promise<Buffer>): Promise<Buffer> {
  let last: unknown;

  for (let attempt = 1; attempt <= ATTEMPTS; attempt += 1) {
    try {
      return await take();
    } catch (error) {
      last = error;

      if (!TRANSIENT.test(error instanceof Error ? error.message : String(error))) throw error;
      if (attempt === ATTEMPTS) break;
    }
  }

  throw last;
}
