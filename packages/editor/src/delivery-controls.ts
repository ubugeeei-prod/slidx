/** Visual controls for how a slide arrives, how long it gets, and whether it can go. */

import type { SlideSummary, TransitionChoice } from "./client";
import { element } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";
import { formatSeconds } from "./storyboard/plan";
import { applyDeliveryControlStyles } from "./delivery-controls-styles";

type Run = (op: EditOp) => void;

const BUDGETS = [
  { seconds: 30, label: "30s" },
  { seconds: 60, label: "1m" },
  { seconds: 120, label: "2m" },
];

/** The deck-wide transition inherited by slides that stay silent. */
export function deckTransitionField(state: EditorState, run: Run): HTMLElement {
  applyDeliveryControlStyles(document);
  const written = state.slides[0]?.frontmatter ?? {};
  const authored = transitionToken(written["transition"]);
  const current = authored ?? "none";

  return transitionField({
    label: "Default transition",
    choices: state.transitions,
    current,
    notice:
      has(written, "transition") && !known(state.transitions, authored)
        ? `“${shown(written["transition"])}” is not available in this renderer.`
        : undefined,
    choose: (id) => run({ op: "setField", slide: 0, key: "transition", value: id }),
  });
}

/** Everything about delivering the selected slide, kept as one designed cluster. */
export function slideDelivery(
  state: EditorState,
  slide: SlideSummary,
  index: number,
  run: Run,
): HTMLElement {
  applyDeliveryControlStyles(document);
  const frontmatter = slide.frontmatter ?? {};
  const controls = [
    ...(index === 0 ? [] : [slideTransition(state, frontmatter, index, run)]),
    budgetField(slide, index, frontmatter, run),
    optionalField(slide, index, frontmatter, run),
  ];

  return element("section", { class: "slidx-delivery", "aria-label": "Slide delivery" }, [
    element("span", { class: "slidx-delivery-label" }, ["Delivery"]),
    ...controls,
  ]);
}

function slideTransition(
  state: EditorState,
  frontmatter: Record<string, unknown>,
  slide: number,
  run: Run,
): HTMLElement {
  const authored = transitionToken(frontmatter["transition"]);
  const current = has(frontmatter, "transition") ? authored : "inherit";
  const inherited = transitionToken(state.slides[0]?.frontmatter?.["transition"]) ?? "none";
  const inheritedName = state.transitions.find((choice) => choice.id === inherited)?.name ?? "Cut";

  return transitionField({
    label: "Arrival",
    choices: state.transitions,
    current,
    inherit: `Deck default · ${inheritedName}`,
    notice:
      has(frontmatter, "transition") && !known(state.transitions, authored)
        ? `“${shown(frontmatter["transition"])}” is not available in this renderer.`
        : undefined,
    choose: (id) =>
      run(
        id === "inherit"
          ? { op: "removeField", slide, key: "transition" }
          : { op: "setField", slide, key: "transition", value: id },
      ),
  });
}

interface TransitionFieldOptions {
  label: string;
  choices: TransitionChoice[];
  current: string | undefined;
  inherit?: string | undefined;
  notice?: string | undefined;
  choose(id: string): void;
}

function transitionField(options: TransitionFieldOptions): HTMLElement {
  const detail =
    options.current === "inherit"
      ? options.inherit
      : options.choices.find((choice) => choice.id === options.current)?.description;
  const cards = [
    ...(options.inherit
      ? [transitionButton("inherit", "Inherit", options.inherit, false, options)]
      : []),
    ...options.choices.map((choice) =>
      transitionButton(choice.id, choice.name, choice.description, choice.moves, options),
    ),
  ];

  return element("div", { class: "slidx-transition-field" }, [
    element("span", { class: "slidx-delivery-name" }, [options.label]),
    ...(options.notice
      ? [element("p", { class: "slidx-delivery-notice", role: "status" }, [options.notice])]
      : []),
    element(
      "div",
      { class: "slidx-transition-choices", role: "group", "aria-label": options.label },
      cards,
    ),
    ...(detail ? [element("p", { class: "slidx-transition-detail" }, [detail])] : []),
  ]);
}

function transitionButton(
  id: string,
  name: string,
  description: string,
  moves: boolean,
  options: TransitionFieldOptions,
): HTMLButtonElement {
  const selected = options.current === id;
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-transition-choice",
      "data-transition": id,
      "data-moves": moves,
      "aria-pressed": String(selected),
      "aria-label": `${name}: ${description}`,
      title: description,
    },
    [
      transitionMiniature(id),
      element("span", { class: "slidx-transition-copy" }, [
        element("strong", {}, [name]),
        element("span", {}, [description]),
      ]),
    ],
  ) as HTMLButtonElement;
  button.addEventListener("click", () => {
    if (!selected) options.choose(id);
  });
  return button;
}

function transitionMiniature(id: string): HTMLElement {
  return element(
    "span",
    { class: "slidx-transition-preview", "data-transition-preview": id, "aria-hidden": "true" },
    [
      element("span", { class: "slidx-transition-preview-from" }, [element("i"), element("i")]),
      element("span", { class: "slidx-transition-preview-to" }, [element("i")]),
    ],
  );
}

