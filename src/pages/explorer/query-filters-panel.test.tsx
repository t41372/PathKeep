/**
 * @file query-filters-panel.test.tsx
 * @description Focused interaction coverage for the Explorer filter chrome.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Verify the render-only Explorer filter owner wires mode, view, filter, chip, and recent-search actions.
 * - Cover active-filter and recent-search branches without mounting the full Explorer route.
 *
 * ## Not responsible for
 * - Re-testing URL debounce behavior or backend query loading.
 * - Re-testing grouped Explorer result panels.
 *
 * ## Dependencies
 * - Depends on shipped i18n namespaces for Explorer and Intelligence labels.
 *
 * ## Performance notes
 * - Direct component rendering keeps broad filter coverage fast and deterministic.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import { ExplorerQueryFiltersPanel } from './query-filters-panel'

const explorerT = createNamespaceTranslator('en', 'explorer')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')

describe('ExplorerQueryFiltersPanel', () => {
  test('wires active filter chips, mode/view controls, filters, and recent searches', async () => {
    const user = userEvent.setup()
    const clearAllFilters = vi.fn()
    const setQueryInput = vi.fn()
    const setSearchParams = vi.fn()
    const setView = vi.fn()
    const updateParam = vi.fn()

    render(
      <ExplorerQueryFiltersPanel
        activeFilters={[
          {
            id: 'domain',
            label: explorerT('filterDomain'),
            value: 'example.com',
          },
        ]}
        activeScopeLabel="Default"
        browserKinds={['chrome', 'safari']}
        buildRecentSearchLabel={(params) =>
          params.q ? `recent:${params.q}` : ''
        }
        clearAllFilters={clearAllFilters}
        explicitProfileId={null}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        mode="keyword"
        profileId="chrome:Default"
        queryInput=""
        recentSearches={[
          {
            label: 'Fallback label',
            params: {
              q: 'sqlite',
              domain: 'example.com',
              mode: null,
            },
          },
        ]}
        regexMode={false}
        regexValid={true}
        searchParams={new URLSearchParams('domain=example.com&sort=newest')}
        selectedProfileIds={['chrome:Default', 'safari:Default']}
        setQueryInput={setQueryInput}
        setSearchParams={setSearchParams}
        setView={setView}
        updateParam={updateParam}
        view="time"
        visibleRecordCount={42}
      />,
    )

    expect(
      screen.getByText(explorerT('visibleRecords', { count: 42 })),
    ).toBeVisible()
    expect(
      screen.getByRole('option', { name: 'Chrome · Default' }),
    ).toHaveValue('chrome:Default')
    expect(screen.getByLabelText(explorerT('filterStart'))).toHaveAttribute(
      'placeholder',
      explorerT('allRecordedTime'),
    )
    expect(screen.getByLabelText(explorerT('filterStart'))).toHaveValue('')
    expect(screen.getByLabelText(explorerT('filterEnd'))).toHaveAttribute(
      'placeholder',
      explorerT('allRecordedTime'),
    )
    expect(screen.getByLabelText(explorerT('filterEnd'))).toHaveValue('')
    expect(screen.queryByText('chrome:Default')).not.toBeInTheDocument()
    await user.click(
      screen.getByRole('button', {
        name: explorerT('removeFilter', {
          label: explorerT('filterDomain'),
          value: 'example.com',
        }),
      }),
    )
    await user.click(
      screen.getByRole('button', { name: explorerT('clearAllFilters') }),
    )
    await user.click(
      screen.getByRole('button', { name: explorerT('modeSemantic') }),
    )
    expect(
      screen.getByRole('button', { name: explorerT('modeSemantic') }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: explorerT('modeHybrid') }),
    ).toBeDisabled()
    await user.click(
      screen.getByRole('button', { name: intelligenceT('viewModeSession') }),
    )
    fireEvent.change(screen.getByLabelText(explorerT('filterKeywordAria')), {
      target: { value: 'sql' },
    })
    await user.click(
      screen.getByRole('button', { name: explorerT('toggleRegex') }),
    )
    fireEvent.change(screen.getByLabelText(explorerT('filterDomain')), {
      target: { value: 'openai.com' },
    })
    await user.selectOptions(
      screen.getByLabelText(explorerT('filterProfileAria')),
      'safari:Default',
    )
    await user.selectOptions(
      screen.getByLabelText(explorerT('filterBrowser')),
      'safari',
    )
    fireEvent.change(screen.getByLabelText(explorerT('filterStart')), {
      target: { value: '2026-04-01' },
    })
    fireEvent.change(screen.getByLabelText(explorerT('filterEnd')), {
      target: { value: '2026-04-25' },
    })
    await user.selectOptions(
      screen.getByLabelText(explorerT('filterSort')),
      'oldest',
    )
    await user.click(screen.getByRole('button', { name: 'recent:sqlite' }))

    expect(updateParam).toHaveBeenCalledWith('domain', null)
    expect(clearAllFilters).toHaveBeenCalledTimes(1)
    expect(updateParam).not.toHaveBeenCalledWith('mode', 'semantic')
    expect(setView).toHaveBeenCalledWith('session')
    expect(setQueryInput).toHaveBeenLastCalledWith('sql')
    expect(updateParam).toHaveBeenCalledWith('regex', '1')
    expect(updateParam).toHaveBeenCalledWith('domain', 'openai.com')
    expect(updateParam).toHaveBeenCalledWith('profileId', 'safari:Default')
    expect(updateParam).toHaveBeenCalledWith('browserKind', 'safari')
    expect(updateParam).toHaveBeenCalledWith('start', '2026-04-01')
    expect(updateParam).toHaveBeenCalledWith('end', '2026-04-25')
    expect(updateParam).toHaveBeenCalledWith('sort', 'oldest')
    expect(setSearchParams).toHaveBeenCalledTimes(1)
    const recentParams = setSearchParams.mock.calls[0]?.[0] as URLSearchParams
    expect(recentParams.get('q')).toBe('sqlite')
    expect(recentParams.get('domain')).toBe('example.com')
    expect(recentParams.has('mode')).toBe(false)
  })

  test('renders regex validity and disabled grouped-view controls for semantic mode', async () => {
    const user = userEvent.setup()
    const updateParam = vi.fn()

    render(
      <ExplorerQueryFiltersPanel
        activeFilters={[]}
        activeScopeLabel={null}
        browserKinds={[]}
        buildRecentSearchLabel={() => ''}
        clearAllFilters={vi.fn()}
        explicitProfileId="chrome:Default"
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        mode="semantic"
        profileId="chrome:Default"
        queryInput="["
        recentSearches={[]}
        regexMode={true}
        regexValid={false}
        searchParams={new URLSearchParams()}
        selectedProfileIds={['chrome:Default']}
        setQueryInput={vi.fn()}
        setSearchParams={vi.fn()}
        setView={vi.fn()}
        updateParam={updateParam}
        view="time"
        visibleRecordCount={null}
      />,
    )

    expect(screen.getByText(explorerT('waitingForQuery'))).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent(
      explorerT('regexInvalid'),
    )
    expect(
      screen.getByRole('button', { name: intelligenceT('viewModeSession') }),
    ).toBeDisabled()
    expect(screen.getByText(explorerT('recentFiltersEmpty'))).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: explorerT('toggleRegex') }),
    )
    await user.click(
      screen.getByRole('button', { name: explorerT('modeKeyword') }),
    )

    expect(updateParam).toHaveBeenCalledWith('regex', null)
    expect(updateParam).toHaveBeenCalledWith('mode', null)
  })

  test('wires semantic and hybrid mode buttons when optional AI is release-enabled', async () => {
    const user = userEvent.setup()
    const updateParam = vi.fn()
    vi.resetModules()
    vi.doMock('../../lib/release-capabilities', () => ({
      deferredFeatureReleaseLabel: 'v0.2',
      optionalAiFeaturesAvailable: true,
      readableContentFetchAvailable: false,
    }))

    try {
      const { ExplorerQueryFiltersPanel: EnabledExplorerQueryFiltersPanel } =
        await import('./query-filters-panel')

      render(
        <EnabledExplorerQueryFiltersPanel
          activeFilters={[]}
          activeScopeLabel={null}
          browserKinds={[]}
          buildRecentSearchLabel={() => ''}
          clearAllFilters={vi.fn()}
          explicitProfileId={null}
          explorerT={explorerT}
          intelligenceT={intelligenceT}
          mode="keyword"
          profileId={null}
          queryInput=""
          recentSearches={[]}
          regexMode={false}
          regexValid={true}
          searchParams={new URLSearchParams()}
          selectedProfileIds={[]}
          setQueryInput={vi.fn()}
          setSearchParams={vi.fn()}
          setView={vi.fn()}
          updateParam={updateParam}
          view="time"
          visibleRecordCount={null}
        />,
      )

      await user.click(
        screen.getByRole('button', { name: explorerT('modeSemantic') }),
      )
      await user.click(
        screen.getByRole('button', { name: explorerT('modeHybrid') }),
      )

      expect(updateParam).toHaveBeenCalledWith('mode', 'semantic')
      expect(updateParam).toHaveBeenCalledWith('mode', 'hybrid')
    } finally {
      vi.doUnmock('../../lib/release-capabilities')
      vi.resetModules()
    }
  })

  test('clears optional filters and falls back to recent-search labels', async () => {
    const user = userEvent.setup()
    const setSearchParams = vi.fn()
    const updateParam = vi.fn()

    render(
      <ExplorerQueryFiltersPanel
        activeFilters={[]}
        activeScopeLabel="Default"
        browserKinds={['chrome']}
        buildRecentSearchLabel={() => ''}
        clearAllFilters={vi.fn()}
        explicitProfileId={null}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        mode="keyword"
        profileId="chrome:Default"
        queryInput="sqlite"
        recentSearches={[
          {
            label: 'Fallback label',
            params: {
              browserKind: null,
              domain: null,
              q: null,
            },
          },
        ]}
        regexMode={true}
        regexValid={true}
        searchParams={
          new URLSearchParams(
            'browserKind=chrome&domain=example.com&end=2026-04-30&start=2026-04-01&sort=oldest',
          )
        }
        selectedProfileIds={['chrome:Default']}
        setQueryInput={vi.fn()}
        setSearchParams={setSearchParams}
        setView={vi.fn()}
        updateParam={updateParam}
        view="time"
        visibleRecordCount={1}
      />,
    )

    expect(screen.getByText(explorerT('regexValid'))).toBeVisible()
    expect(screen.getByText('Fallback label')).toBeVisible()
    expect(screen.getByLabelText(explorerT('filterStart'))).toHaveValue(
      '2026-04-01',
    )
    expect(screen.getByLabelText(explorerT('filterEnd'))).toHaveValue(
      '2026-04-30',
    )

    fireEvent.change(screen.getByLabelText(explorerT('filterDomain')), {
      target: { value: '' },
    })
    await user.selectOptions(
      screen.getByLabelText(explorerT('filterProfileAria')),
      '',
    )
    await user.selectOptions(
      screen.getByLabelText(explorerT('filterBrowser')),
      '',
    )
    fireEvent.change(screen.getByLabelText(explorerT('filterStart')), {
      target: { value: '' },
    })
    fireEvent.change(screen.getByLabelText(explorerT('filterEnd')), {
      target: { value: '' },
    })
    await user.click(screen.getByRole('button', { name: 'Fallback label' }))

    expect(updateParam).toHaveBeenCalledWith('domain', null)
    expect(updateParam).toHaveBeenCalledWith('profileId', null)
    expect(updateParam).toHaveBeenCalledWith('browserKind', null)
    expect(updateParam).toHaveBeenCalledWith('start', null)
    expect(updateParam).toHaveBeenCalledWith('end', null)
    expect(setSearchParams).toHaveBeenCalledTimes(1)
  })
})
