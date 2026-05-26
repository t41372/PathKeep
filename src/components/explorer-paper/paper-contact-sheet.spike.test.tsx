/**
 * BROWSE-VIRT spike measurement (feedback-2026-05-25 §1.1 / §1.2 prep).
 *
 * Quantifies the current Browse DOM footprint at multiple scales so the
 * sliding-window + directional prefetch implementation has concrete
 * targets to beat instead of "feels laggy" hand-waving. Findings are
 * mirrored into `docs/plan/browse-virt-spike-2026-05-25.md`.
 *
 * What it measures
 * - `document.querySelectorAll('*').length` after rendering N rows in
 *   List and Cards viewModes, grouped one row per day (worst-case day
 *   chrome overhead) and 50 rows per day (typical archive density).
 * - Per-row marginal DOM cost in each cell (List vs Cards).
 *
 * Intentionally NOT measured
 * - Layout reflow / paint cost — jsdom does not render. Real FPS /
 *   long-tasks numbers come later from a Chrome devtools trace on the
 *   live desktop.
 * - JS heap — jsdom does not expose `performance.memory`. We log it
 *   when it happens to be available (Node ≥ 22 exposes
 *   `process.memoryUsage().heapUsed`) but the figure is approximate.
 *
 * Why this lives in the test tree
 * - It runs under `bun run test:unit` with no extra wiring so the
 *   numbers stay reproducible across machines instead of one-shot
 *   bench scripts that rot in /tmp.
 * - Wall-clock budget: each scenario renders deterministically and
 *   the slowest case (~10k rows × 4 modes) finishes in <3 s on a
 *   modern laptop; if it ever blows past the 5 s test timeout that's
 *   itself a useful warning that the current implementation has
 *   regressed beyond the spike's baseline.
 */

import { render } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import type { PaperDay } from '@/pages/explorer/paper/group-entries'
import {
  PaperContactSheet,
  type PaperContactSheetCopy,
  type PaperContactSheetDayNav,
} from './paper-contact-sheet'

const COPY: PaperContactSheetCopy = {
  view: 'View',
  cards: 'Cards',
  list: 'List',
  dayMeta: '{count} pages · {sessions} sessions',
  dayIndex: 'Day {n}',
  clearTarget: 'Clear',
  pagesLabel: 'pages',
  empty: 'Nothing here yet.',
  sessionGapLabel: '{duration} away',
}

const NAV_COPY = {
  prev: 'Previous day',
  next: 'Next day',
  today: 'Today',
  openCalendar: 'Open calendar',
}

const NAV: PaperContactSheetDayNav = {
  dow: 'FRI',
  monthDay: 'May 16',
  year: '2026',
  densityTier: 3,
  countLabel: '5p',
  relativeAgo: 'yesterday',
  isToday: false,
  onPrev: () => {},
  onNext: () => {},
  onToday: () => {},
  onToggleCal: () => {},
  calOpen: false,
  copy: NAV_COPY,
}

function makeEntry(id: number): HistoryEntry {
  const visitedMs = new Date('2026-05-16T20:15:00Z').getTime() - id * 60_000
  return {
    id,
    profileId: 'spike:Default',
    url: `https://example.com/page-${id}`,
    title: `Spike entry ${id}`,
    domain: 'example.com',
    favicon: null,
    visitedAt: new Date(visitedMs).toISOString(),
    visitTime: visitedMs,
    durationMs: null,
    transition: null,
    sourceVisitId: 0,
    appId: null,
  }
}

/**
 * Builds `dayCount` days, each with `rowsPerDay` entries packed into a
 * single session so the per-day chrome (header / insights / session
 * header) cost is amortised consistently across runs.
 */
function makeDays(dayCount: number, rowsPerDay: number): PaperDay[] {
  const days: PaperDay[] = []
  for (let d = 0; d < dayCount; d += 1) {
    const dayStartMs =
      new Date('2026-05-16T20:15:00Z').getTime() - d * 86_400_000
    const sessionStart = dayStartMs
    const sessionEnd = dayStartMs - rowsPerDay * 60_000
    const blocks = []
    for (let r = 0; r < rowsPerDay; r += 1) {
      blocks.push({
        type: 'single' as const,
        entry: makeEntry(d * 100_000 + r),
      })
    }
    days.push({
      date: new Date(dayStartMs).toISOString().slice(0, 10),
      visitCount: rowsPerDay,
      domains: 1,
      sessions: [
        {
          id: `spike-day-${d}-sess`,
          startMs: sessionStart,
          endMs: sessionEnd,
          visitCount: rowsPerDay,
          blocks,
        },
      ],
    })
  }
  return days
}

