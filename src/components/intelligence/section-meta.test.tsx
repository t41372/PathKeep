/**
 * Guards the compact evidence/freshness review popover against interaction and
 * metadata regressions.
 */

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test } from 'vitest'
import { IntelligenceSectionMeta } from './section-meta'
import { createNamespaceTranslator, I18nProvider } from '../../lib/i18n'
import type { CoreIntelligenceSectionMeta as SectionMeta } from '../../lib/core-intelligence'

const intelligenceT = createNamespaceTranslator('en', 'intelligence')
const settingsT = createNamespaceTranslator('en', 'settings')

function createMeta(overrides: Partial<SectionMeta> = {}): SectionMeta {
  return {
    sectionId: 'digest-summary',
    generatedAt: '2026-04-17T09:45:00Z',
    window: {
      kind: 'date-range',
      dateRange: { start: '2026-04-01', end: '2026-04-07' },
    },
    moduleIds: ['daily-rollups'],
    sourceTables: ['daily_summary_rollups'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: ['Fresh deterministic rebuild completed.'],
    ...overrides,
  }
}

function renderSectionMeta(meta: SectionMeta = createMeta()) {
  return render(
    <I18nProvider>
      <div>
        <IntelligenceSectionMeta meta={meta} scopeLabel="All profiles" />
        <button type="button">Outside action</button>
      </div>
    </I18nProvider>,
  )
}

describe('IntelligenceSectionMeta', () => {
  test('renders compact trigger badges and keeps the panel hidden by default', () => {
    renderSectionMeta()

    const root = screen.getByTestId('intelligence-section-meta-digest-summary')
    expect(
      within(root).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    expect(
      within(root).getByText(settingsT('deterministicModuleReady')),
    ).toBeVisible()
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
  })

  test('opens on click, pins the panel, and closes on outside click or second click', async () => {
    const user = userEvent.setup()
    renderSectionMeta(
      createMeta({
        state: 'stale',
        stateReason: 'Visibility changed after the last deterministic rebuild.',
      }),
    )

    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })

    await user.click(trigger)

    const panel = screen.getByTestId(
      'intelligence-section-meta-panel-digest-summary',
    )
    expect(
      within(panel).getByText(intelligenceT('sectionMetaGeneratedAt')),
    ).toBeVisible()
    expect(
      within(panel).getByText(
        'Visibility changed after the last deterministic rebuild.',
      ),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Outside action' }))
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      screen.getByTestId('intelligence-section-meta-panel-digest-summary'),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaClosePanelAria'),
      }),
    )
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
  })

  test('opens on hover and keyboard focus, and closes with Escape', async () => {
    const user = userEvent.setup()
    renderSectionMeta()

    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })

    await user.hover(trigger)
    expect(
      screen.getByTestId('intelligence-section-meta-panel-digest-summary'),
    ).toBeVisible()

    await user.unhover(trigger)
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()

    await user.tab()
    expect(trigger).toHaveFocus()
    expect(
      screen.getByTestId('intelligence-section-meta-panel-digest-summary'),
    ).toBeVisible()

    await user.keyboard('{Escape}')
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
  })

  test('degrades malformed window metadata instead of crashing', async () => {
    const user = userEvent.setup()
    const malformedMeta = createMeta({
      window: {
        kind: 'date-range',
      } as unknown as SectionMeta['window'],
    })

    expect(() => renderSectionMeta(malformedMeta)).not.toThrow()

    expect(
      screen.getByText(intelligenceT('sectionMetaStateDegraded')),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )

    expect(
      screen.getByText(intelligenceT('sectionMetaMetadataFallback')),
    ).toBeVisible()
  })
})
