/**
 * The backend gives text positions as UTF-8 byte offsets (search snippets,
 * find occurrences); JavaScript strings index UTF-16 code units. These turn
 * one into the other.
 */

/** The UTF-8 length of one code point. */
function utf8Length(codePoint: number) {
  if (codePoint < 0x80) return 1
  if (codePoint < 0x800) return 2
  if (codePoint < 0x10000) return 3
  return 4
}

/**
 * The string index where UTF-8 byte offset `byte` falls in `text`. A byte
 * inside a character resolves to the end of that character; past the end,
 * to the end of the string.
 */
export function utf8ToIndex(text: string, byte: number) {
  if (byte <= 0) return 0
  let bytes = 0
  let index = 0
  while (index < text.length) {
    const codePoint = text.codePointAt(index)!
    bytes += utf8Length(codePoint)
    index += codePoint > 0xffff ? 2 : 1
    if (bytes >= byte) return index
  }
  return text.length
}

/** A half-open UTF-8 byte range of `text` as string indexes. */
export function utf8RangeToIndexes(
  text: string,
  [start, end]: readonly [number, number],
): [number, number] {
  const from = utf8ToIndex(text, start)
  return [from, Math.max(from, utf8ToIndex(text, end))]
}
