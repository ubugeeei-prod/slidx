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
}

/** What a viewer said about itself. */
export interface Position {
  slide: number;
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
      seats.set(id, { order: arrivals, slide: 0, at: now(), ...about });
    },

    moved(id, position) {
      const seat = seats.get(id);
      if (!seat) return;

      seat.slide = Math.max(0, Math.trunc(position.slide));
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
        }));
    },
  };
}
