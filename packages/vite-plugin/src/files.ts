/**
 * Where a deck's bytes live: one source to the parser, a directory to the
 * author.
 *
 * The pipeline splices the joined source, and something has to say which file
 * each spliced byte was in. That is this module, and the rule it keeps is that
 * **nothing here decides what Markdown looks like**. A file's new bytes are
 * always a slice of the source the pipeline returned, wrapped in the whitespace
 * that file already had at its edges. slidx has exactly one writer of Markdown
 * and it is written in Rust; a second one in TypeScript would end the promise
 * that the canvas and the file are the same document.
 *
 * # What a file owns
 *
 * A file holds whole slides — the deck is joined *with* the separator, so a
 * file boundary is always a slide boundary. A deck where that is not true (an
 * unclosed fence swallows the separator between two files) is refused rather
 * than half-written.
 *
 * The bytes before the first slide are the deck's own frontmatter, which
 * belongs to no slide. They follow whichever slide is first, so deleting the
 * opening slide does not take the deck's title with it.
 */

/** A slide file, as found on disk. */
export interface DeckFile {
  /** Absolute path, for writing and for error messages. */
  path: string;
  /** Path relative to the project root, for logs. */
  label: string;
  source: string;
}

/** A half-open byte range, as the pipeline reports one. */
export interface ByteSpan {
  readonly start: number;
  readonly end: number;
}

/** The deck as the parser reads it, and where each file sits in it. */
export interface JoinedDeck {
  source: string;
  /** One span per file, in the order the files were given. */
  spans: ByteSpan[];
}

/** What to do to one file. */
export interface FileWrite {
  path: string;
  label: string;
  /** The file's new bytes, or `null` when no slide is left in it. */
  source: string | null;
}

/** A deck source before or after an edit, with its slides located in it. */
export interface LocatedSource {
  source: string;
  slides: readonly ByteSpan[];
}

/**
 * Joins slide files into the source the parser reads.
 *
 * The first file's frontmatter is the deck's, which is why it is left at the
 * very start: the parser reads deck metadata from the top of the source.
 *
 * The join has to be the exact inverse of the cut, or the byte offsets an
 * editor is holding stop meaning anything the moment a file is re-read. Two
 * rules make it one:
 *
 * **A blank line under the separator.** A separator followed immediately by
 * lines that happen to parse as YAML — `## Heading` is a YAML comment, and
 * `<!-- notes: x -->` is a key — is how a slide declares its own frontmatter,
 * so joining tightly would swallow the next file's first slide into the one
 * before it.
 *
 * **A file that already opens with a separator brings its own.** That line is
 * the opening delimiter of the slide's frontmatter block *and* the break
 * between the two slides; writing another above it would leave an empty slide
 * between them. This is the same rule the pipeline follows when it moves a
 * slide, which is what makes the two agree.
 *
 * A file with nothing in it contributes no separator, so a file this session
 * emptied does not add a blank slide to the deck. It keeps its place in the
 * list, which is what lets an undo put its slides back where they were.
 */
export function joinDeck(files: readonly DeckFile[], separator: string): JoinedDeck {
  const spans: ByteSpan[] = [];
  let source = "";

  for (const file of files) {
    const trimmed = file.source.trim();

    if (trimmed.length > 0 && source.length > 0) {
      source += opensWithSeparator(trimmed, separator) ? "\n\n" : `\n\n${separator}\n\n`;
    }

    spans.push({ start: source.length, end: source.length + trimmed.length });
    source += trimmed;
  }

  return { source, spans };
}

/** True when a file's first line is the deck separator and nothing else. */
function opensWithSeparator(source: string, separator: string): boolean {
  const first = source.split("\n", 1)[0]!.trimEnd();

  return first.length - first.trimStart().length <= 3 && first.trim() === separator;
}

/**
 * Which files an edit changed, and what they now say.
 *
 * A file whose bytes are the same as before is not in the result at all —
 * not "written with identical content". The difference is a modification time
 * that never moves and a watcher that never fires.
 */
