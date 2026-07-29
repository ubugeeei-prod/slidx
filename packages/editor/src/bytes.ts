/**
 * Byte offsets, in a language that counts in UTF-16 code units.
 *
 * Every span slidx uses is measured in bytes, because a splice indexes straight
 * into the file the author saved. A browser's `String.prototype.slice` counts
 * differently, and on a deck written in Japanese the two answers diverge on the
 * first character — silently, and in a way that shows up as an edit landing in
 * the middle of a word. So nothing here indexes a source with a plain slice.
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** How many bytes `text` occupies in a file. */
export function byteLength(text: string): number {
  return encoder.encode(text).length;
}

/** The text a byte range names, or `""` when it names none. */
export function sliceBytes(source: string, start: number, end: number): string {
  if (!(end > start)) return "";

  return decoder.decode(encoder.encode(source).subarray(start, end));
}