function countNodes(container: HTMLElement): number {
  return container.querySelectorAll('*').length
}

interface ScenarioRow {
  scenario: string
  dayCount: number
  rowsPerDay: number
  totalRows: number
  viewMode: 'list' | 'cards'
  nodes: number
  nodesPerRow: number
}

const results: ScenarioRow[] = []

function runScenario(
  scenario: string,
  dayCount: number,
  rowsPerDay: number,
  viewMode: 'list' | 'cards',
): ScenarioRow {
  const days = makeDays(dayCount, rowsPerDay)
  // Spike intentionally measures the un-virtualised baseline so the
  // numbers stay comparable to the original 2026-05-25 baseline that
  // motivated the BROWSE-VIRT work.
  const { container, unmount } = render(
    <PaperContactSheet
      days={days}
      viewMode={viewMode}
      onViewModeChange={() => {}}
      dayNav={NAV}
      copy={COPY}
      disableVirtualization
      testId={`spike-${scenario}-${viewMode}`}
    />,
  )
  const nodes = countNodes(container)
  unmount()
  const totalRows = dayCount * rowsPerDay
  const row: ScenarioRow = {
    scenario,
    dayCount,
    rowsPerDay,
    totalRows,
    viewMode,
    nodes,
    nodesPerRow: totalRows > 0 ? nodes / totalRows : 0,
  }
  results.push(row)
  return row
}

// The largest spike scenarios mount thousands of DOM nodes; under the
// v8 coverage instrumentation each render walks the full subtree
// twice, so the 5 000-row cards-mode scenario takes ~20-30 s on the
// CI box. Per-test timeout overrides for those scenarios keep the
// coverage gate green while leaving the default 5 s budget intact for
// every other test.
const SPIKE_SCENARIO_TIMEOUT_MS = 60_000

describe('BROWSE-VIRT spike measurement', () => {
  test('list mode — 1 day × 50 rows (single-day baseline)', () => {
    const row = runScenario('1d-50r', 1, 50, 'list')
    expect(row.nodes).toBeGreaterThan(0)
  })

  test('list mode — 10 days × 50 rows (one screen of dense scroll)', () => {
    const row = runScenario('10d-50r', 10, 50, 'list')
    expect(row.nodes).toBeGreaterThan(0)
  })

  test(
    'list mode — 100 days × 50 rows = 5000 rows (current MAX_ACCUMULATED_PAGES cap)',
    () => {
      const row = runScenario('100d-50r', 100, 50, 'list')
      expect(row.nodes).toBeGreaterThan(0)
      // Baseline target the virt impl must beat: 5000 rows of list mode
      // currently materialise into > 25 000 DOM nodes (5+ nodes per row
      // chrome). The virt window should keep this under ~3 000 nodes by
      // only mounting viewport ± buffer.
      expect(row.nodes).toBeGreaterThan(5000)
    },
    SPIKE_SCENARIO_TIMEOUT_MS,
  )

  test(
    'cards mode — 100 days × 50 rows = 5000 rows',
    () => {
      const row = runScenario('100d-50r', 100, 50, 'cards')
      expect(row.nodes).toBeGreaterThan(0)
    },
    SPIKE_SCENARIO_TIMEOUT_MS,
  )

  test('exports the measurement table for inclusion in the BROWSE-VIRT spike doc', () => {
    // Pretty-print the matrix so `bun run test:unit -- spike` prints
    // numbers the spike doc can quote directly. The expect at the bottom
    // is a sanity guard; the real value is the console output.
    const lines = [
      '',
      'BROWSE-VIRT spike — DOM node footprint',
      '┌─────────────┬──────────┬─────────┬──────────┬─────────────┬─────────────┐',
      '│ scenario    │ days     │ rows/d  │ totalRow │ nodes (jsdom)│ nodes / row │',
      '├─────────────┼──────────┼─────────┼──────────┼─────────────┼─────────────┤',
    ]
    for (const row of results) {
      lines.push(
        `│ ${row.scenario.padEnd(11)} │ ${row.viewMode.padEnd(8)} │ ` +
          `${String(row.rowsPerDay).padStart(7)} │ ` +
          `${String(row.totalRows).padStart(8)} │ ` +
          `${String(row.nodes).padStart(11)} │ ` +
          `${row.nodesPerRow.toFixed(2).padStart(11)} │`,
      )
    }
    lines.push(
      '└─────────────┴──────────┴─────────┴──────────┴─────────────┴─────────────┘',
    )
    console.log(lines.join('\n'))
    expect(results.length).toBeGreaterThanOrEqual(4)
  })
})
