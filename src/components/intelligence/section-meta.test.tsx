/**
 * Guards the compact evidence/freshness review popover against interaction and
 * metadata regressions.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { IntelligenceSectionMeta } from './section-meta'
import { createNamespaceTranslator, I18nProvider } from '../../lib/i18n'
import type { CoreIntelligenceSectionMeta as SectionMeta } from '../../lib/core-intelligence'
import { formatDateTime } from '../../lib/format'

const intelligenceT = createNamespaceTranslator('en', 'intelligence')
const settingsT = createNamespaceTranslator('en', 'settings')
const commonT = createNamespaceTranslator('en', 'common')

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

function panelFor(sectionId = 'digest-summary') {
  return screen.getByTestId(`intelligence-section-meta-panel-${sectionId}`)
}

function expectPanelRow(
  panel: HTMLElement,
  label: string,
  value: string | RegExp,
) {
  const labelNode = within(panel).getByText(label)
  const row = labelNode.closest('.intelligence-section-meta__row')
  expect(row).not.toBeNull()
  expect(row).toHaveTextContent(value)
}

describe('IntelligenceSectionMeta', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('renders compact trigger badges and keeps the panel hidden by default', () => {
    renderSectionMeta()

    const root = screen.getByTestId('intelligence-section-meta-digest-summary')
    expect(
      screen.getByTestId('intelligence-section-meta-trigger-digest-summary'),
    ).toBeVisible()
    expect(
      within(root).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    const readyBadge = within(root).getByText(
      settingsT('deterministicModuleReady'),
    )
    expect(readyBadge).toBeVisible()
    expect(readyBadge).toHaveClass(
      'status-badge',
      'intelligence-section-meta__state',
      'intelligence-section-meta__state--ready',
    )
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
  })

  test('opens on click, pins the panel, renders complete metadata, and recovers after closing', async () => {
    const user = userEvent.setup()
    renderSectionMeta(
      createMeta({
        moduleIds: ['daily-rollups', 'sessions'],
        state: 'stale',
        stateReason: 'Visibility changed after the last deterministic rebuild.',
        sourceTables: ['daily_summary_rollups', 'visit_facts'],
      }),
    )

    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })

    await user.click(trigger)

    const panel = panelFor()
    expect(
      within(panel).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    const panelStateBadge = within(panel).getByText(
      settingsT('deterministicModuleStale'),
    )
    expect(panelStateBadge).toHaveClass(
      'status-badge',
      'intelligence-section-meta__state',
      'intelligence-section-meta__state--stale',
    )
    expectPanelRow(
      panel,
      intelligenceT('sectionMetaGeneratedAt'),
      formatDateTime('2026-04-17T09:45:00Z', 'en') ?? '',
    )
    expectPanelRow(panel, intelligenceT('sectionMetaScope'), 'All profiles')
    expectPanelRow(
      panel,
      intelligenceT('sectionMetaWindow'),
      intelligenceT('sectionMetaWindowDateRange', {
        start: '2026-04-01',
        end: '2026-04-07',
      }),
    )
    expectPanelRow(
      panel,
      intelligenceT('sectionMetaModules'),
      'Daily rollups, Sessions',
    )
    expectPanelRow(
      panel,
      intelligenceT('sectionMetaSourceTables'),
      'daily_summary_rollups, visit_facts',
    )
    expectPanelRow(panel, intelligenceT('sectionMetaEnrichment'), 'No')
    expectPanelRow(
      panel,
      intelligenceT('sectionMetaStateReason'),
      'Visibility changed after the last deterministic rebuild.',
    )
    expect(
      within(panel).getByText(intelligenceT('sectionMetaNotes')),
    ).toBeVisible()
    expect(
      within(panel).getByText('Fresh deterministic rebuild completed.'),
    ).toHaveClass('mono-support')

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

    fireEvent.mouseEnter(
      screen.getByTestId('intelligence-section-meta-digest-summary'),
    )
    expect(panelFor()).toBeVisible()
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

    fireEvent.mouseLeave(
      screen.getByTestId('intelligence-section-meta-digest-summary'),
    )
    expect(panelFor()).toBeVisible()

    await user.keyboard('{Escape}')
    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()

    await user.hover(trigger)
    expect(panelFor()).toBeVisible()
  })

  test('keeps a pinned panel open across non-closing keyboard and blur events', async () => {
    const user = userEvent.setup()
    renderSectionMeta()

    const root = screen.getByTestId('intelligence-section-meta-digest-summary')
    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })
    const outside = screen.getByRole('button', { name: 'Outside action' })

    await user.click(trigger)
    fireEvent.mouseEnter(root)
    fireEvent.keyDown(document, { key: 'Enter' })
    fireEvent.blur(root, { relatedTarget: outside })

    expect(
      screen.getByTestId('intelligence-section-meta-panel-digest-summary'),
    ).toBeVisible()
  })

  test('only attaches document listeners while the panel state needs them', async () => {
    const addSpy = vi.spyOn(document, 'addEventListener')
    const removeSpy = vi.spyOn(document, 'removeEventListener')
    const user = userEvent.setup()
    renderSectionMeta()

    expect(addSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(false)
    expect(addSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(false)
    expect(addSpy.mock.calls.some(([type]) => type === 'touchstart')).toBe(
      false,
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )

    expect(addSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(true)
    expect(addSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(true)
    expect(addSpy.mock.calls.some(([type]) => type === 'touchstart')).toBe(true)

    fireEvent.mouseDown(document.body)

    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
    expect(removeSpy.mock.calls.some(([type]) => type === 'keydown')).toBe(true)
    expect(removeSpy.mock.calls.some(([type]) => type === 'mousedown')).toBe(
      true,
    )
    expect(removeSpy.mock.calls.some(([type]) => type === 'touchstart')).toBe(
      true,
    )
  })

  test('closes focus-opened panels on outside blur but keeps focus inside the root open', () => {
    renderSectionMeta()

    const root = screen.getByTestId('intelligence-section-meta-digest-summary')
    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })
    const outside = screen.getByRole('button', { name: 'Outside action' })

    fireEvent.focusIn(trigger, { relatedTarget: outside })
    expect(panelFor()).toBeVisible()

    fireEvent.focusOut(root, { relatedTarget: outside })
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

  test('treats every malformed metadata window shape as degraded unavailable metadata', async () => {
    const malformedWindows: Array<[string, unknown]> = [
      ['null window', null],
      ['primitive window', 'date-range'],
      [
        'callable date range',
        Object.assign(() => {}, {
          kind: 'date-range',
          dateRange: { start: '2026-04-01', end: '2026-04-07' },
        }),
      ],
      [
        'callable calendar day',
        Object.assign(() => {}, {
          kind: 'calendar-day-history',
          referenceDate: '2026-04-25',
        }),
      ],
      ['missing dateRange', { kind: 'date-range' }],
      ['null dateRange', { kind: 'date-range', dateRange: null }],
      [
        'non-string start',
        { kind: 'date-range', dateRange: { start: 1, end: '2026-04-07' } },
      ],
      [
        'non-string end',
        { kind: 'date-range', dateRange: { start: '2026-04-01', end: 7 } },
      ],
      ['missing calendar reference', { kind: 'calendar-day-history' }],
      [
        'non-string calendar reference',
        { kind: 'calendar-day-history', referenceDate: 20260425 },
      ],
      [
        'reference date with wrong kind',
        { kind: 'date-range', referenceDate: '2026-04-25' },
      ],
    ]

    for (const [caseName, window] of malformedWindows) {
      const user = userEvent.setup()
      const view = renderSectionMeta(
        createMeta({
          sectionId: `digest-summary-${caseName.replaceAll(' ', '-')}`,
          notes: ['Source note still visible.'],
          window: window as SectionMeta['window'],
        }),
      )

      expect(
        screen.getByText(intelligenceT('sectionMetaStateDegraded')),
      ).toBeVisible()

      await user.click(
        screen.getByRole('button', {
          name: intelligenceT('sectionMetaOpenPanelAria'),
        }),
      )

      const panel = panelFor(`digest-summary-${caseName.replaceAll(' ', '-')}`)
      expectPanelRow(
        panel,
        intelligenceT('sectionMetaWindow'),
        commonT('notAvailable'),
      )
      expect(
        within(panel).getByText(intelligenceT('sectionMetaMetadataFallback')),
      ).toBeVisible()
      expect(
        within(panel).getByText('Source note still visible.'),
      ).toBeVisible()

      view.unmount()
    }
  })

  test('renders calendar-day metadata windows', async () => {
    const user = userEvent.setup()
    renderSectionMeta(
      createMeta({
        window: {
          kind: 'calendar-day-history',
          dateRange: { start: '2026-04-01', end: '2026-04-07' },
          referenceDate: '2026-04-25',
        } as unknown as SectionMeta['window'],
      }),
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )

    expect(
      screen.getByText(
        intelligenceT('sectionMetaWindowCalendarDayHistory', {
          date: '2026-04-25',
        }),
      ),
    ).toBeVisible()
  })

  test('renders metadata fallbacks and enrichment-enabled state', async () => {
    const user = userEvent.setup()
    const first = renderSectionMeta(
      createMeta({
        generatedAt: 'not-a-valid-date',
        includesEnrichment: true,
        moduleIds: [],
        notes: [],
        sourceTables: [],
        state: 'disabled',
      }),
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )

    const panel = screen.getByTestId(
      'intelligence-section-meta-panel-digest-summary',
    )
    expect(
      within(panel).getByText(settingsT('deterministicModuleDisabled')),
    ).toBeVisible()
    expect(
      within(panel).getByText(intelligenceT('sectionMetaDirectRead')),
    ).toBeVisible()
    expect(
      within(panel).getAllByText(commonT('notAvailable')).length,
    ).toBeGreaterThan(0)
    expect(within(panel).getByText('not-a-valid-date')).toBeVisible()
    expect(
      within(panel).getByText(intelligenceT('sectionMetaEnrichmentEnabled')),
    ).toBeVisible()
    first.unmount()

    renderSectionMeta(
      createMeta({
        generatedAt: null,
        notes: [],
        sourceTables: [],
      }),
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expectPanelRow(
      panelFor(),
      intelligenceT('sectionMetaGeneratedAt'),
      commonT('notAvailable'),
    )
  })

  test('keeps multiple notes keyed distinctly without React duplicate-key warnings', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    const user = userEvent.setup()
    renderSectionMeta(
      createMeta({
        notes: ['First review note.', 'Second review note.'],
      }),
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )

    expect(screen.getByText('First review note.')).toBeVisible()
    expect(screen.getByText('Second review note.')).toBeVisible()
    expect(consoleError).not.toHaveBeenCalledWith(
      expect.stringContaining('Encountered two children with the same key'),
      expect.anything(),
    )
  })

  test('keeps focus transitions inside the metadata root from reopening the panel', () => {
    renderSectionMeta()

    const trigger = screen.getByRole('button', {
      name: intelligenceT('sectionMetaOpenPanelAria'),
    })

    fireEvent.focus(trigger, { relatedTarget: trigger })

    expect(
      screen.queryByTestId('intelligence-section-meta-panel-digest-summary'),
    ).not.toBeInTheDocument()
  })
})
