/**
 * @file external-outputs-panel.test.tsx
 * @description Focused Settings external-output panel fallback coverage.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify the manual output panel shows the generic unavailable body when payload data is missing without an error.
 * - Keep the ready-state fallback covered without booting the full Integrations route harness.
 *
 * ## Not responsible for
 * - Re-testing embed, widget, public, or local-host tab internals.
 * - Re-testing Core Intelligence payload providers.
 *
 * ## Dependencies
 * - Mocks Core Intelligence hooks so the component stays render-only.
 *
 * ## Performance notes
 * - Uses one tiny render path and no IPC.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../lib/core-intelligence'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import { SettingsExternalOutputsPanel } from './external-outputs-panel'

const { useAsyncDataMock } = vi.hoisted(() => ({
  useAsyncDataMock: vi.fn(),
}))

vi.mock('../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()
  return {
    ...actual,
    getIntelligenceEmbedCards: vi.fn(),
    getIntelligencePublicSnapshot: vi.fn(),
    getIntelligenceWidgetSnapshot: vi.fn(),
    useAsyncData: useAsyncDataMock,
    useTimeRange: () => ({
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
      preset: 'month',
      setCustomRange: vi.fn(),
      setPreset: vi.fn(),
    }),
  }
})

describe('SettingsExternalOutputsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders the generic unavailable body when data is missing without an error', () => {
    useAsyncDataMock.mockReturnValue({
      data: null,
      error: null,
      loading: false,
      refresh: vi.fn(),
    })

    render(
      <I18nProvider>
        <MemoryRouter>
          <ProfileScopeContext.Provider
            value={{ activeProfileId: null, setActiveProfileId: vi.fn() }}
          >
            <SettingsExternalOutputsPanel initialized unlocked />
          </ProfileScopeContext.Provider>
        </MemoryRouter>
      </I18nProvider>,
    )

    expect(screen.getByText('Manual outputs are unavailable')).toBeVisible()
    expect(
      screen.getByText(
        'PathKeep could not load the current manual output preview. Try refreshing after the shell finishes reloading.',
      ),
    ).toBeVisible()
  })
})
