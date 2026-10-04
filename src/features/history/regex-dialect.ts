/**
 * Checks a regex against the dialect the backend runs: Rust's `regex` crate
 * (`RegexBuilder::case_insensitive(true)`), not JavaScript's. Patterns are
 * checked as the user types, so one the backend would reject never reaches it.
 *
 * Responsible for: the structure Rust's parser checks (groups, classes,
 * escapes, repeat counts) and naming what is wrong in terms a user can act on:
 * look-around, back-references, group kinds Rust lacks, escapes it does not
 * know, braces that are not a repeat count, plain syntax errors.
 * Not responsible for: semantic errors only the compiler finds (an unknown
 * Unicode property, a reversed range, the size limit). The page treats a
 * backend regex error as an invalid pattern, so those still read as such.
 *
 * JavaScript's own `RegExp` is deliberately not used: it rejects patterns Rust
 * accepts (`a++`, `(?i)`, `(?P<name>…)`) and accepts ones Rust rejects (`\1`,
 * `\cA`, `a{,5}`). The cases below were checked against regex 1.12.
 *
 * Ways this can go wrong, and what guards each:
 * 1. A named group `(?<name>…)` mistaken for look-behind: only `(?<=` and
 *    `(?<!` count as look-behind.
 * 2. Syntax inside a character class read as groups: classes (nested, and
 *    `[:alpha:]`, as Rust allows) are skipped as a whole.
 * 3. An escaped character read as syntax (`\(?=`): a backslash consumes what
 *    follows it before anything else looks at it.
 */

export type RegexProblem =
  | { kind: 'syntax' }
  | { kind: 'lookAround' }
  | { kind: 'backreference' }
  | { kind: 'group' }
  | { kind: 'escape'; escape: string }
  | { kind: 'brace' }

/** Letter and digit escapes Rust understands; other punctuation escapes are all fine. */
const letterEscapes = new Set('aftnrvxuUdDsSwWpPbBAz')
const hexDigits = /^[0-9a-fA-F]+$/
/** `{n}`, `{n,}`, `{n,m}`, spaces allowed inside. */
const repeatCount = /^\{\s*\d+\s*(,\s*\d*\s*)?\}/
const namedGroup = /^P?<[A-Za-z_][A-Za-z0-9_.[\]]*>/
/** `(?i)` sets flags (at least one); `(?:` and `(?i:` open a group. */
const flagGroup = /^([imsUuxR-]+\)|[imsUuxR-]*:)/
const wordBoundary = /^\{(start|end|start-half|end-half)\}/

/** Thrown inside the scan to stop at the first problem; never escapes `checkRustRegex`. */
class Problem extends Error {
  readonly problem: RegexProblem
  constructor(problem: RegexProblem) {
    super(problem.kind)
    this.problem = problem
  }
}

/** The first reason the backend would refuse `pattern`, or null when it would run. */
export function checkRustRegex(pattern: string): RegexProblem | null {
  try {
    scanPattern(pattern)
    return null
  } catch (caught) {
    if (caught instanceof Problem) return caught.problem
    throw caught
  }
}

function fail(problem: RegexProblem): never {
  throw new Problem(problem)
}

