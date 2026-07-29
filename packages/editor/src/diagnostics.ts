/**
 * What the linter found, next to the slide it found it on.
 *
 * The pipeline returns findings with every parse, so this costs one render
 * rather than a second analysis. Showing them here is the difference between a
 * contrast failure caught at the desk and one discovered from a stage.
 *
 * The slide the author is on comes first. A finding about another slide is
 * still listed, because "which slide has the problem" is the question a
 * diagnostics panel exists to answer.
 */

import { element, fill } from "./dom";
import type { Finding } from "./client";
import type { Surface } from "./outline";
import type { EditorState } from "./session";

export interface DiagnosticsHandlers {
  select(slide: number): void;
}

export function createDiagnostics(handlers: DiagnosticsHandlers): Surface {
  const list = element("ul", { class: "slidx-findings" });
  const root = element("section", { class: "slidx-diagnostics", "aria-label": "Diagnostics" }, [
    list,
  ]);

  return {
    root,
    render(state) {
      const findings = order(state.diagnostics, state.selection.slide);
      root.setAttribute("data-empty", String(findings.length === 0 && !state.refusal));

      fill(list, [
        ...(state.problem ? [message("error", state.problem)] : []),
        ...(state.refusal ? [message("warning", refusalText(state.refusal))] : []),
        ...findings.map((finding) => row(finding, handlers)),
      ]);
    },
  };
}

function order(findings: Finding[], slide: number): Finding[] {
  return [...findings].sort((a, b) => rank(a, slide) - rank(b, slide));
}

function rank(finding: Finding, slide: number): number {
  return (finding.slideIndex === slide ? 0 : 10) + (finding.severity === "error" ? 0 : 1);
}

function row(finding: Finding, handlers: DiagnosticsHandlers): HTMLElement {
  const where =
    finding.slideIndex === undefined ? "deck" : `slide ${(finding.slideIndex ?? 0) + 1}`;

  const item = element("li", { class: "slidx-finding", "data-severity": finding.severity }, [
    element("span", { class: "slidx-finding-where" }, [where]),
    element("span", { class: "slidx-finding-message" }, [finding.message]),
    element("span", { class: "slidx-finding-code" }, [finding.code]),
  ]);

  if (finding.slideIndex !== undefined) {
    item.setAttribute("role", "button");
    item.setAttribute("tabindex", "0");
    item.addEventListener("click", () => handlers.select(finding.slideIndex!));
  }

  return item;
}

function message(severity: string, text: string): HTMLElement {
  return element("li", { class: "slidx-finding", "data-severity": severity }, [
    element("span", { class: "slidx-finding-message" }, [text]),
  ]);
}

/**
 * A refusal, in words.
 *
 * The editor builds operations from a deck it parsed a moment ago, so naming a
 * slide that has since gone is ordinary traffic. It is worth one line and never
 * worth an alert.
 */
function refusalText(refusal: { error: string; [key: string]: unknown }): string {
  const detail = Object.entries(refusal)
    .filter(([key]) => key !== "error")
    .map(([, value]) => String(value))
    .join(", ");

  return detail ? `${spaced(refusal.error)}: ${detail}` : spaced(refusal.error);
}

function spaced(name: string): string {
  return name.replace(/([A-Z])/g, " $1").toLowerCase();
}
