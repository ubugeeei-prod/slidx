/** Searchable actions and slide navigation, one keystroke from anywhere. */

import type { CanvasSurface } from "./canvas";
import {
  commandAction,
  commandMatches,
  firstEnabledCommand,
  foldCommandQuery,
  lastEnabledCommand,
  type CommandEntry,
} from "./command-palette-model";
import { applyCommandPaletteStyles } from "./command-palette-styles";
import { element, fill } from "./dom";
import type { BlockKind, SlideKind } from "./operations";
import type { Surface } from "./outline";
import type { EditorState, Session } from "./session";
import {
  resolveTextStyle,
  TEXT_TONES,
  textFaceOf,
  textStyleOperation,
  textToneOf,
  textWeightOf,
  toggleTextFace,
  toggleTextWeight,
  withTextTone,
  type TextStyleTarget,
  type TextTone,
} from "./text-inspector";

export interface CommandPaletteActions {
  addSlide(): void;
  createSlide(kind: SlideKind): void;
  focusCanvas(): void;
  canvasFocused(): boolean;
  present(): void;
  audience(): void;
  print(): void;
}

export interface CommandPalette extends Surface {
  trigger: HTMLButtonElement;
  keydown(event: KeyboardEvent): void;
  show(): void;
  hide(): void;
}

let nextPalette = 0;

