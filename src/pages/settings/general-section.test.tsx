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
})
