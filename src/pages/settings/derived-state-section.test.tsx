/**
 * @file derived-state-section.test.tsx
 * @description Render-level coverage for the Settings derived-state section shell.
 * @module pages/settings
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import {
  DerivedStateSection,
  type DerivedStateSectionState,
} from './derived-state-section'

describe('DerivedStateSection', () => {
  test('renders nothing until the shell snapshot is available', () => {
    const { container } = render(
      <I18nProvider>
        <DerivedStateSection
          navItem={{
            id: 'settings-derived',
            icon: 'memory',
            key: 'derived',
            label: 'Derived state',
          }}
          snapshot={null}
          state={stateFixture()}
        />
      </I18nProvider>,
    )

    expect(container).toBeEmptyDOMElement()
  })

  test('rebuild + clear buttons fire their state handlers', async () => {
    const user = userEvent.setup()
    const onRebuildDerivedState = vi.fn().mockResolvedValue(undefined)
    const onClearDerivedState = vi.fn().mockResolvedValue(undefined)
    render(
      <MemoryRouter>
        <I18nProvider>
          <DerivedStateSection
            navItem={{
              id: 'settings-derived',
              icon: 'memory',
              key: 'derived',
              label: 'Derived state',
            }}
            snapshot={structuredClone(mockSnapshot)}
            state={{
              ...stateFixture(),
              onRebuildDerivedState,
              onClearDerivedState,
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    const rebuildButton = screen.getByRole('button', { name: 'Rebuild' })
    await user.click(rebuildButton)
    expect(onRebuildDerivedState).toHaveBeenCalledTimes(1)

    const clearButton = screen.getByRole('button', { name: 'Clear all' })
    await user.click(clearButton)
    expect(onClearDerivedState).toHaveBeenCalledTimes(1)
  })
})

function stateFixture(): DerivedStateSectionState {
  return {
    action: null,
    clearReport: null,
    dashboardRecentRun: null,
    intelligenceRuntime: null,
    intelligenceRuntimeError: null,
    rebuildQueueReport: null,
    searchEngineRuleDraft: null,
    searchEngineRuleDraftValid: false,
    searchEngineRuleError: null,
    searchEngineRules: [],
    searchEngineRulesLoading: false,
    onCancelRuntimeJob: vi.fn().mockResolvedValue(undefined),
    onCancelSearchEngineRuleEdit: vi.fn(),
    onClearDerivedState: vi.fn().mockResolvedValue(undefined),
    onDeleteSearchEngineRule: vi.fn().mockResolvedValue(undefined),
    onDeterministicModuleToggle: vi.fn().mockResolvedValue(undefined),
    onEditSearchEngineRule: vi.fn(),
    onEnrichmentPluginToggle: vi.fn().mockResolvedValue(undefined),
    onRebuildDerivedState: vi.fn().mockResolvedValue(undefined),
    onRetryRuntimeJob: vi.fn().mockResolvedValue(undefined),
    onSaveSearchEngineRule: vi.fn().mockResolvedValue(undefined),
    onSearchEngineRuleDraftChange: vi.fn(),
    onStartSearchEngineRule: vi.fn(),
  }
}