export function createCommandPalette(
  session: Session,
  canvas: CanvasSurface,
  actions: CommandPaletteActions,
): CommandPalette {
  const number = ++nextPalette;
  const resultsId = `slidx-command-results-${number}`;
  const input = element("input", {
    type: "text",
    class: "slidx-command-input",
    placeholder: "Search actions and slides",
    autocomplete: "off",
    spellcheck: "false",
    role: "combobox",
    "aria-label": "Search actions and slides",
    "aria-autocomplete": "list",
    "aria-expanded": "true",
    "aria-controls": resultsId,
  }) as HTMLInputElement;
  const results = element("div", {
    id: resultsId,
    class: "slidx-command-results",
    role: "listbox",
    "aria-label": "Commands and slides",
  });
  const empty = element("p", { class: "slidx-command-empty", hidden: true }, [
    "No matching action or slide.",
  ]);
  const dialog = element(
    "section",
    {
      class: "slidx-command-dialog",
      role: "dialog",
      "aria-modal": "true",
      "aria-label": "Commands and slide search",
    },
    [
      element("div", { class: "slidx-command-search" }, [
        element("span", { class: "slidx-command-search-mark", "aria-hidden": "true" }, ["⌕"]),
        input,
      ]),
      results,
      empty,
      element("footer", { class: "slidx-command-footer", "aria-hidden": "true" }, [
        element("span", {}, [element("kbd", {}, ["↑↓"]), "Navigate"]),
        element("span", {}, [element("kbd", {}, ["↵"]), "Run"]),
        element("span", {}, [element("kbd", {}, ["Esc"]), "Close"]),
      ]),
    ],
  );
  const root = element("aside", { class: "slidx-command-palette" }, [dialog]);
  root.hidden = true;
  const trigger = element(
    "button",
    {
      type: "button",
      class: "slidx-command-trigger",
      "aria-label": "Search commands and slides",
      "aria-expanded": "false",
      title: "Search commands and slides (⌘/Ctrl K)",
    },
    [
      element("span", { class: "slidx-command-trigger-icon", "aria-hidden": "true" }, ["⌕"]),
      element("span", { class: "slidx-command-trigger-label" }, ["Commands"]),
      element("kbd", { "aria-hidden": "true" }, ["⌘K"]),
    ],
  ) as HTMLButtonElement;
  applyCommandPaletteStyles(root.ownerDocument);

  let state: EditorState | undefined;
  let shown: CommandEntry[] = [];
  let active = -1;

  function entries(current: EditorState): CommandEntry[] {
    const selected = current.selection.slide;
    const hasSlides = current.slides.length > 0;
    const readOnly = current.canEdit === false;
    const commands: CommandEntry[] = [
      commandAction(
        "undo",
        "Undo",
        "Take back the last deck change",
        "history back",
        "↶",
        () => session.undo(),
        readOnly || current.writing || !current.canUndo,
      ),
      commandAction(
        "redo",
        "Redo",
        "Restore the change just taken back",
        "history forward",
        "↷",
        () => session.redo(),
        readOnly || current.writing || !current.canRedo,
      ),
      commandAction(
        "add-slide",
        "Add slide",
        "Create after the current slide",
        "new insert page",
        "+",
        () => actions.addSlide(),
        readOnly,
      ),
      commandAction(
        "add-content",
        "Add content",
        "Insert into the current slide",
        "block text list quote",
        "+",
        () => canvas.addContent(),
        readOnly || !hasSlides,
      ),
      commandAction(
        "edit-text",
        "Edit slide text",
        "Put the caret into the rendered slide",
        "type write",
        "T",
        () => canvas.focusText(),
        readOnly || !hasSlides,
      ),
      commandAction(
        "speaker-notes",
        readOnly ? "Read speaker notes" : "Write speaker notes",
        readOnly
          ? "Review the current slide's speaking notes"
          : "Keep the current slide on screen while you write",
        "notes script narration talk",
        "N",
        () => canvas.focusNotes(),
        !hasSlides,
      ),
      commandAction(
        "visual",
        "Visual mode",
        "Show the rendered slide",
        "canvas preview",
        "V",
        () => canvas.showVisual(),
        !hasSlides,
      ),
      commandAction(
        "markdown",
        readOnly ? "Read slide source" : "Markdown mode",
        readOnly ? "Review this slide's Markdown" : "Edit this slide's source",
        "source code",
        "M",
        () => canvas.showMarkdown(),
        !hasSlides,
      ),
      commandAction(
        "focus-canvas",
        actions.canvasFocused() ? "Restore workspace" : "Focus canvas",
        actions.canvasFocused()
          ? "Bring the outline, inspector, and timeline back"
          : "Give the rendered slide the whole workspace",
        "workspace panels fullscreen concentrate",
        "□",
        () => actions.focusCanvas(),
      ),
      commandAction(
        "present",
        "Open presenter view",
        "Start from the selected slide",
        "play present",
        "▶",
        () => actions.present(),
        !hasSlides,
      ),
      commandAction(
        "previous",
        "Previous slide",
        "Move one slide backward",
        "navigate page",
        "←",
        () => selectSlide(selected - 1),
        !hasSlides || selected === 0,
      ),
      commandAction(
        "next",
        "Next slide",
        "Move one slide forward",
        "navigate page",
        "→",
        () => selectSlide(selected + 1),
        !hasSlides || selected >= current.slides.length - 1,
      ),
    ];
    const slides = current.slides.map(
      (slide, index): CommandEntry => ({
        id: `slide-${slide.id}-${index}`,
        kind: "slide",
        label: slide.title?.trim() || "Untitled slide",
        hint: index === selected ? `Slide ${index + 1} · current` : `Slide ${index + 1}`,
        keywords: `slide page ${index + 1}`,
        mark: String(index + 1),
        disabled: false,
        act: () => selectSlide(index),
      }),
    );
    const moreOutputs = [
      commandAction(
        "audience",
        "Open audience view",
        "Show the selected slide without editor chrome",
        "projector fullscreen audience deliver",
        "▣",
        () => actions.audience(),
        !hasSlides,
      ),
      commandAction(
        "print",
        "Open print / PDF view",
        "Put every stop into one printable document",
        "export download handout print pdf deliver",
        "⇩",
        () => actions.print(),
        !hasSlides,
      ),
    ];

    // A selection makes text commands the immediate context. They lead the
    // zero-query view while ordinary actions and every slide remain searchable.
    // With no selection this is the same deck-first list as before.
    return [
      ...textCommands(current),
      ...creationCommands(current),
      ...commands,
      ...slides,
      ...moreOutputs,
    ];
  }

  function creationCommands(current: EditorState): CommandEntry[] {
    const hasSlides = current.slides.length > 0;
    const locked = current.canEdit === false || current.writing;
    const blockHint =
      current.selection.block === undefined
        ? `Add to the end of slide ${current.selection.slide + 1}`
        : "Place after the selected block";
    const slideHint = hasSlides
      ? `Start after slide ${current.selection.slide + 1}`
      : "Start the first slide";

    const blocks: ReadonlyArray<{
      kind: BlockKind;
      label: string;
      hint: string;
      mark: string;
      keywords: string;
    }> = [
      {
        kind: "heading",
        label: "Insert heading",
        hint: `Section title · ${blockHint}`,
        mark: "H",
        keywords: "add create block title section",
      },
      {
        kind: "text",
        label: "Insert text",
        hint: `Paragraph · ${blockHint}`,
        mark: "¶",
        keywords: "add create block body paragraph copy",
      },
      {
        kind: "list",
        label: "Insert list",
        hint: `Key points · ${blockHint}`,
        mark: "•",
        keywords: "add create block bullets points",
      },
      {
        kind: "quote",
        label: "Insert quote",
        hint: `Takeaway · ${blockHint}`,
        mark: "“",
        keywords: "add create block quotation statement",
      },
    ];
    const slides: ReadonlyArray<{
      kind: SlideKind;
      label: string;
      hint: string;
      mark: string;
      keywords: string;
    }> = [
      {
        kind: "title-body",
        label: "New title + body slide",
        hint: `Lead, then explain · ${slideHint}`,
        mark: "▤",
        keywords: "add create page heading paragraph",
      },
      {
        kind: "statement",
        label: "New statement slide",
        hint: `One idea, full frame · ${slideHint}`,
        mark: "“",
        keywords: "add create page quote takeaway",
      },
      {
        kind: "comparison",
        label: "New comparison slide",
        hint: `Two equal sides · ${slideHint}`,
        mark: "⇄",
        keywords: "add create page versus split columns",
      },
      {
        kind: "points",
        label: "New key points slide",
        hint: `A title and list · ${slideHint}`,
        mark: "≡",
        keywords: "add create page bullets agenda list",
      },
    ];

    return [
      ...blocks.map(
        (choice): CommandEntry => ({
          id: `insert-${choice.kind}`,
          kind: "block",
          label: choice.label,
          hint: choice.hint,
          keywords: choice.keywords,
          mark: choice.mark,
          disabled: locked || !hasSlides,
          searchOnly: true,
          act: () => canvas.insertContent(choice.kind),
        }),
      ),
      ...slides.map(
        (choice): CommandEntry => ({
          id: `create-${choice.kind}`,
          kind: "new",
          label: choice.label,
          hint: choice.hint,
          keywords: choice.keywords,
          mark: choice.mark,
          disabled: locked,
          searchOnly: true,
          act: () => actions.createSlide(choice.kind),
        }),
      ),
    ];
  }

  function textCommands(current: EditorState): CommandEntry[] {
    const resolved = resolveTextStyle(current, {
      bodyOf: (slide) => session.bodyOf(slide),
      blocksOf: (slide) => session.blocksOf(slide),
    });
    if ("problem" in resolved) return [];

    const { target } = resolved;
    const locked = current.canEdit === false || current.writing;
    const bold = textWeightOf(target.attributes) === "bold";
    const mono = textFaceOf(target.attributes) === "code";
    const selectedTone = textToneOf(target.attributes);
    const commit = (attributes: Parameters<typeof textStyleOperation>[1]) =>
      session.run(textStyleOperation(target, attributes));
    const choices = TEXT_TONES.map(({ value, label }) =>
      toneCommand(target, value, label, selectedTone, locked, () =>
        commit(withTextTone(target.attributes, value)),
      ),
    );

    return [
      textCommand(
        "text-bold",
        bold ? "Use regular text weight" : "Bold selected text",
        bold
          ? `Remove bold emphasis from ${quoted(target.text)}`
          : `Add bold emphasis to ${quoted(target.text)} · ⌘/Ctrl B`,
        "bold emphasis regular weight strong",
        "B",
        () => commit(toggleTextWeight(target.attributes)),
        locked,
      ),
      textCommand(
        "text-face",
        mono ? "Use theme typeface" : "Use mono typeface",
        mono
          ? `Return ${quoted(target.text)} to the deck typeface`
          : `Set ${quoted(target.text)} in the deck's mono typeface`,
        "mono code typeface font theme",
        "<>",
        () => commit(toggleTextFace(target.attributes)),
        locked,
      ),
      ...choices,
      textCommand(
        "text-finish",
        "Finish text styling",
        `Keep the block selected and leave ${quoted(target.text)}`,
        "done close escape selection",
        "✓",
        () => {
          canvas.finishTextSelection();
          session.select({ text: undefined, range: undefined });
        },
      ),
    ];
  }

  function selectSlide(slide: number): void {
    if (!state || slide < 0 || slide >= state.slides.length) return;
    session.select({ slide, block: undefined, range: undefined, text: undefined });
  }

  function draw(reset: boolean): void {
    if (!state) return;
    const previous = shown[active]?.id;
    const query = foldCommandQuery(input.value);
    shown = entries(state)
      .filter((entry) => commandMatches(entry, query))
      .slice(0, 14);

    active = reset ? firstEnabledCommand(shown) : shown.findIndex((entry) => entry.id === previous);
    if (active < 0 || shown[active]?.disabled) active = firstEnabledCommand(shown);

    fill(
      results,
      shown.map((entry, index) => row(entry, index)),
    );
    empty.hidden = shown.length !== 0;
    updateActive(false);
  }

  function row(entry: CommandEntry, index: number): HTMLButtonElement {
    const id = `slidx-command-${number}-${index}`;
    const button = element(
      "button",
      {
        id,
        type: "button",
        class: "slidx-command-item",
        role: "option",
        tabindex: -1,
        "aria-selected": String(index === active),
        "aria-disabled": String(entry.disabled),
        "aria-current": entry.current ? "true" : undefined,
        "data-command-tone": entry.tone,
      },
      [
        element("span", { class: "slidx-command-kind", "aria-hidden": "true" }, [entry.mark]),
        element("span", { class: "slidx-command-copy" }, [
          element("span", { class: "slidx-command-title" }, [entry.label]),
          element("span", { class: "slidx-command-hint" }, [entry.hint]),
        ]),
        element("span", { class: "slidx-command-type" }, [entry.current ? "Current" : entry.kind]),
      ],
    ) as HTMLButtonElement;
    button.addEventListener("pointermove", () => {
      if (entry.disabled || active === index) return;
      active = index;
      updateActive(false);
    });
    button.addEventListener("click", () => run(entry));
    return button;
  }

  function updateActive(scroll: boolean): void {
    const rows = [...results.querySelectorAll<HTMLElement>(".slidx-command-item")];
    rows.forEach((row, index) => row.setAttribute("aria-selected", String(index === active)));
    const selected = rows[active];
    if (!selected) {
      input.removeAttribute("aria-activedescendant");
      return;
    }
    input.setAttribute("aria-activedescendant", selected.id);
    if (scroll) selected.scrollIntoView?.({ block: "nearest" });
  }

  function move(by: number): void {
    if (shown.length === 0) return;
    let next = active;
    for (let tried = 0; tried < shown.length; tried += 1) {
      next = (next + by + shown.length) % shown.length;
      if (!shown[next]?.disabled) {
        active = next;
        updateActive(true);
        return;
      }
    }
  }

  function run(entry: CommandEntry | undefined): void {
    if (!entry || entry.disabled) return;
    hide(false);
    void entry.act();
  }

  function show(): void {
    if (!root.hidden) return;
    root.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
    input.value = "";
    draw(true);
    input.focus();
    input.select();
  }

  function hide(restore = true): void {
    if (root.hidden) return;
    root.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
    if (restore) trigger.focus();
  }

  trigger.addEventListener("click", show);
  input.addEventListener("input", () => draw(true));
  root.addEventListener("pointerdown", (event) => {
    if (event.target === root) hide();
  });

  return {
    root,
    trigger,
    show,
    hide: () => hide(),
    render(next) {
      state = next;
      if (!root.hidden) draw(false);
    },
    keydown(event) {
      const key = event.key.toLowerCase();
      const primary = event.metaKey || event.ctrlKey;

      if (primary && key === "k") {
        event.preventDefault();
        if (root.hidden) show();
        else hide();
        return;
      }
      if (root.hidden || event.isComposing) return;

      if (key === "escape") {
        event.preventDefault();
        hide();
      } else if (key === "arrowdown") {
        event.preventDefault();
        move(1);
      } else if (key === "arrowup") {
        event.preventDefault();
        move(-1);
      } else if (key === "home") {
        event.preventDefault();
        active = firstEnabledCommand(shown);
        updateActive(true);
      } else if (key === "end") {
        event.preventDefault();
        active = lastEnabledCommand(shown);
        updateActive(true);
      } else if (key === "enter") {
        event.preventDefault();
        run(shown[active]);
      } else if (key === "tab") {
        event.preventDefault();
        input.focus();
      }
    },
  };
}

