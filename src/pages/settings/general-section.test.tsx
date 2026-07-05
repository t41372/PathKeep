/**
 * @file general-section.test.tsx
 * @description Presentational coverage for general Settings controls.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify language changes are forwarded to the route state owner.
 * - Keep low-stakes Settings controls covered outside the full Settings route.
 *
 * ## Not responsible for
 * - Re-testing persistence or shell refresh behavior.
 * - Re-testing every Settings section.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and preview snapshot fixture.
 *
 * ## Performance notes
 * - Pure render and select coverage only.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import type { LanguagePreference } from '../../lib/types'
import { GeneralSection } from './general-section'

describe('GeneralSection', () => {
  test('forwards interface-language changes', async () => {
    const user = userEvent.setup()
    const onLanguageChange = vi.fn().mockResolvedValue(undefined)

    render(
      <I18nProvider>
        <GeneralSection
          explorerBackgroundPrefetchPages={1}
          navItem={{
            id: 'settings-general',
            icon: 'settings',
            key: 'general',
            label: 'General',
          }}
          onExplorerBackgroundPrefetchPagesChange={vi.fn()}
          onLanguageChange={onLanguageChange}
          saving={false}
          snapshot={mockSnapshot}
        />
      </I18nProvider>,
    )

    await user.selectOptions(screen.getByLabelText('Language'), 'zh-CN')

    expect(onLanguageChange).toHaveBeenCalledWith('zh-CN')
  })

  function renderWithPreferredLanguage(preferredLanguage: LanguagePreference) {
    render(
      <I18nProvider>
        <GeneralSection
          explorerBackgroundPrefetchPages={1}
          navItem={{
            id: 'settings-general',
            icon: 'settings',
            key: 'general',
            label: 'General',
          }}
          onExplorerBackgroundPrefetchPagesChange={vi.fn()}
          onLanguageChange={vi.fn()}
          saving={false}
          snapshot={{
            ...mockSnapshot,
            config: { ...mockSnapshot.config, preferredLanguage },
          }}
        />
      </I18nProvider>,
    )
  }

  test('shows the read-only "current language" row only under the system preference', () => {
    // Under "Follow system" the select reads "Follow system", so this row is
    // the only place the resolved language is named — it must render.
    renderWithPreferredLanguage('system')
    expect(screen.getByText('Current')).toBeVisible()
  })

  test('hides the "current language" row when an explicit language is chosen', () => {
    // The select already shows the active value for every explicit choice, so
    // repeating it in a read-only row would be redundant noise.
    renderWithPreferredLanguage('en')
    expect(screen.queryByText('Current')).toBeNull()
  })
})
