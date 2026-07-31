/**
 * Who is connected, and where in the deck they are.
 *
 * Two people editing the same slide without knowing it is how one of them loses
 * a paragraph. Nothing here prevents that — the shared document does — but the
 * paragraph that survives is no comfort if neither person understood why their
 * screen changed under them. So the roster exists to make the other person
 * visible before the collision, not to report it afterwards.
 *
 * # There are no names, and none are invented
 *
 * slidx has no accounts, no sign-in and no identity of any kind, and a share URL
 * is a capability rather than a person. So a viewer is labelled by the order
 * they arrived — the author is "you", everyone else is "guest 2", "guest 3" —
 * and where they are is a slide number. Prompting for a name would be a login
 * screen on a dev server, and guessing one from a hostname or an IP address
 * would be putting somebody's machine name on the author's screen without
 * asking.
 *
 * # Why a heartbeat rather than a disconnect
 *
 * A phone that walked out of Wi-Fi range does not close its connection; it stops
 * answering. A roster that waited for a close event would show a co-presenter who
 * left the building, which is worse than showing nobody. So presence expires,
 * and the stream that carries it is what refreshes it.
 */

/** How long a viewer stays in the roster after its last word. */
export const PRESENCE_TIMEOUT_MS = 20_000;

/** One connected viewer, as the editor draws them. */
export interface Viewer {
  id: string;
  /** "you" for the author, "guest 2" upward for everybody else. */
  label: string;
  /** True for the author's own machine, which reached this over loopback. */
  local: boolean;
  /** True when this viewer may change the deck. */
  canEdit: boolean;
  /** Which slide they are looking at, counting from zero. */
  slide: number;
  /**
   * Which block on it they have selected, when they have one.
   *
   * Absent rather than zero for a viewer who has selected nothing, because
   * zero is a block. An editor drawing this has to be able to tell "on this
   * slide, in the first paragraph" from "on this slide, nowhere in
   * particular", and a number that means both is how the second becomes a
   * marker sitting on somebody's title.
   */
  block?: number;
}

/**
 * What a viewer said about itself.
 *
 * `block` is written `| undefined` rather than left merely optional because
 * this is a report rather than a patch: a viewer who has deselected has to be
 * able to say so, and leaving the field out of an object built from a request
 * body would mean building a different object for that case.
 */
export interface Position {
  slide: number;
  block?: number | undefined;
}

export interface Roster {
  /** Adds a viewer, or refreshes one that is already here. */
  seen(id: string, about: { local: boolean; canEdit: boolean }): void;
  /** Records where a viewer is. Ignores one nobody has seen. */
  moved(id: string, position: Position): void;
  gone(id: string): void;
  /** Everyone still connected, the author first. */
  viewers(): Viewer[];
}

interface Seat {
  order: number;
  local: boolean;
  canEdit: boolean;
  slide: number;
  block: number | undefined;
  at: number;
}

export function createRoster(now: () => number = Date.now): Roster {
  const seats = new Map<string, Seat>();
  let arrivals = 0;

  function alive(): [string, Seat][] {
    const cutoff = now() - PRESENCE_TIMEOUT_MS;

    return [...seats.entries()].filter(([id, seat]) => {
      if (seat.at >= cutoff) return true;
      seats.delete(id);
      return false;
    });
  }

  return {
    seen(id, about) {
      const seat = seats.get(id);

      if (seat) {
        seat.at = now();
        seat.canEdit = about.canEdit;
        return;
      }

      arrivals += 1;
      seats.set(id, { order: arrivals, slide: 0, block: undefined, at: now(), ...about });
    },

    moved(id, position) {
      const seat = seats.get(id);
      if (!seat) return;

      seat.slide = Math.max(0, Math.trunc(position.slide));
      seat.block = blockOf(position.block);
      seat.at = now();
    },

    gone: (id) => void seats.delete(id),

    viewers() {
      // The author first, then in the order people arrived. A roster that
      // reordered itself as people moved would be unreadable exactly when it
      // matters, which is while somebody else is typing.
      return alive()
        .sort(([, a], [, b]) => Number(b.local) - Number(a.local) || a.order - b.order)
        .map(([id, seat]) => ({
          id,
          label: seat.local ? "you" : `guest ${seat.order}`,
          local: seat.local,
          canEdit: seat.canEdit,
          slide: seat.slide,
          ...(seat.block === undefined ? {} : { block: seat.block }),
        }));
    },
  };
}

/**
 * A block number, or nothing at all.
 *
 * Everything that reaches here came off a POST body, and a share link is handed
 * to somebody else — so this is one of the few places in the dev server where
 * the input is not the author's own. A fraction, a negative, an infinity or a
 * string all mean the same thing as saying nothing: that viewer has no block
 * selected. Rounding them into a block instead would put another person's name
 * on a paragraph they have never seen.
 */
function blockOf(said: number | undefined): number | undefined {
  return typeof said === "number" && Number.isInteger(said) && said >= 0 ? said : undefined;
}
