/**
 * When a slide counts as on screen.
 *
 * This is the seam that makes an island lazy, and the reason it is a seam at
 * all: happy-dom has no layout, so a real `IntersectionObserver` would never
 * report anything and the behaviour that matters most would be the behaviour
 * least covered.
 *
 * The failure modes guarded here:
 *
 * - One observer per island would put as many layout subscriptions on an
 *   overview grid as there are slides.
 * - An entry can be delivered for an element that has already stopped being
 *   watched; acting on it would mount an island the deck has finished with.
 * - `IntersectionObserver` is absent in the PDF exporter's DOM and in older
 *   embedded webviews. A deck that shows nothing there is worse than a deck
 *   that mounts everything, so the fallback is eager rather than never.
 */

import { describe, expect, it } from "vitest";

import {
  defaultVisibility,
  eagerVisibility,
  observerVisibility,
  type ObserverOptions,
} from "../src/visibility";

interface Entry {
  target: Element;
  isIntersecting: boolean;
}

/** What one fake observer recorded, and how a test drives it. */
interface Recorder {
  observed: Element[];
  unobserved: Element[];
  options: ObserverOptions | undefined;
  enter(target: Element): void;
  leave(target: Element): void;
}

/** Stands in for `IntersectionObserver`, driven by hand rather than by layout. */
function fakeObserver() {
  const instances: Recorder[] = [];

  class Observer {
    private readonly recorder: Recorder;

    constructor(callback: (entries: Entry[]) => void, options?: ObserverOptions) {
      this.recorder = {
        observed: [],
        unobserved: [],
        options,
        enter: (target) => callback([{ target, isIntersecting: true }]),
        leave: (target) => callback([{ target, isIntersecting: false }]),
      };
      instances.push(this.recorder);
    }

    observe(target: Element): void {
      this.recorder.observed.push(target);
    }

    unobserve(target: Element): void {
      this.recorder.unobserved.push(target);
    }

    disconnect(): void {}
  }

  return { Observer, instances };
}

function element(): HTMLElement {
  return document.createElement("div");
}

describe("eager visibility", () => {
  it("reports visible straight away", () => {
    const seen: boolean[] = [];
    eagerVisibility().observe(element(), (visible) => seen.push(visible));

    expect(seen).toEqual([true]);
  });

  it("answers each island independently", () => {
    const seen: string[] = [];
    const visibility = eagerVisibility();

    visibility.observe(element(), () => seen.push("first"));
    visibility.observe(element(), () => seen.push("second"));

    expect(seen).toEqual(["first", "second"]);
  });

  it("returns a stop function that is safe to call", () => {
    const stop = eagerVisibility().observe(element(), () => {});

    expect(() => stop()).not.toThrow();
  });
});

describe("observer visibility", () => {
  it("observes the element it was given", () => {
    const { Observer, instances } = fakeObserver();
    const target = element();

    observerVisibility(Observer).observe(target, () => {});

    expect(instances[0]?.observed).toEqual([target]);
  });

  it("reports an element entering view", () => {
    const { Observer, instances } = fakeObserver();
    const target = element();
    const seen: boolean[] = [];

    observerVisibility(Observer).observe(target, (visible) => seen.push(visible));
    instances[0]?.enter(target);

    expect(seen).toEqual([true]);
  });

  it("reports an element leaving view", () => {
    const { Observer, instances } = fakeObserver();
    const target = element();
    const seen: boolean[] = [];

    observerVisibility(Observer).observe(target, (visible) => seen.push(visible));
    instances[0]?.enter(target);
    instances[0]?.leave(target);

    expect(seen).toEqual([true, false]);
  });

  it("shares one observer across every island on the page", () => {
    // An overview grid has as many islands as slides, and each observer is its
    // own layout subscription.
    const { Observer, instances } = fakeObserver();
    const visibility = observerVisibility(Observer);

    visibility.observe(element(), () => {});
    visibility.observe(element(), () => {});

    expect(instances).toHaveLength(1);
    expect(instances[0]?.observed).toHaveLength(2);
  });

  it("routes each entry to the island it belongs to", () => {
    const { Observer, instances } = fakeObserver();
    const visibility = observerVisibility(Observer);
    const first = element();
    const second = element();
    const seen: string[] = [];

    visibility.observe(first, () => seen.push("first"));
    visibility.observe(second, () => seen.push("second"));
    instances[0]?.enter(second);

    expect(seen).toEqual(["second"]);
  });

  it("stops watching when the stop function is called", () => {
    const { Observer, instances } = fakeObserver();
    const target = element();
    const seen: boolean[] = [];

    const stop = observerVisibility(Observer).observe(target, (visible) => seen.push(visible));
    stop();
    instances[0]?.enter(target);

    expect(instances[0]?.unobserved).toEqual([target]);
    expect(seen).toEqual([]);
  });

  it("ignores an entry for something it is not watching", () => {
    // `unobserve` does not retract deliveries already queued, so an entry can
    // arrive after a deck has finished with the island.
    const { Observer, instances } = fakeObserver();
    const seen: boolean[] = [];

    observerVisibility(Observer).observe(element(), (visible) => seen.push(visible));
    instances[0]?.enter(element());

    expect(seen).toEqual([]);
  });

  it("passes its options through to the observer", () => {
    const { Observer, instances } = fakeObserver();

    observerVisibility(Observer, { rootMargin: "200px" }).observe(element(), () => {});

    expect(instances[0]?.options).toEqual({ rootMargin: "200px" });
  });
});

describe("choosing an implementation", () => {
  it("uses the observer where the browser has one", () => {
    const { Observer, instances } = fakeObserver();
    const target = element();

    defaultVisibility({ IntersectionObserver: Observer }).observe(target, () => {});

    expect(instances[0]?.observed).toEqual([target]);
  });

  it("mounts everything where there is no observer", () => {
    // The PDF exporter's DOM has no IntersectionObserver. A blank deck there
    // would be discovered in the exported file, which is far too late.
    const seen: boolean[] = [];

    defaultVisibility({}).observe(element(), (visible) => seen.push(visible));

    expect(seen).toEqual([true]);
  });

  it("forwards observer options when it does use one", () => {
    const { Observer, instances } = fakeObserver();

    defaultVisibility({ IntersectionObserver: Observer }, { threshold: 0.25 }).observe(
      element(),
      () => {},
    );

    expect(instances[0]?.options).toEqual({ threshold: 0.25 });
  });
});