function textCommand(
  id: string,
  label: string,
  hint: string,
  keywords: string,
  mark: string,
  act: () => void | Promise<void>,
  disabled = false,
): CommandEntry {
  return { id, kind: "text", label, hint, keywords, mark, disabled, act };
}

function toneCommand(
  target: TextStyleTarget,
  tone: TextTone,
  label: string,
  selected: TextTone | undefined,
  locked: boolean,
  act: () => void | Promise<void>,
): CommandEntry {
  const current = tone === selected;
  const hint = current
    ? `Current tone for ${quoted(target.text)}`
    : tone === "theme"
      ? `Use the inherited text colour for ${quoted(target.text)}`
      : `Apply the deck's ${label.toLowerCase()} tone to ${quoted(target.text)}`;

  return {
    id: `text-tone-${tone}`,
    kind: "text",
    label: `${label} text tone`,
    hint,
    keywords: `tone color colour text ${label.toLowerCase()}`,
    mark: "●",
    disabled: locked || current,
    current,
    tone,
    act,
  };
}

function quoted(value: string): string {
  const characters = Array.from(
    new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(value),
    ({ segment }) => segment,
  );
  const shown = characters.length > 36 ? `${characters.slice(0, 35).join("")}…` : value;
  return `“${shown}”`;
}
