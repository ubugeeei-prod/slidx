/**
 * When a slide is on screen.
 *
 * This is what makes an island lazy. A built deck is one document per slide,
 * so on the audience path the answer is usually "immediately" — but the
 * presenter view, the overview grid, and the print shell all put many slides
 * in one document, and that is exactly where eager mounting hurts: a Three.js
 * scene on slide 40 would take its WebGL context the moment the deck opened,
 * and forty of them do not fit on a laptop.
 *
 * The observer is an interface rather than a call to `IntersectionObserver`
 * because lazy mounting is the behaviour most worth testing and least testable
 * against a real layout engine — happy-dom has no layout, so nothing ever
 * intersects. Tests drive visibility by hand; the browser gets the real thing.
 */

/** Watches whether an element is on screen. */
export interface IslandVisibility {
  /**
   * Calls `onChange` whenever `element` enters or leaves view, and returns a
   * function that stops watching it.
   */
  observe(element: HTMLElement, onChange: (visible: boolean) => void): () => void;
}

/** The part of `IntersectionObserver` this module uses, so a test can substitute one. */
interface ObserverEntryLike {
  readonly target: Element;
  readonly isIntersecting: boolean;
}

interface ObserverLike {
  observe(target: Element): void;
  unobserve(target: Element): void;
  disconnect(): void;
}

export interface ObserverOptions {
  root?: Element | Document | null;
  rootMargin?: string;
  threshold?: number | number[];
}

type ObserverConstructor = new (
  callback: (entries: ObserverEntryLike[]) => void,
  options?: ObserverOptions,
) => ObserverLike;

/** Where `IntersectionObserver` is looked up. A parameter so the fallback is testable. */
export interface ObserverScope {
  IntersectionObserver?: ObserverConstructor;
}

/**
 * Everything is visible, from the start.
 *
 * The fallback, and the right answer for print: a deck that shows nothing is
 * worse than a deck that is briefly slow, so where visibility cannot be
 * observed every island mounts rather than none.
 */
export function eagerVisibility(): IslandVisibility {
  return {
    observe(_element, onChange) {
      onChange(true);
      return () => {};
    },
  };
}

/**
 * Real visibility, through one observer shared by every island on the page.
 *
 * One observer rather than one per island because a deck in overview mode has
 * as many islands as slides, and each observer is its own layout subscription.
 */
export function observerVisibility(
  Observer: ObserverConstructor,
  options: ObserverOptions = {},
): IslandVisibility {
  const watchers = new Map<Element, (visible: boolean) => void>();

  const observer = new Observer((entries) => {
    for (const entry of entries) {
      // An entry can arrive for an element that has since stopped being
      // watched: `unobserve` does not retract deliveries already queued.
      watchers.get(entry.target)?.(entry.isIntersecting);
    }
  }, options);

  return {
    observe(element, onChange) {
      watchers.set(element, onChange);
      observer.observe(element);

      return () => {
        watchers.delete(element);
        observer.unobserve(element);
      };
    },
  };
}

/**
 * The observer where the browser has one, eager mounting where it does not.
 *
 * `IntersectionObserver` is absent in the PDF exporter's DOM shim and in older
 * embedded webviews. Neither is a place to discover that half the deck is
 * blank, so the degraded path is "mount everything" rather than "mount
 * nothing".
 */
export function defaultVisibility(
  scope: ObserverScope = globalThis,
  options: ObserverOptions = {},
): IslandVisibility {
  const Observer = scope.IntersectionObserver;
  return Observer === undefined ? eagerVisibility() : observerVisibility(Observer, options);
}
