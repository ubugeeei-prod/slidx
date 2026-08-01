/** Pure search and selection helpers for the command palette. */

export interface CommandEntry {
  id: string;
  kind: "action" | "slide" | "text" | "block" | "new";
  label: string;
  hint: string;
  keywords: string;
  mark: string;
  disabled: boolean;
  /** The semantic tone whose real editor colour the command mark previews. */
  tone?: string | undefined;
  /** A disabled choice because it already describes the selected words. */
  current?: boolean | undefined;
  /** A precise action kept out of the calm zero-query overview. */
  searchOnly?: boolean | undefined;
  act(): void | Promise<void>;
}

export function commandAction(
  id: string,
  label: string,
  hint: string,
  keywords: string,
  mark: string,
  act: () => void | Promise<void>,
  disabled = false,
): CommandEntry {
  return { id, kind: "action", label, hint, keywords, mark, disabled, act };
}

export function foldCommandQuery(value: string): string {
  return value.normalize("NFKC").toLocaleLowerCase().trim();
}

export function commandMatches(entry: CommandEntry, query: string): boolean {
  if (query.length === 0) return entry.searchOnly !== true;
  return foldCommandQuery(`${entry.label} ${entry.hint} ${entry.keywords}`).includes(query);
}

export function firstEnabledCommand(entries: CommandEntry[]): number {
  return entries.findIndex((entry) => !entry.disabled);
}

export function lastEnabledCommand(entries: CommandEntry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (!entries[index]!.disabled) return index;
  }
  return -1;
}
