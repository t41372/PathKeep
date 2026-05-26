/**
 * Active-filter chip helpers for the Paper search hero.
 *
 * ## Responsibilities
 * - Tokenise the user's keyword query the same way `vault-core` does
 *   (whitespace + ASCII / smart double quotes, leading `-` for negation)
 *   so the chip surface stays faithful to what the backend parser will
 *   actually act on.
 * - Project tokens that look like advanced operators (`tag:`, `note:`,
 *   `site:`, `filetype:`, `intitle:`, `inurl:`, `after:`, `before:`,
 *   plus their aliases) into removable {@link ActiveSearchFilter} chips.
 * - Provide pure string helpers to **append** a new operator placeholder
 *   (`appendOperator(query, 'tag')` → `"prev tag:"`) and **remove** a
 *   specific operator token by its tokenised index without disturbing
 *   the user's surrounding text or other operator tokens.
 *
 * ## Not responsible for
 * - Resolving the values against the archive — that lives in
 *   `vault-core::archive::search_query`.
 * - Mode / regex / semantic switching — owned by PaperSearchHero.
 * - Cursor placement after an append — owned by the panel (caller
 *   knows where the input DOM node lives).
 */

const KNOWN_OPERATORS: ReadonlySet<string> = new Set([
  'tag',
  'note',
  'site',
  'filetype',
  'ext',
  'after',
  'before',
  'intitle',
  'title',
  'inurl',
  'url',
])

const OPERATOR_KIND: Record<string, ActiveFilterKind> = {
  tag: 'tag',
  note: 'note',
  site: 'site',
  filetype: 'filetype',
  ext: 'filetype',
  after: 'after',
  before: 'before',
  intitle: 'intitle',
  title: 'intitle',
  inurl: 'inurl',
  url: 'inurl',
}

export type ActiveFilterKind =
  | 'tag'
  | 'note'
  | 'site'
  | 'filetype'
  | 'after'
  | 'before'
  | 'intitle'
  | 'inurl'

export interface ActiveSearchFilter {
  /**
   * Stable id encoded from (kind, negated, value, occurrenceIndex).
   * Identical across re-parses of the same query AND across re-parses
   * of a query that has only had unrelated tokens added/removed — so
   * a chip click resolves the right operator even when query state
   * shifted between render and the click event. Use as a React key
   * AND as the lookup id for {@link removeFilterToken}.
   */
  id: string
  /** Canonical operator family (after aliasing `ext`→`filetype` etc.). */
  kind: ActiveFilterKind
  /** Operator value with surrounding quotes stripped. */
  value: string
  /** True when the user prefixed the token with `-` (negation). */
  negated: boolean
  /** Display label, e.g. `tag:rust` or `-tag:archived`. */
  label: string
  /**
   * The Nth occurrence (0-based) of this (kind, negated, value)
   * triple within the parsed filter list. Combined with the other
   * fields in {@link id}, it disambiguates duplicates like
   * `tag:rust tag:rust` so removing one doesn't ambiguously match
   * both.
   */
  occurrenceIndex: number
  /**
   * Index into {@link tokenizeQuery}'s output. {@link removeFilterToken}
   * uses this to strip the exact token even when the user typed two
   * identical operators (`tag:rust tag:rust`).
   */
  tokenIndex: number
}

interface QueryToken {
  literal: string
  startIndex: number
  endIndex: number
}

/**
 * Splits the raw query into the same token stream the backend parser
 * sees: whitespace separated, double-quoted phrases stay together
 * (supports both ASCII `"` and smart `“`/`”`), backslash escapes one
 * character inside quotes.
 */
export function tokenizeQuery(raw: string): QueryToken[] {
  const tokens: QueryToken[] = []
  let i = 0
  while (i < raw.length) {
    while (i < raw.length && isWhitespace(raw[i])) i++
    if (i >= raw.length) break
    const start = i
    let inQuote = false
    let openQuoteChar = ''
    while (i < raw.length) {
      const ch = raw[i]
      if (inQuote) {
        if (ch === '\\' && i + 1 < raw.length) {
          i += 2
          continue
        }
        if (matchesClosingQuote(openQuoteChar, ch)) {
          inQuote = false
          i++
          continue
        }
        i++
      } else if (ch === '"' || ch === '“' || ch === '”') {
        inQuote = true
        openQuoteChar = ch
        i++
      } else if (isWhitespace(ch)) {
        break
      } else {
        i++
      }
    }
    tokens.push({
      literal: raw.slice(start, i),
      startIndex: start,
      endIndex: i,
    })
  }
  return tokens
}

