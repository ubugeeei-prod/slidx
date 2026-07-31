/**
 * Showing diagnostics to whoever is looking.
 *
 * The same findings reach a terminal during `vite build` and an overlay during
 * `vite dev`. Both say the same thing in the same words, because a message
 * that is worded differently in two places reads as two different problems.
 */

import type { Finding } from "@slidxjs/wasm";

/** Findings grouped the way a person reads them. */
export interface Report {
  blocking: Finding[];
  warnings: Finding[];
}

export function groupFindings(findings: Finding[]): Report {
  return {
    blocking: findings.filter((finding) => finding.severity === "error"),
    warnings: findings.filter((finding) => finding.severity !== "error"),
  };
}

/**
 * One finding, on one line, with its help indented under it.
 *
 * The slide number comes first because it is what a person acts on: they open
 * that slide. The rule code comes last, for looking up or suppressing.
 */
export function formatFinding(finding: Finding, titles: readonly (string | null)[]): string {
  const where =
    finding.slideIndex === undefined
      ? "deck"
      : `slide ${finding.slideIndex + 1}${title(titles[finding.slideIndex])}`;

  const head = `${finding.severity}  ${where}: ${finding.message}  [${finding.code}]`;
  return finding.help ? `${head}\n        ${finding.help}` : head;
}

function title(value: string | null | undefined): string {
  return value ? ` (${value})` : "";
}

/** The whole report, for a terminal. */
export function formatReport(findings: Finding[], titles: readonly (string | null)[]): string {
  return findings.map((finding) => formatFinding(finding, titles)).join("\n");
}

/**
 * The sentence shown when a build is stopped.
 *
 * It names the count and the escape hatch in the same breath: someone who has
 * decided to ship anyway should not have to search for how.
 */
export function blockingSummary(count: number): string {
  return (
    `${count} blocking diagnostic${count === 1 ? "" : "s"}. ` +
    "Fix them, or pass `failOnDiagnostics: false` to build anyway."
  );
}
