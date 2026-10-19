/**
 * Tag rules shared by the tag editor and the tag filter. They mirror
 * `vault_core::annotations::normalize_tags` so a tag the backend would drop or
 * reject never appears in the UI first.
 */

/** Per page, as in `MAX_TAGS_PER_URL`. */
export const MAX_TAGS = 64
/** Per tag, in UTF-8 bytes after trimming, as in `MAX_TAG_BYTES`. */
export const MAX_TAG_BYTES = 64

const encoder = new TextEncoder()

export type TagProblem = 'tooLong' | 'tooMany'

/**
 * Adds typed tags (comma-separated) to a list: trimmed, empty ones skipped,
 * case-insensitive duplicates ignored. Returns the new list, or the first
 * reason the backend would refuse it.
 */
export function addTags(
  current: readonly string[],
  input: string,
): { tags: string[] } | { problem: TagProblem } {
  const tags = [...current]
  const seen = new Set(current.map((tag) => tag.toLowerCase()))
  for (const raw of input.split(',')) {
    const tag = raw.trim()
    if (!tag) continue
    if (encoder.encode(tag).length > MAX_TAG_BYTES)
      return { problem: 'tooLong' }
    if (seen.has(tag.toLowerCase())) continue
    if (tags.length >= MAX_TAGS) return { problem: 'tooMany' }
    seen.add(tag.toLowerCase())
    tags.push(tag)
  }
  return { tags }
}

/**
 * The search text that filters History to one tag. Tags with spaces or quotes
 * are quoted, which the query parser accepts right after the colon.
 */
export function tagQuery(tag: string) {
  return /[\s"\\]/.test(tag)
    ? `tag:"${tag.replace(/["\\]/g, (char) => `\\${char}`)}"`
    : `tag:${tag}`
}
