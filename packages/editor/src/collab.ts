/**
 * Who else is here, and where they are in the deck.
 *
 * Two people editing the same slide without knowing it is how one of them loses
 * a paragraph. The dev server's shared document means neither change is thrown
 * away, but a paragraph that survives is no comfort if neither person
 * understood why their screen moved. So this surface exists to make the other
 * person visible *before* the collision.
 *
 * # It holds no document
 *
 * This is the part worth being careful about. `history.ts` refuses to keep a
 * stack of whole sources because it would mean the editor holds a second copy
 * of the document that can disagree with the file, and collaboration is exactly
 * the feature that would justify one. So it does not have one: when the server
 * says the deck changed, this asks the session to re-read the deck through the
 * same route it uses on open. Nothing is merged here, nothing is diffed here,
 * and the editor's model of the deck still comes from one place.
 *
 * An announcement carrying the source the editor already has is ignored, which
 * is what keeps the author's own edits from re-reading — and, more importantly,
 * from clearing the undo stack they just added to.
 *
 * # Why a read of a stream rather than an `EventSource`
 *
 * `EventSource` cannot set a header, and the share secret must not travel any
 * other way: a query parameter reaches an access log, and a log that holds this
 * value is a log that is enough to join the session. So the stream is read from
 * a `fetch` response, which can carry the credential in a header the same way
 * every other editor request does.
 */

import { element, fill } from "./dom";
import type { Surface } from "./outline";
import type { EditorState } from "./session";

export interface PresenceOptions {
  /** Where the editing routes live. */
  base?: string;
  /** Asked to re-read the deck when somebody else changed it. */
  reload(): void;
  fetch?: typeof globalThis.fetch;
  /** The URL to take the share credential out of. Defaults to this page's. */
  href?: string;
  /** How long to wait before reconnecting. */
  retry?: number;
}

/** One connected viewer, as the dev server reports them. */
export interface Viewer {
  id: string;
  label: string;
  local: boolean;
  canEdit: boolean;
  slide: number;
}

/**
 * How long to wait before reconnecting a dropped stream.
 *
 * Fixed, and short. This faces a dev server on the same machine or the same
 * room, so there is no relay to overwhelm and no herd to spread out — the only
 * thing being waited for is a restart the author asked for.
 */
const RETRY_MS = 2_000;

/** The fragment key the share URL puts the secret under. */
const FRAGMENT_KEY = "s";

/** The header the credential is presented in. */
export const CREDENTIAL_HEADER = "x-slidx-share";

