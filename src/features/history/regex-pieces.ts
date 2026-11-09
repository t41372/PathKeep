/**
 * The Advanced search form's "Pattern" tab: a pattern described as a row of
 * pieces (some text, a number, anything up to the next slash…) and written
 * as a regex in the dialect the backend runs (Rust's `regex`, matched
 * case-insensitively against each page's address and title, see
 * `vault-core/src/search/regex.rs`).
 *
 * Responsible for: what each piece means as regex, escaping typed text, the
 * start / end / letter-case options, and the ready-made examples.
 * Not responsible for: checking the result (`regex-dialect.ts` does, the same
 * check a typed regex gets) or reading a typed regex back into pieces.
 *
 * Ways this can go wrong, and what guards each:
 * 1. Typed text with regex syntax in it (`?id=`, `c++`, `(beta)`) changing
 *    the pattern: every character Rust treats as syntax is escaped, the same
 *    set as `regex::escape`.
 * 2. "One of" alternatives leaking into the pieces around them (`a|b` next
 *    to text matches "a" or "b + text"): alternatives sit in a
 *    non-capturing group.
 * 3. Empty pieces writing empty groups or stray `|`: text and "one of"
 *    pieces with nothing typed write nothing.
 * 4. "Match case" doing nothing because the backend folds case: it adds the
 *    inline flag `(?-i)`, which turns the backend's case folding off.
 * 5. "Starts with" read as "the site starts with": addresses start with
 *    `https://`, so the option's label says it means the whole address or
 *    title, and the examples never use it.
 */

export type PieceKind = 'text' | 'oneOf' | 'number' | 'word' | 'segment' | 'any'

export interface Piece {
  kind: PieceKind
  /** Typed text for `text`; comma-separated alternatives for `oneOf`. */
  value: string
}

export interface PatternForm {
  pieces: Piece[]
  /** The match must begin where the address or title begins. */
  atStart: boolean
  /** The match must end where the address or title ends. */
  atEnd: boolean
  /** Upper and lower case must match as typed. */
  matchCase: boolean
}

/** A piece on screen: the id keeps each row's input its own while rows come and go. */
export type EditablePiece = Piece & { id: number }
export type EditablePattern = Omit<PatternForm, 'pieces'> & {
  pieces: EditablePiece[]
}

let nextId = 1
export function editable(pieces: readonly Piece[]): EditablePiece[] {
  return pieces.map((piece) => ({ ...piece, id: nextId++ }))
}

export const emptyPatternForm: PatternForm = {
  pieces: [{ kind: 'text', value: '' }],
  atStart: false,
  atEnd: false,
  matchCase: false,
}

/** Pieces that take typed text. */
export function takesText(kind: PieceKind) {
  return kind === 'text' || kind === 'oneOf'
}

/** `regex::escape`'s set: everything Rust's parser treats as syntax. */
export function escapeRegex(text: string) {
  return text.replace(/[\\.+*?()|[\]{}^$#&\-~]/g, (char) => `\\${char}`)
}

function alternatives(value: string) {
  return value
    .split(',')
    .map((alternative) => alternative.trim())
    .filter(Boolean)
}

function pieceRegex(piece: Piece) {
  switch (piece.kind) {
    case 'text':
      return escapeRegex(piece.value)
    case 'oneOf': {
      const options = alternatives(piece.value).map(escapeRegex)
      return options.length > 1
        ? `(?:${options.join('|')})`
        : (options[0] ?? '')
    }
    case 'number':
      return '\\d+'
    case 'word':
      return '\\w+'
    case 'segment':
      return '[^/]+'
    case 'any':
      return '.*'
  }
}

/** The regex the pieces describe, or an empty string when they describe nothing yet. */
export function composePattern(form: PatternForm) {
  const body = form.pieces.map(pieceRegex).join('')
  if (!body) return ''
  return [
    form.matchCase ? '(?-i)' : '',
    form.atStart ? '^' : '',
    body,
    form.atEnd ? '$' : '',
  ].join('')
}

export type PatternExample =
  | 'githubThreads'
  | 'hackerNews'
  | 'youtube'
  | 'stackOverflow'

/** Ready-made patterns, each one a place to start editing from. */
export const patternExamples: Record<PatternExample, Piece[]> = {
  githubThreads: [
    { kind: 'text', value: 'github.com/' },
    { kind: 'segment', value: '' },
    { kind: 'text', value: '/' },
    { kind: 'segment', value: '' },
    { kind: 'text', value: '/' },
    { kind: 'oneOf', value: 'pull, issues' },
    { kind: 'text', value: '/' },
    { kind: 'number', value: '' },
  ],
  hackerNews: [
    { kind: 'text', value: 'news.ycombinator.com/item?id=' },
    { kind: 'number', value: '' },
  ],
  youtube: [{ kind: 'oneOf', value: 'youtube.com/watch?v=, youtu.be/' }],
  stackOverflow: [
    { kind: 'text', value: 'stackoverflow.com/questions/' },
    { kind: 'number', value: '' },
  ],
}
