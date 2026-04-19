/**
 * Keeps Explorer display text from leaking callback parameters or email-like strings into titles.
 */

import { describe, expect, test } from 'vitest'
import { sanitizeExplorerDisplayText } from './helpers'

describe('sanitizeExplorerDisplayText', () => {
  test('redacts token-like query params by collapsing URLs to safe host/path output', () => {
    expect(
      sanitizeExplorerDisplayText(
        'https://example.com/callback?code=secret&token=secret&email=test@example.com',
      ),
    ).toBe('example.com/callback')
  })

  test('redacts plain email-like strings outside URLs', () => {
    expect(
      sanitizeExplorerDisplayText('Follow up with test@example.com about auth'),
    ).toBe('Follow up with … about auth')
  })
})
