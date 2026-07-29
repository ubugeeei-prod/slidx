/**
 * Moving through a deck.
 *
 * A deck is multi-page HTML, so "next" means one of two different things and
 * the runtime has to know which: advance a step on this page, or leave for
 * another page. Getting the boundary wrong is the difference between a deck
 * that steps and a deck that skips.
 *
 * The behaviours worth stating are the ones a speaker meets under pressure:
 *
 * - Going back across a slide boundary must land on the *end* of the previous
 *   slide, not its start, or every backward step replays the whole build.
 * - A deep link must survive a reload, because that is how a speaker recovers
 *   from a crashed browser mid-talk.
 * - Holding a key must not queue navigation the speaker cannot see the end of.
 */

import { describe, expect, it, vi } from "vite-plus/test";

import { createNavigator, type Navigator } from "../src/navigate";

/** A stage with `stops` stops, recording where it was told to go. */
function stage(stops: number) {
  let index = 0;
  return {
    get stopCount() {
      return stops;
    },
    get index() {
      return index;
    },
    apply(next: number) {
      index = Math.max(0, Math.min(next, stops - 1));
      return index;
    },
    applyPrint() {},
  };
}

function navigatorFor(
  options: { stops?: number; slide?: number; slideCount?: number; step?: number } = {},
): { navigator: Navigator; visited: string[]; url: () => string } {
  const visited: string[] = [];
  let url = `/slides/${(options.slide ?? 0) + 1}/`;

  const navigator = createNavigator({
    stage: stage(options.stops ?? 3),
    slide: options.slide ?? 0,
    slideCount: options.slideCount ?? 3,
    step: options.step,
    hrefFor: (slide, step) => `/slides/${slide + 1}/${step === undefined ? "" : `?step=${step}`}`,
    navigate: (href) => visited.push(href),
    replaceUrl: (next) => {
      url = next;
    },
  });

  return { navigator, visited, url: () => url };
}

describe("stepping within a slide", () => {
  it("advances one stop at a time", () => {
    const { navigator } = navigatorFor({ stops: 3 });

    navigator.next();
    expect(navigator.step).toBe(1);

    navigator.next();
    expect(navigator.step).toBe(2);
  });

  it("goes back one stop at a time", () => {
    const { navigator } = navigatorFor({ stops: 3, step: 2 });

    navigator.previous();
    expect(navigator.step).toBe(1);
  });

  it("stays on the page while there are stops left", () => {
    const { navigator, visited } = navigatorFor({ stops: 3 });

    navigator.next();
    expect(visited).toEqual([]);
  });
});

describe("crossing a slide boundary", () => {
  it("leaves for the next slide after the last stop", () => {
    const { navigator, visited } = navigatorFor({ stops: 2, slide: 0, step: 1 });

    navigator.next();
    expect(visited).toEqual(["/slides/2/"]);
  });

  it("lands on the end of the previous slide when going back", () => {
    // Landing on its start would replay a build the audience already watched,
    // which is the single most visible way to look lost.
    const { navigator, visited } = navigatorFor({ stops: 3, slide: 1, step: 0 });

    navigator.previous();
    expect(visited).toEqual(["/slides/1/?step=last"]);
  });

  it("does nothing at the end of the deck", () => {
    const { navigator, visited } = navigatorFor({ stops: 1, slide: 2, slideCount: 3 });

    navigator.next();
    expect(visited).toEqual([]);
  });

  it("does nothing at the start of the deck", () => {
    const { navigator, visited } = navigatorFor({ stops: 1, slide: 0 });

    navigator.previous();
    expect(visited).toEqual([]);
  });
});

describe("deep links", () => {
  it("opens at the stop the URL names", () => {
    // How a speaker recovers from a crashed browser mid-talk.
    const { navigator } = navigatorFor({ stops: 4, step: 2 });
    expect(navigator.step).toBe(2);
  });

  it("clamps a stop the slide no longer has", () => {
    // A link written before an edit must land somewhere real.
    const { navigator } = navigatorFor({ stops: 2, step: 9 });
    expect(navigator.step).toBe(1);
  });

  it("understands `last`", () => {
    const { navigator } = navigatorFor({ stops: 4, step: "last" as unknown as number });
    expect(navigator.step).toBe(3);
  });

  it("writes the current stop into the URL", () => {
    const { navigator, url } = navigatorFor({ stops: 3 });

    navigator.next();
    expect(url()).toContain("step=1");
  });

  it("leaves the URL clean on the resting stop", () => {
    // `?step=0` is noise in a URL someone is about to share.
    const { navigator, url } = navigatorFor({ stops: 3, step: 1 });

    navigator.previous();
    expect(url()).not.toContain("step");
  });
});

