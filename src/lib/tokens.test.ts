import { describe, expect, test } from 'vitest'
import { themes, tokens } from './tokens'

describe('design tokens', () => {
  test('exports both supported themes', () => {
    expect(themes).toEqual(['dark', 'light'])
  })

  test('maps token helpers to CSS variables', () => {
    expect(tokens.color.accent).toBe('var(--accent)')
    expect(tokens.font.mono).toBe('var(--font-mono)')
    expect(tokens.space[4]).toBe('var(--space-4)')
    expect(tokens.density.sidebar).toBe('var(--layout-sidebar-width)')
  })
})
