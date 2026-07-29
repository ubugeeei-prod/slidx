/**
 * Reading an island's props out of the markup.
 *
 * Props reach the browser as JSON in an attribute, which means they arrive as
 * text that something else generated: a compiler, a template, sometimes a
 * generator pulling from a CMS. Any of those can emit something that is not a
 * JSON object, and none of them are around at presentation time to be asked.
 *
 * So this never throws. A malformed attribute costs the island its props, not
 * the slide — a chart with no data still leaves the slide readable, and a
 * `JSON.parse` that escapes takes every island after it on the page down with
 * it. The problem is returned rather than reported here so this module stays
 * free of the reporter and can be read as a pure function.
 */

import type { IslandProps } from "./contract";

export interface ParsedProps {
  props: IslandProps;
  /** One line naming what was wrong, or absent when nothing was. */
  problem?: string;
}

/**
 * `__proto__` as an own property of a props object is not an error a framework
 * survives. Vue and React both copy props with `Object.assign`, which assigns
 * rather than defines, so the inherited setter fires and the target's
 * prototype is replaced. Dropping the key here is the only place that can
 * catch it, because by the time props reach an integration they look ordinary.
 */
const FORBIDDEN_KEY = "__proto__";

/** How much of a bad attribute to quote back. Enough to find it, not enough to fill a console. */
const QUOTE_LIMIT = 60;

export function parseProps(raw: string | null | undefined): ParsedProps {
  // An absent or empty attribute is the compiler saying "no props", not a
  // mistake. Warning about it would fire on the majority of islands.
  if (raw === null || raw === undefined || raw.trim() === "") return { props: {} };

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { props: {}, problem: `props are not valid JSON (${detail}): ${quote(raw)}` };
  }

  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {
      props: {},
      problem: `props must be a JSON object, got ${describe(parsed)}: ${quote(raw)}`,
    };
  }

  const props: IslandProps = {};
  let forbidden = false;

  // Own enumerable string keys only, copied by assignment into a fresh object,
  // with the one key that would escape it removed.
  for (const [key, value] of Object.entries(parsed)) {
    if (key === FORBIDDEN_KEY) {
      forbidden = true;
      continue;
    }
    props[key] = value;
  }

  return forbidden
    ? { props, problem: `props contained a "${FORBIDDEN_KEY}" key, which was dropped` }
    : { props };
}

/** The parsed value's kind, as an author would name it. */
function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  return `a ${typeof value}`;
}

function quote(raw: string): string {
  const trimmed = raw.trim();
  return trimmed.length <= QUOTE_LIMIT ? trimmed : `${trimmed.slice(0, QUOTE_LIMIT)}…`;
}