describe("the keyboard", () => {
  /**
   * Dispatches a real key event, the way the page does.
   *
   * A constructed-but-undispatched event has no `target`, so checking one
   * would prove nothing about the rule that matters most here — that keys
   * belonging to a focused field are left alone.
   */
  function press(
    navigator: Navigator,
    key: string,
    { from = document.body, ...init }: KeyboardEventInit & { from?: HTMLElement } = {},
  ) {
    const listener = (event: Event) => navigator.handleKey(event as KeyboardEvent);
    document.addEventListener("keydown", listener);

    const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true, ...init });
    from.dispatchEvent(event);

    document.removeEventListener("keydown", listener);
    return event;
  }

  it("advances on the keys a clicker sends", () => {
    // Presentation remotes send PageDown and ArrowRight, not Space.
    for (const key of ["ArrowRight", "PageDown", " ", "ArrowDown"]) {
      const { navigator } = navigatorFor({ stops: 3 });
      press(navigator, key);
      expect(navigator.step, key).toBe(1);
    }
  });

  it("goes back on the matching keys", () => {
    for (const key of ["ArrowLeft", "PageUp", "ArrowUp"]) {
      const { navigator } = navigatorFor({ stops: 3, step: 2 });
      press(navigator, key);
      expect(navigator.step, key).toBe(1);
    }
  });

  it("jumps to the ends", () => {
    const { navigator } = navigatorFor({ stops: 4 });

    press(navigator, "End");
    expect(navigator.step).toBe(3);

    press(navigator, "Home");
    expect(navigator.step).toBe(0);
  });

  it("ignores a key with a modifier", () => {
    // Cmd-ArrowRight is "go forward in history", and a deck that eats it is a
    // deck the browser no longer works in.
    const { navigator } = navigatorFor({ stops: 3 });
    press(navigator, "ArrowRight", { metaKey: true });

    expect(navigator.step).toBe(0);
  });

  it("ignores keys while typing in a field", () => {
    // The editor and the audience channel both put inputs on the page. A space
    // bar that advances the slide while someone types a question is a bug
    // reported as the deck being haunted.
    const { navigator } = navigatorFor({ stops: 3 });
    const input = document.createElement("input");
    document.body.append(input);

    press(navigator, "ArrowRight", { from: input });
    expect(navigator.step).toBe(0);

    input.remove();
  });

  it("claims only the keys it acts on", () => {
    const { navigator } = navigatorFor({ stops: 3 });

    expect(press(navigator, "ArrowRight").defaultPrevented).toBe(true);
    expect(press(navigator, "t").defaultPrevented).toBe(false);
  });
});

describe("telling other windows", () => {
  it("reports every move so a mirror can follow", () => {
    const seen = vi.fn();
    const { navigator } = navigatorFor({ stops: 3 });
    navigator.subscribe(seen);

    navigator.next();

    expect(seen).toHaveBeenCalledWith({ slide: 0, step: 1 });
  });

  it("follows a position it is given", () => {
    const { navigator } = navigatorFor({ stops: 3 });
    navigator.show({ slide: 0, step: 2 });

    expect(navigator.step).toBe(2);
  });

  it("leaves the page when told about another slide", () => {
    const { navigator, visited } = navigatorFor({ stops: 3, slide: 0 });
    navigator.show({ slide: 2, step: 1 });

    expect(visited).toEqual(["/slides/3/?step=1"]);
  });

  it("does not re-announce a position it was given", () => {
    // Announcing it would send it straight back and the two windows would
    // volley one move forever.
    const seen = vi.fn();
    const { navigator } = navigatorFor({ stops: 3 });
    navigator.subscribe(seen);

    navigator.show({ slide: 0, step: 1 });

    expect(seen).not.toHaveBeenCalled();
  });
});
