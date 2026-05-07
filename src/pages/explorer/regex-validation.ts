/**
 * @file regex-validation.ts
 * @description Validate Explorer regex input against the Rust regex dialect used by the desktop backend.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Keep client-side regex validation from green-lighting patterns the backend
 *   will reject, such as look-around and backreferences.
 * - Preserve the existing lightweight validation contract for the Explorer
 *   search input while making the dialect boundary explicit.
 *
 * ## Not responsible for
 * - Implementing a full Rust regex parser in TypeScript.
 * - Executing regex searches or deciding fallback keyword behavior.
 *
 * ## Dependencies
 * - Uses the browser `RegExp` constructor for baseline syntax validation, then
 *   rejects Rust-unsupported constructs that JavaScript accepts.
 *
 * ## Performance notes
 * - The scan is linear in pattern length and runs only on the current search
 *   input value.
 */

const unsupportedRustRegexFragments = [
  '(?=',
  '(?!',
  '(?<=',
  '(?<!',
  '(?<',
  '\\k<',
]

/**
 * Returns whether a pattern is safe to send to Rust `regex`.
 *
 * JavaScript accepts look-around and backreferences that Rust rejects. This
 * helper keeps the UI conservative so invalid backend patterns are caught
 * before a query clears the visible result set.
 */
export function isRustRegexCompatible(pattern: string) {
  try {
    new RegExp(pattern)
  } catch {
    return false
  }

  if (
    unsupportedRustRegexFragments.some((fragment) => pattern.includes(fragment))
  ) {
    return false
  }

  return !containsNumericBackreference(pattern)
}

function containsNumericBackreference(pattern: string) {
  for (let index = 0; index < pattern.length - 1; index += 1) {
    if (pattern[index] !== '\\') continue
    let slashCount = 1
    let cursor = index - 1
    while (cursor >= 0 && pattern[cursor] === '\\') {
      slashCount += 1
      cursor -= 1
    }
    if (slashCount % 2 === 0) continue
    const next = pattern[index + 1]
    if (next && next >= '1' && next <= '9') return true
  }
  return false
}