function budgetField(
  slide: SlideSummary,
  index: number,
  frontmatter: Record<string, unknown>,
  run: Run,
): HTMLElement {
  const authored = shown(frontmatter["budget"]);
  const declared = has(frontmatter, "budget");
  const valid = slide.budgetSeconds !== undefined;
  const choices = [
    budgetButton("estimate", "Estimate", !declared, () => {
      if (declared) run({ op: "removeField", slide: index, key: "budget" });
    }),
    ...BUDGETS.map((budget) =>
      budgetButton(
        String(budget.seconds),
        budget.label,
        slide.budgetSeconds === budget.seconds,
        () => run({ op: "setField", slide: index, key: "budget", value: budget.label }),
      ),
    ),
  ];
  const custom = element("input", {
    type: "text",
    class: "slidx-budget-custom",
    "data-key": "budget",
    "aria-label": "Custom slide budget",
    placeholder: "e.g. 1m30s",
  }) as HTMLInputElement;
  custom.value = authored;
  custom.addEventListener("blur", () => {
    const value = custom.value.trim();
    if (value === authored) return;
    run(
      value.length === 0
        ? { op: "removeField", slide: index, key: "budget" }
        : { op: "setField", slide: index, key: "budget", value },
    );
  });

  const pace = paceOf(slide, declared, valid);
  return element("div", { class: "slidx-budget-field" }, [
    element("span", { class: "slidx-delivery-name" }, ["Timing"]),
    element(
      "div",
      { class: "slidx-budget-choices", role: "group", "aria-label": "Slide budget" },
      choices,
    ),
    element("label", { class: "slidx-budget-entry" }, [element("span", {}, ["Custom"]), custom]),
    element("p", { class: "slidx-budget-status", "data-state": pace.state }, [pace.label]),
  ]);
}

function budgetButton(
  value: string,
  label: string,
  selected: boolean,
  choose: () => void,
): HTMLButtonElement {
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-budget-choice",
      "data-budget": value,
      "aria-pressed": String(selected),
    },
    [label],
  ) as HTMLButtonElement;
  button.addEventListener("click", () => {
    if (!selected) choose();
  });
  return button;
}

function paceOf(
  slide: SlideSummary,
  declared: boolean,
  valid: boolean,
): { state: string; label: string } {
  const estimate = slide.estimatedSeconds;
  const budget = slide.budgetSeconds;
  if (declared && !valid)
    return { state: "invalid", label: "This value has no resolved duration." };
  if (budget === undefined) {
    return estimate > 0
      ? { state: "estimate", label: `≈ ${formatSeconds(estimate)} from speaker notes` }
      : { state: "empty", label: "Add notes or choose a budget to give this slide a length." };
  }
  if (estimate === 0) {
    return { state: "budget", label: `${formatSeconds(budget)} reserved · no notes yet` };
  }

  const difference = budget - estimate;
  if (difference < 0) {
    return {
      state: "over",
      label: `≈ ${formatSeconds(estimate)} spoken · ${formatSeconds(-difference)} over budget`,
    };
  }
  if (difference > 0) {
    return {
      state: "under",
      label: `≈ ${formatSeconds(estimate)} spoken · ${formatSeconds(difference)} spare`,
    };
  }
  return { state: "exact", label: `≈ ${formatSeconds(estimate)} spoken · right on budget` };
}

function optionalField(
  slide: SlideSummary,
  index: number,
  frontmatter: Record<string, unknown>,
  run: Run,
): HTMLElement {
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-optional-choice",
      "data-key": "optional",
      "data-optional": String(slide.optional),
      "aria-pressed": String(slide.optional),
      "aria-label": slide.optional
        ? "Keep this slide in the core talk"
        : "Mark this slide as safe to skip",
    },
    [
      element("span", { class: "slidx-optional-mark", "aria-hidden": "true" }, [
        slide.optional ? "CUT" : "CORE",
      ]),
      element("span", { class: "slidx-optional-copy" }, [
        element("strong", {}, ["Safe to skip"]),
        element("span", {}, [
          slide.optional
            ? "Prepared as a cut if the talk runs long."
            : "Keep this slide in the core story.",
        ]),
      ]),
    ],
  ) as HTMLButtonElement;
  button.addEventListener("click", () =>
    run(
      slide.optional && has(frontmatter, "optional")
        ? { op: "removeField", slide: index, key: "optional" }
        : { op: "setField", slide: index, key: "optional", value: true },
    ),
  );
  return button;
}

function transitionToken(value: unknown): string | undefined {
  if (value === false) return "none";
  return typeof value === "string" && value.trim().length > 0
    ? value.trim().toLocaleLowerCase()
    : undefined;
}

function known(choices: TransitionChoice[], value: string | undefined): boolean {
  return value !== undefined && choices.some((choice) => choice.id === value);
}

function has(values: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(values, key);
}

function shown(value: unknown): string {
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return value === null || value === undefined ? "" : "a structured value";
}
