/**
 * @file derived-state-section.test.tsx
 * @description Render-level coverage for the Settings derived-state section shell.
 * @module pages/settings
 */

import { render } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
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