/** Reads one escape starting at the backslash; returns the index of its last character. */
function readEscape(pattern: string, at: number, inClass = false): number {
  const next = pattern[at + 1]
  if (next === undefined) fail({ kind: 'syntax' })
  // Assertions match positions, not characters, so a class cannot hold them.
  if (inClass && /[bBAz<>]/.test(next))
    fail({ kind: 'escape', escape: `\\${next}` })
  if (/[0-9]/.test(next)) fail({ kind: 'backreference' })
  if (next === 'k' && pattern[at + 2] === '<') fail({ kind: 'backreference' })
  if (!/[A-Za-z]/.test(next)) return at + 1
  if (!letterEscapes.has(next)) fail({ kind: 'escape', escape: `\\${next}` })

  const after = at + 2
  const braced = (hex: boolean) => {
    const close = pattern.indexOf('}', after)
    if (close < 0) fail({ kind: 'syntax' })
    const body = pattern.slice(after + 1, close)
    if (hex ? !hexDigits.test(body) : !body.trim()) fail({ kind: 'syntax' })
    return close
  }
  const digits = (count: number) => {
    const run = pattern.slice(after, after + count)
    if (run.length < count || !hexDigits.test(run)) fail({ kind: 'syntax' })
    return after + count - 1
  }
  switch (next) {
    case 'x':
      return pattern[after] === '{' ? braced(true) : digits(2)
    case 'u':
      return pattern[after] === '{' ? braced(true) : digits(4)
    case 'U':
      return pattern[after] === '{' ? braced(true) : digits(8)
    case 'p':
    case 'P':
      if (pattern[after] === '{') return braced(false)
      // One-letter form: only the general categories exist.
      if (!/[CLMNPSZ]/i.test(pattern[after] ?? '')) fail({ kind: 'syntax' })
      return after
    case 'b': {
      // `\b{start}` and friends; any other brace is a repeat count of `\b`.
      const named = wordBoundary.exec(pattern.slice(after))
      return named ? after + named[0].length - 1 : at + 1
    }
    default:
      return at + 1
  }
}

/** Reads a character class starting at `[`; returns the index of its closing `]`. */
function readClass(pattern: string, open: number): number {
  let at = open + 1
  if (pattern[at] === '^') at += 1
  // A `]` first in the class is a literal in Rust, not the end of an empty class.
  if (pattern[at] === ']') at += 1
  for (; at < pattern.length; at += 1) {
    const char = pattern[at]
    if (char === '\\') at = readEscape(pattern, at, true)
    else if (char === ']') return at
    else if (char === '[') {
      const ascii = /^\[:\^?[a-z]+:\]/.exec(pattern.slice(at))
      at = ascii ? at + ascii[0].length - 1 : readClass(pattern, at)
    }
  }
  return fail({ kind: 'syntax' })
}

function scanPattern(pattern: string) {
  let depth = 0
  // True where a repeat would have nothing to repeat: the start, after `(` or `|`.
  let atStart = true
  for (let at = 0; at < pattern.length; at += 1) {
    const char = pattern[at]
    if (char === '\\') {
      at = readEscape(pattern, at)
      atStart = false
    } else if (char === '[') {
      at = readClass(pattern, at)
      atStart = false
    } else if (char === '(') {
      if (pattern[at + 1] === '?') {
        const rest = pattern.slice(at + 2)
        if (rest === '') fail({ kind: 'syntax' })
        if (/^(=|!|<=|<!)/.test(rest)) fail({ kind: 'lookAround' })
        const named = namedGroup.exec(rest)
        const flags = named ? null : flagGroup.exec(rest)
        if (!named && !flags) fail({ kind: 'group' })
        const opening = (named ?? flags)![0]
        at += 1 + opening.length
        // `(?i)` only sets flags; `(?i:…)` and named groups open a group.
        if (!opening.endsWith(')')) {
          depth += 1
          atStart = true
        }
      } else {
        depth += 1
        atStart = true
      }
    } else if (char === ')') {
      if (depth === 0) fail({ kind: 'syntax' })
      depth -= 1
      atStart = false
    } else if (char === '|') {
      atStart = true
    } else if (char === '*' || char === '+' || char === '?') {
      if (atStart) fail({ kind: 'syntax' })
    } else if (char === '{') {
      const count = repeatCount.exec(pattern.slice(at))
      if (!count) fail({ kind: 'brace' })
      if (atStart) fail({ kind: 'syntax' })
      at += count[0].length - 1
    } else {
      atStart = false
    }
  }
  if (depth > 0) fail({ kind: 'syntax' })
}