function isWhitespace(ch: string): boolean {
  return ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r' || ch === '\f'
}

function matchesClosingQuote(openChar: string, candidate: string): boolean {
  if (openChar === '"') return candidate === '"'
  // Both smart quote variants close either smart quote, matching the
  // tolerant behaviour of `vault-core::archive::search_query`.
  return candidate === '”' || candidate === '“'
}

function stripSurroundingQuotes(value: string): string {
  if (value.length < 2) return value
  const first = value[0]
  const last = value[value.length - 1]
  const asciiPaired = first === '"' && last === '"'
  const smartPaired =
    (first === '“' || first === '”') && (last === '“' || last === '”')
  if (asciiPaired || smartPaired) return value.slice(1, -1)
  return value
}

/**
 * Extracts removable filter chips from a raw query string. Tokens
 * whose operator is unknown, whose operand is empty, or whose colon
 * is preceded by non-alphabetic characters (e.g. `123:abc`) are
 * skipped so the chip row only reflects real active filters.
 */
export function parseActiveSearchFilters(query: string): ActiveSearchFilter[] {
  const tokens = tokenizeQuery(query)
  const filters: ActiveSearchFilter[] = []
  const occurrenceCount = new Map<string, number>()
  tokens.forEach((token, tokenIndex) => {
    const negated = token.literal.startsWith('-') && token.literal.length > 1
    const body = negated ? token.literal.slice(1) : token.literal
    const colonIndex = body.indexOf(':')
    if (colonIndex <= 0) return
    const operatorRaw = body.slice(0, colonIndex).toLowerCase()
    if (!/^[a-z]+$/.test(operatorRaw)) return
    if (!KNOWN_OPERATORS.has(operatorRaw)) return
    const operand = body.slice(colonIndex + 1).trim()
    if (operand.length === 0) return
    const display = stripSurroundingQuotes(operand).trim()
    if (display.length === 0) return
    const kind = OPERATOR_KIND[operatorRaw]
    const identityKey = `${kind}::${negated ? 'neg' : 'pos'}::${display}`
    const occurrenceIndex = occurrenceCount.get(identityKey) ?? 0
    occurrenceCount.set(identityKey, occurrenceIndex + 1)
    filters.push({
      // Identity-based id: stable across re-parses of the same query
      // even after unrelated tokens were added/removed, so a chip
      // click resolves the right operator when state shifted between
      // render and click. occurrenceIndex disambiguates duplicates.
      id: `${identityKey}::${occurrenceIndex}`,
      kind,
      value: display,
      negated,
      label: `${negated ? '-' : ''}${kind}:${display}`,
      occurrenceIndex,
      tokenIndex,
    })
  })
  return filters
}

/**
 * Appends `operator:` to the query (separated by a single space when
 * the query already has content) and returns the next query. The
 * caller is expected to focus the input and place the caret at the
 * end so the user can start typing the value immediately.
 */
export function appendOperator(query: string, operator: string): string {
  const op = operator.trim().toLowerCase()
  if (op.length === 0 || !/^[a-z]+$/.test(op)) return query
  const trimmedEnd = query.replace(/\s+$/, '')
  if (trimmedEnd.length === 0) return `${op}:`
  return `${trimmedEnd} ${op}:`
}

/**
 * Removes the token at the supplied {@link ActiveSearchFilter.tokenIndex}
 * from the query and collapses the whitespace **at the junction**
 * between the kept before / after slices so the resulting string
 * stays clean. Critically, whitespace inside surviving tokens (e.g.
 * a quoted phrase like `"release\tnotes"`) is preserved — running a
 * naïve `replace(/[ \t]+/g, ' ')` over the full stitched string
 * silently rewrites those phrases and they would no longer match the
 * vault-core search-query parser's literal tokens. Returns the
 * query unchanged when the index is out of range (which can happen
 * if the user edited the query between parse and click).
 */
export function removeFilterToken(query: string, tokenIndex: number): string {
  const tokens = tokenizeQuery(query)
  const token = tokens[tokenIndex]
  if (!token) return query
  // Walk the kept slice on either side of the removed token, dropping
  // ONLY the whitespace immediately abutting the cut. Anything past
  // the first non-whitespace character is preserved verbatim — that
  // includes whitespace inside quoted phrases further down the line.
  const before = query.slice(0, token.startIndex).replace(/[ \t]+$/, '')
  const after = query.slice(token.endIndex).replace(/^[ \t]+/, '')
  if (before.length === 0) return after
  if (after.length === 0) return before
  return `${before} ${after}`
}