export function createPresence(options: PresenceOptions): Surface {
  const base = options.base ?? "/__slidx/";
  const send = options.fetch ?? globalThis.fetch.bind(globalThis);
  const credential = readCredential(options.href ?? location.href);

  const list = element("ul", { class: "slidx-presence-list" });
  const root = element("aside", { class: "slidx-presence", "data-empty": true }, [
    element("span", { class: "slidx-presence-label" }, ["Editing together"]),
    list,
  ]);

  let seat: string | undefined;
  let source = "";
  let reported = -1;
  let stopped = false;

  /**
   * The two things that keep the loop alive between attempts.
   *
   * Held rather than local because stopping is somebody else's decision: the
   * read is parked on a response that only ends when the server says so, and
   * the wait between attempts is parked on a timer. Neither notices an editor
   * being torn down, so `destroy` has to reach both.
   */
  let live: AbortController | undefined;
  let waking: { timer: ReturnType<typeof setTimeout>; wake: () => void } | undefined;

  const headers: Record<string, string> = credential ? { [CREDENTIAL_HEADER]: credential } : {};

  function draw(viewers: Viewer[]): void {
    // Nobody else connected means no new chrome at all. An author working alone
    // should not have to look at a panel telling them they are alone.
    root.setAttribute("data-empty", String(viewers.length < 2));

    fill(
      list,
      viewers.map((viewer) =>
        element("li", { class: "slidx-presence-who", "data-local": viewer.local }, [
          element("span", { class: "slidx-presence-name" }, [viewer.label]),
          element("span", { class: "slidx-presence-where" }, [`slide ${viewer.slide + 1}`]),
          ...(viewer.canEdit
            ? []
            : [element("span", { class: "slidx-presence-role" }, ["reading"])]),
        ]),
      ),
    );
  }

  function heard(event: string, payload: Record<string, unknown>): void {
    if (event === "hello" && typeof payload["id"] === "string") {
      seat = payload["id"];
      reported = -1;
      return;
    }

    if (event === "presence") {
      draw(Array.isArray(payload["viewers"]) ? (payload["viewers"] as Viewer[]) : []);
      return;
    }

    // The bytes are the whole test for whether this is news. The author's own
    // edit comes back here too, and re-reading on it would clear the undo
    // stack the author has just added to.
    if (event === "state" && typeof payload["source"] === "string") {
      if (payload["source"] !== source) options.reload();
    }
  }

  async function listen(): Promise<void> {
    while (!stopped) {
      try {
        live = new AbortController();
        const response = await send(`${base}live`, { headers, signal: live.signal });
        const reader = response.body?.getReader();
        if (!reader) return;

        const decoder = new TextDecoder();
        let buffer = "";

        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const read = readFrames(buffer);
          buffer = read.rest;

          for (const { event, data } of read.frames) heard(event, data);
        }
      } catch {
        // A dev server that restarted, or a network that went. Neither is worth
        // an error in front of somebody who is writing a talk.
      }

      if (stopped) return;
      await new Promise<void>((wake) => {
        waking = { timer: setTimeout(wake, options.retry ?? RETRY_MS), wake };
      });
      waking = undefined;
    }
  }

  void listen();

  return {
    root,

    /**
     * Stops listening, for good.
     *
     * The reconnect is deliberately unbounded — a dev server the author
     * restarted must be picked back up without them reloading anything — which
     * is exactly why an editor that has gone away has to say so. Otherwise a
     * remounted editor leaves the previous one's loop reconnecting behind it,
     * asking a session that no longer exists to re-read the deck.
     */
    destroy() {
      stopped = true;
      live?.abort();

      if (waking) {
        clearTimeout(waking.timer);
        // Woken as well as cleared, so the loop returns on its own `stopped`
        // check rather than being left parked on a promise nothing resolves.
        waking.wake();
        waking = undefined;
      }
    },

    render(state: EditorState) {
      source = state.source;

      // Only when it changed. A render happens on every keystroke in the
      // inspector, and a post per keystroke would make presence the busiest
      // thing on the wire.
      if (stopped || seat === undefined || state.selection.slide === reported) return;
      reported = state.selection.slide;

      void send(`${base}here`, {
        method: "POST",
        headers: { ...headers, "content-type": "application/json" },
        body: JSON.stringify({ id: seat, slide: reported }),
      }).catch(() => {
        // Presence is the first thing that should fail quietly.
      });
    },
  };
}

/**
 * The share credential this page was opened with, if any.
 *
 * From the fragment and nowhere else. A URL carrying the secret in its query is
 * one that has already been written to a log, and the dev server refuses it —
 * this is the reading half of that rule, and `packages/runtime/src/remote.ts`
 * is where the rule is stated for the phone remote it came from.
 */
export function readCredential(href: string): string | undefined {
  const hash = href.indexOf("#");
  if (hash < 0) return undefined;

  return new URLSearchParams(href.slice(hash + 1)).get(FRAGMENT_KEY) ?? undefined;
}

/** One parsed frame. */
export interface Frame {
  event: string;
  data: Record<string, unknown>;
}

/**
 * Complete frames out of a buffer, and whatever is left over.
 *
 * A read from a stream lands wherever the network split it, so the last frame
 * in a buffer is usually half of one. Acting on a half-frame is how a deck
 * announcement becomes a parse error, so incomplete text stays in the buffer
 * until the rest of it arrives.
 */
export function readFrames(buffer: string): { frames: Frame[]; rest: string } {
  const parts = buffer.split("\n\n");
  const rest = parts.pop() ?? "";
  const frames: Frame[] = [];

  for (const part of parts) {
    let event = "message";
    const data: string[] = [];

    for (const line of part.split("\n")) {
      if (line.startsWith("event: ")) event = line.slice(7);
      else if (line.startsWith("data: ")) data.push(line.slice(6));
    }

    if (data.length === 0) continue;

    try {
      const parsed: unknown = JSON.parse(data.join("\n"));
      if (typeof parsed === "object" && parsed !== null) {
        frames.push({ event, data: parsed as Record<string, unknown> });
      }
    } catch {
      // A keep-alive comment, or a frame from a server that is not this one.
    }
  }

  return { frames, rest };
}
