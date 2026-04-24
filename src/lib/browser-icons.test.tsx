/**
 * This test file protects the front-end helper and contract logic in Browser Icons.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import {
  BrowserIcon,
  browserIconKeyForName,
  supportedBrowsers,
} from './browser-icons'

describe('browser icons', () => {
  test('maps every supported browser to a packaged icon and falls back safely', () => {
    expect(supportedBrowsers).toHaveLength(15)
    expect(new Set(supportedBrowsers.map((browser) => browser.name)).size).toBe(
      supportedBrowsers.length,
    )
    expect(browserIconKeyForName('Google Chrome')).toBe('chrome')
    expect(browserIconKeyForName('ChatGPT Atlas')).toBe('atlas')
    expect(browserIconKeyForName('Microsoft Edge Dev')).toBe('edge-dev')
    expect(browserIconKeyForName('Safari')).toBe('safari')
    expect(browserIconKeyForName('Unknown Browser')).toBe('generic')

    const { container } = render(
      <>
        {supportedBrowsers.map((browser) => (
          <BrowserIcon
            key={browser.name}
            browserName={browser.name}
            title={`${browser.name} icon`}
          />
        ))}
        <BrowserIcon
          browserName="Unknown Browser"
          title="Unknown Browser icon"
        />
      </>,
    )

    expect(container.querySelectorAll('svg')).toHaveLength(
      supportedBrowsers.length + 1,
    )
    expect(
      screen.getByRole('img', { name: 'Google Chrome icon' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('img', { name: 'ChatGPT Atlas icon' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('img', { name: 'Unknown Browser icon' }),
    ).toBeInTheDocument()
  })

  test('supports decorative icons and default accessible titles', () => {
    const { container } = render(
      <>
        <BrowserIcon browserName="Firefox" decorative />
        <BrowserIcon browserName="Arc" />
      </>,
    )

    expect(
      screen.queryByRole('img', { name: 'Firefox icon' }),
    ).not.toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'Arc icon' })).toBeInTheDocument()
    expect(container.querySelectorAll('svg')).toHaveLength(2)
  })
})
