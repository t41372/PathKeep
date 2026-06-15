/**
 * WCAG contrast guard for the paper/archival ink ramp.
 *
 * Why this file exists:
 * - tokens.css is the visual source of truth for color, but readability is a
 *   hard product contract, not a matter of taste: every ink token that can
 *   carry copy must clear WCAG AA (4.5:1) on the surfaces it sits on. An earlier
 *   ramp shipped light --ink-faint at 1.87:1 and dark --ink-muted at 2.93:1,
 *   which failed AA for the 9-13px metadata they style.
 * - This test parses the real tokens.css so the contract is enforced against
 *   shipping values; any future retune that regresses contrast fails here
 *   instead of silently shipping illegible metadata.
 *
 * Not responsible for:
 * - Tailwind/shadcn variable bridging (tailwind.css) or per-component class
 *   wiring; only the raw token values in tokens.css.
 *
 * Source-of-truth notes:
 * - Mirrors the readability rule documented in docs/design/design-tokens.md.
 *   --ink-ghost is intentionally excluded: it is decorative-only and must never
 *   hold readable text, so it carries no contrast obligation.
 */

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

// Read the real stylesheet bytes so the contract is checked against the file the
// shell ships, not a transform of it. Vitest runs from the repo root, so the
// path resolves deterministically. (Vite's ?raw import is not usable here: the
// CSS pipeline rewrites .css content even under ?raw, dropping the selectors.)
const tokensCss = readFileSync(
  path.resolve(process.cwd(), 'src/styles/tokens.css'),
  'utf8',
)

/** WCAG 2.x relative luminance for an 8-bit sRGB channel. */
function channelLuminance(value: number): number {
  const c = value / 255
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}

function relativeLuminance(hex: string): number {
  const h = hex.replace('#', '')
  const r = Number.parseInt(h.slice(0, 2), 16)
  const g = Number.parseInt(h.slice(2, 4), 16)
  const b = Number.parseInt(h.slice(4, 6), 16)
  return (
    0.2126 * channelLuminance(r) +
    0.7152 * channelLuminance(g) +
    0.0722 * channelLuminance(b)
  )
}

/** WCAG contrast ratio between two hex colors (order-independent). */
function contrastRatio(a: string, b: string): number {
  const la = relativeLuminance(a)
  const lb = relativeLuminance(b)
  const hi = Math.max(la, lb)
  const lo = Math.min(la, lb)
  return (hi + 0.05) / (lo + 0.05)
}

/** Extract the declaration body of the rule that owns `selector`. */
function ruleBody(selector: string): string {
  const start = tokensCss.indexOf(selector)
  if (start === -1) throw new Error(`selector ${selector} not found`)
  const open = tokensCss.indexOf('{', start)
  const close = tokensCss.indexOf('}', open)
  return tokensCss.slice(open + 1, close)
}

/**
 * Pull the hex value of a CSS custom property from a single rule body. Each
 * theme defines the same names once, so scoping to the matched block avoids
 * cross-theme bleed.
 */
function readToken(body: string, name: string): string {
  const match = body.match(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`))
  if (!match) throw new Error(`token --${name} not found in rule body`)
  return match[1].toLowerCase()
}

const AA_NORMAL = 4.5
const AA_LARGE = 3

// The light tokens live on the combined `:root, :root[data-theme='light']`
// rule (light is the default theme); dark on its own attribute rule.
const themes = [
  { name: 'light', body: ruleBody(":root[data-theme='light']") },
  { name: 'dark', body: ruleBody(":root[data-theme='dark']") },
] as const

// ink-ghost is decorative-only and deliberately excluded — see file header.
const textTokens = ['ink', 'ink-secondary', 'ink-muted', 'ink-faint'] as const

describe('tokens.css ink ramp meets WCAG AA', () => {
  for (const theme of themes) {
    describe(`${theme.name} mode`, () => {
      const paper = readToken(theme.body, 'bg-paper')
      const card = readToken(theme.body, 'bg-card')
      const page = readToken(theme.body, 'bg-page')

      for (const token of textTokens) {
        test(`--${token} clears AA (4.5:1) on paper and card`, () => {
          const value = readToken(theme.body, token)
          expect(contrastRatio(value, paper)).toBeGreaterThanOrEqual(AA_NORMAL)
          expect(contrastRatio(value, card)).toBeGreaterThanOrEqual(AA_NORMAL)
        })
      }

      test('--ink-faint clears large-text AA (3:1) on the page surface', () => {
        const faint = readToken(theme.body, 'ink-faint')
        expect(contrastRatio(faint, page)).toBeGreaterThanOrEqual(AA_LARGE)
      })

      test('muted reads more prominent than faint against paper', () => {
        const paperLum = relativeLuminance(paper)
        const mutedDelta = Math.abs(
          relativeLuminance(readToken(theme.body, 'ink-muted')) - paperLum,
        )
        const faintDelta = Math.abs(
          relativeLuminance(readToken(theme.body, 'ink-faint')) - paperLum,
        )
        // Light mode: darker text is more prominent. Dark mode: lighter text is.
        // Either way muted must out-contrast faint against the paper surface.
        expect(mutedDelta).toBeGreaterThan(faintDelta)
      })
    })
  }

  test('ink-ghost is defined for both themes but excluded from text contrast', () => {
    // It must still exist as a decorative token; we only assert presence so the
    // exclusion above is intentional, not an accidental omission.
    expect(readToken(themes[0].body, 'ink-ghost')).toMatch(/^#[0-9a-f]{6}$/)
    expect(readToken(themes[1].body, 'ink-ghost')).toMatch(/^#[0-9a-f]{6}$/)
  })
})
