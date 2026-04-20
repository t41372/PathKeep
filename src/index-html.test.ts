/**
 * This test keeps the desktop HTML shell free of remote runtime asset fetches.
 *
 * Why this file exists:
 * - PathKeep's desktop shell is local-first, so startup must not silently depend
 *   on third-party icon or font CDNs.
 * - Checking `index.html` directly catches regressions before they ship into the
 *   bundled desktop WebView.
 *
 * Main declarations:
 * - none
 */

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, test } from 'vitest'

describe('desktop html shell', () => {
  test('does not depend on remote icon or font stylesheets', () => {
    const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8')

    expect(html).not.toContain('fonts.googleapis.com')
    expect(html).not.toContain('fonts.gstatic.com')
  })
})