export function planFileWrites(
  files: readonly DeckFile[],
  separator: string,
  before: LocatedSource,
  after: LocatedSource,
): FileWrite[] {
  const { spans } = joinDeck(files, separator);
  const owner = before.slides.map((slide) => ownerOf(files, spans, slide));
  const assignment = assign(owner, opening(spans, before.slides), texts(before), texts(after));

  const writes: FileWrite[] = [];

  files.forEach((file, index) => {
    const mine = assignment.flatMap((owned, slide) => (owned === index ? [slide] : []));
    const source = mine.length === 0 ? null : rewrap(file.source, cut(after, mine));

    // Nothing left, and nothing there: a file this session already emptied.
    if (source === null && file.source.length === 0) return;
    if (source === file.source) return;

    writes.push({ path: file.path, label: file.label, source });
  });

  return writes;
}

/** The file a slide's bytes are in. */
function ownerOf(files: readonly DeckFile[], spans: ByteSpan[], slide: ByteSpan): number {
  const index = spans.findIndex(
    (span) => span.start <= slide.start && slide.end <= span.end && span.end > span.start,
  );

  if (index !== -1) return index;

  const crossed = spans.findIndex((span) => span.end > slide.start);
  throw new Error(
    `A slide runs past the end of ${files[Math.max(crossed, 0)]?.label ?? "a slide file"}. ` +
      "Each file has to hold whole slides — an unclosed code fence is the usual cause, " +
      "because it swallows the separator between one file and the next.",
  );
}

/**
 * For each file, the index of the first slide that is not behind it.
 *
 * This is what a slide with no predecessor to inherit from lands on: an
 * inserted slide joins the file it pushed down, and a slide restored by an undo
 * finds the file it was removed from still holding its place.
 */
function opening(spans: ByteSpan[], slides: readonly ByteSpan[]): number[] {
  return spans.map((span) => slides.filter((slide) => slide.end <= span.start).length);
}

/**
 * Which file each slide of the edited deck belongs in.
 *
 * Derived from what changed rather than from which operation was run, so a new
 * operation in Rust needs no case here. The unchanged slides at each end keep
 * their file; the run between them is matched up one for one for as far as it
 * goes, which is what makes a reorder move bytes between files instead of
 * piling them into one.
 */
function assign(
  owner: number[],
  opens: number[],
  before: readonly string[],
  after: readonly string[],
): number[] {
  const prefix = shared(before, after);
  const suffix = sharedFromEnd(before, after, prefix);
  const oldRunEnd = before.length - suffix;
  const newRunEnd = after.length - suffix;

  return after.map((_, index) => {
    if (index < prefix) return owner[index]!;
    if (index >= newRunEnd) return owner[index - after.length + before.length]!;
    if (oldRunEnd > prefix) return owner[Math.min(index, oldRunEnd - 1)]!;

    // Nothing was replaced, so these slides are new. They go where the slide
    // they displaced starts, or in the last file when there is nothing after
    // them to displace.
    const landing = opens.findIndex((from) => from >= prefix);
    return landing === -1 ? owner.length - 1 : landing;
  });
}

function shared(before: readonly string[], after: readonly string[]): number {
  let count = 0;
  while (count < before.length && count < after.length && before[count] === after[count])
    count += 1;

  return count;
}

function sharedFromEnd(
  before: readonly string[],
  after: readonly string[],
  prefix: number,
): number {
  const room = Math.min(before.length, after.length) - prefix;
  let count = 0;

  while (count < room && before[before.length - 1 - count] === after[after.length - 1 - count]) {
    count += 1;
  }

  return count;
}

function texts(located: LocatedSource): string[] {
  return located.slides.map((slide) => located.source.slice(slide.start, slide.end));
}

/**
 * The bytes of a run of slides, taken whole from the edited source.
 *
 * A run that opens the deck starts at byte zero rather than at its first slide,
 * because the deck's own frontmatter sits above every slide and belongs to no
 * one of them.
 */
function cut(after: LocatedSource, slides: number[]): string {
  const first = slides[0]!;
  const start = first === 0 ? 0 : after.slides[first]!.start;

  return after.source.slice(start, after.slides[slides[slides.length - 1]!]!.end);
}

/**
 * New content, in the whitespace the file already had around its own.
 *
 * A file that loses its final newline reads as a whole-file change in every
 * review tool, which is the opposite of what a splice is for. A file that had
 * nothing in it gets one, because every text file ends with a line.
 */
function rewrap(original: string, content: string): string {
  if (original.length === 0) return `${content}${content.includes("\r\n") ? "\r\n" : "\n"}`;

  const lead = original.length - original.trimStart().length;
  const trail = Math.min(original.length - original.trimEnd().length, original.length - lead);

  return `${original.slice(0, lead)}${content}${original.slice(original.length - trail)}`;
}
