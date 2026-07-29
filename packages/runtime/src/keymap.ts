/**
 * The keyboard, as a table.
 *
 * A speaker drives a deck with their hands off the screen — from a clicker,
 * from the keyboard, in the dark. That makes the binding list something a
 * person needs to *see*, so it is data rather than a switch statement buried
 * in a handler: it can be listed on screen, rebound, and checked for the one
 * mistake nobody catches by reading, which is two commands claiming one key.
 *
 * Only commands the page actually supplied are bound. A projector page has no
 * timer, so `t` there is inert rather than throwing into a slide — and the
 * help lists what this page can do rather than a catalogue of features it
 * does not have.
 */

/** Everything a deck page can be asked to do from the keyboard. */
export type Command =
  | "next"
  | "previous"
  | "first"
  | "last"
  | "overview"
  | "presenter"
  | "blackout"
  | "fullscreen"
  | "toggleTimer"
  | "resetTimer"
  | "toggleDemo"
  | "help";

/** One row of the table. */
export interface Binding {
  command: Command;
  /** Alternatives, in the order a help panel should read them. */
  keys: string[];
  description: string;
}

export interface Keymap {
  /** Runs the bound command, and consumes the event only if one ran. */
  handle(event: KeyboardEvent): void;
  /** The bindings this page can actually act on, for a help panel. */
  bindings(): Binding[];
}

export interface KeymapOptions {
  /** What this page can do. A command that is absent is simply not bound. */
  commands: Partial<Record<Command, () => void>>;
  bindings?: Binding[];
}

/**
 * The default table.
 *
 * The navigation keys are the ones presentation remotes actually send —
 * PageDown and the arrows, not letters. The letters are the conventions
 * presenters already have in their fingers from every other tool: `b` blacks
 * out, `f` goes fullscreen, `?` asks for help.
 */
export const DEFAULT_BINDINGS: Binding[] = [
  {
    command: "next",
    keys: ["ArrowRight", " ", "PageDown", "ArrowDown", "Enter"],
    description: "Next stop",
  },
  {
    command: "previous",
    keys: ["ArrowLeft", "PageUp", "ArrowUp", "Backspace"],
    description: "Previous stop",
  },
  { command: "first", keys: ["Home"], description: "First stop on this slide" },
  { command: "last", keys: ["End"], description: "Last stop on this slide" },
  { command: "overview", keys: ["o"], description: "Slide overview" },
  { command: "presenter", keys: ["p"], description: "Presenter view" },
  // The oldest presenter shortcut there is, and the one that matters most:
  // it takes the audience's eyes off the slide and puts them on you.
  { command: "blackout", keys: ["b", "."], description: "Black out the screen" },
  { command: "fullscreen", keys: ["f"], description: "Fullscreen" },
  { command: "toggleTimer", keys: ["t"], description: "Start or pause the timer" },
  { command: "resetTimer", keys: ["r"], description: "Reset the timer" },
  // One key, no modifier, and only bound on a slide that declared a demo. It
  // is pressed exactly once, in front of an audience, by someone whose live
  // demo has just died — so it cannot be a chord and it cannot need aiming.
  { command: "toggleDemo", keys: ["d"], description: "Switch the demo for its recording" },
  { command: "help", keys: ["?"], description: "Show these shortcuts" },
];

export function createKeymap(options: KeymapOptions): Keymap {
  const table = options.bindings ?? DEFAULT_BINDINGS;

  // Only what this page supplied. Binding a key to nothing would consume it
  // from the browser and give nothing back.
  const available = table.filter((binding) => options.commands[binding.command]);

  const byKey = new Map<string, Command>();
  for (const binding of available) {
    for (const key of binding.keys) byKey.set(normalise(key), binding.command);
  }

  return {
    handle(event) {
      // A modifier means the browser's shortcut, not ours. ⌘R and ⌘← are the
      // two a speaker reaches for when something has gone wrong.
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (isTyping(event.target)) return;

      const command = byKey.get(normalise(event.key));
      if (!command) return;

      options.commands[command]?.();
      event.preventDefault();
    },

    bindings: () => [...available],
  };
}

/**
 * Keys compare case-insensitively.
 *
 * Caps lock is easy to leave on in a dark room, and a deck that stops
 * responding because of it looks broken rather than pedantic.
 */
function normalise(key: string): string {
  return key.toLowerCase();
}

/**
 * True when the key belongs to a field rather than to the deck.
 *
 * The editor and the audience channel both put inputs on the page, and a
 * space bar that advances the slide while someone types a question is a bug
 * reported as the deck being haunted.
 */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;

  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}

/** How a key is drawn on a keyboard, rather than how the DOM names it. */
const KEY_LABELS: Record<string, string> = {
  " ": "Space",
  ArrowRight: "→",
  ArrowLeft: "←",
  ArrowUp: "↑",
  ArrowDown: "↓",
};

/**
 * One binding, as a help panel shows it.
 *
 * `ArrowRight` is what the DOM calls it and `→` is what is printed on the
 * key. A help panel that used the first would be a help panel people have to
 * translate.
 */
export function formatBinding(binding: Binding): string {
  return binding.keys.map((key) => KEY_LABELS[key] ?? key).join(" / ");
}
