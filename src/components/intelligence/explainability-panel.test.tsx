/**
 * Verifies that explainability copy is localized instead of leaking backend rule strings verbatim.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ExplainabilityPanel } from './explainability-panel'
import { createNamespaceTranslator } from '../../lib/i18n'
import * as api from '../../lib/core-intelligence/api'
import type { Explanation } from '../../lib/core-intelligence/types'

vi.mock('../../lib/core-intelligence/api', () => ({
  explainEntity: vi.fn(),
}))

const intelligenceT = createNamespaceTranslator('zh-CN', 'intelligence')
const echoT = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('ExplainabilityPanel', () => {
  beforeEach(() => {
    vi.mocked(api.explainEntity).mockReset()
  })

  test('localizes trigger rules and factor labels for preloaded explanations', async () => {
    const user = userEvent.setup()

    render(
      <ExplainabilityPanel
        entityType="refind_page"
        entityId="https://example.com/docs"
        explanation={{
          entityType: 'refind_page',
          entityId: 'https://example.com/docs',
          triggerRule: 'Refind score >= 3.2',
          factors: [
            {
              label: 'cross_day_count',
              rawValue: 4,
              weight: 1,
              contribution: 4,
            },
          ],
          participatingVisitIds: [1, 2],
        }}
        t={intelligenceT}
      />,
    )

    await user.click(
      screen.getByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    )

    expect(
      screen.getByText(
        intelligenceT('explainRuleRefindScore', { score: '3.2' }),
      ),
    ).toBeVisible()
    expect(
      screen.getByText(intelligenceT('explainFactorCrossDayCount')),
    ).toBeVisible()
  })

  test('renders the full factor-label map, table chrome, visit cap, and factor bar scale', async () => {
    const user = userEvent.setup()
    const factorLabels = [
      ['cross_day_count', 'explainFactorCrossDayCount'],
      ['trail_count', 'explainFactorTrailCount'],
      ['search_arrival_count', 'explainFactorSearchArrivalCount'],
      ['typed_revisit_count', 'explainFactorTypedRevisitCount'],
      ['visit_count', 'explainFactorVisitCount'],
      ['search_count', 'explainFactorSearchCount'],
      ['unique_domain_count', 'explainFactorUniqueDomainCount'],
      ['navigation_chain_depth', 'explainFactorNavigationChainDepth'],
      ['duration_minutes', 'explainFactorDurationMinutes'],
      ['reformulation_count', 'explainFactorReformulationCount'],
      ['max_depth', 'explainFactorMaxDepth'],
      ['landing_detected', 'explainFactorLandingDetected'],
      ['member_count', 'explainFactorMemberCount'],
      ['distinct_query_count', 'explainFactorDistinctQueryCount'],
      ['occurrence_count', 'explainFactorOccurrenceCount'],
      ['distinct_days', 'explainFactorDistinctDays'],
      ['domain_count', 'explainFactorDomainCount'],
      ['mean_interval_days', 'explainFactorMeanIntervalDays'],
      ['page_count', 'explainFactorPageCount'],
      ['coefficient_of_variation', 'explainFactorCoefficientOfVariation'],
      ['interrupted', 'explainFactorInterrupted'],
      ['step_count', 'explainFactorStepCount'],
      ['alternation_count', 'explainFactorAlternationCount'],
    ] as const

    render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="entity-1"
        explanation={explanationFixture({
          factors: factorLabels.map(([label], index) => ({
            label,
            rawValue: index,
            weight: index + 0.5,
            contribution: index === 0 ? 10 : index === 1 ? 5 : 0,
          })),
          participatingVisitIds: Array.from(
            { length: 23 },
            (_, index) => index + 1,
          ),
        })}
        t={echoT}
      />,
    )

    expect(screen.getByText('▸')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'explainTitle' }))

    expect(screen.getByText('▾')).toBeVisible()
    expect(screen.getByText('explainRule')).toBeVisible()
    expect(screen.getByText('explainFactors')).toBeVisible()
    expect(screen.getByText('explainFactorName')).toBeVisible()
    expect(screen.getByText('explainFactorRaw')).toBeVisible()
    expect(screen.getByText('explainFactorWeight')).toBeVisible()
    expect(screen.getByText('explainFactorContribution')).toBeVisible()
    for (const [, expectedKey] of factorLabels) {
      expect(screen.getByText(expectedKey)).toBeVisible()
    }

    const bars = document.querySelectorAll<HTMLElement>(
      '.explainability-panel__factor-bar',
    )
    expect(bars[0]).toHaveStyle({ width: '100%' })
    expect(bars[1]).toHaveStyle({ width: '50%' })
    expect(bars[2]).toHaveStyle({ width: '0%' })
    expect(screen.getByText('10.0')).toBeVisible()
    expect(screen.getByText('×0.5')).toBeVisible()
    expect(screen.getByText('explainVisits:{"count":23}')).toBeVisible()
    expect(screen.getByText('#1')).toBeVisible()
    expect(screen.getByText('#20')).toBeVisible()
    expect(screen.queryByText('#21')).not.toBeInTheDocument()
    expect(screen.getByText('+3')).toBeVisible()
  })

  test('omits optional factor and visit sections for empty explanations and handles zero bars', async () => {
    const user = userEvent.setup()
    render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="entity-2"
        explanation={explanationFixture({
          factors: [],
          participatingVisitIds: [],
        })}
        t={echoT}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'explainTitle' }))

    expect(screen.queryByText('explainFactors')).not.toBeInTheDocument()
    expect(screen.queryByText(/^explainVisits/)).not.toBeInTheDocument()
    expect(
      document.querySelector('.explainability-panel__factors-table'),
    ).not.toBeInTheDocument()
    expect(
      document.querySelector('.explainability-panel__visits'),
    ).not.toBeInTheDocument()

    render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="entity-3"
        explanation={explanationFixture({
          factors: [
            {
              label: 'zero_metric',
              rawValue: 0,
              weight: 1,
              contribution: 0,
            },
          ],
        })}
        t={echoT}
      />,
    )

    await user.click(screen.getAllByRole('button', { name: 'explainTitle' })[1])
    expect(
      document.querySelectorAll<HTMLElement>(
        '.explainability-panel__factor-bar',
      )[0],
    ).toHaveStyle({ width: '0%' })

    render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="entity-4"
        explanation={explanationFixture({
          participatingVisitIds: Array.from(
            { length: 20 },
            (_, index) => index + 1,
          ),
        })}
        t={echoT}
      />,
    )

    await user.click(screen.getAllByRole('button', { name: 'explainTitle' })[2])
    expect(screen.getByText('explainVisits:{"count":20}')).toBeVisible()
    expect(screen.getByText('#20')).toBeVisible()
    expect(screen.queryByText('+0')).not.toBeInTheDocument()
  })

  test('shows loading skeleton while async explanations are pending', async () => {
    const user = userEvent.setup()
    vi.mocked(api.explainEntity).mockReturnValue(new Promise(() => {}))

    const { container } = render(
      <ExplainabilityPanel entityType="session" entityId="pending" t={echoT} />,
    )

    await user.click(screen.getByRole('button', { name: 'explainTitle' }))

    expect(container.querySelector('.intelligence-skeleton--card')).toHaveStyle(
      {
        height: '100px',
      },
    )
  })

  test('fetches missing explanations and caps the rendered visit list', async () => {
    const user = userEvent.setup()
    vi.mocked(api.explainEntity).mockResolvedValue(
      explanationFixture({
        triggerRule:
          "Search trail anchored by 'tauri v2' and extended through navigation ancestry within the session window.",
        factors: [
          {
            label: 'unknown_metric',
            rawValue: 0,
            weight: 0.5,
            contribution: 0,
          },
        ],
        participatingVisitIds: Array.from(
          { length: 23 },
          (_, index) => index + 1,
        ),
      }),
    )

    render(
      <ExplainabilityPanel
        entityType="trail"
        entityId="trail-1"
        t={intelligenceT}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )

    expect(api.explainEntity).toHaveBeenCalledWith('trail', 'trail-1')
    expect(
      await screen.findByText(
        intelligenceT('explainRuleSearchTrail', { query: 'tauri v2' }),
      ),
    ).toBeVisible()
    expect(screen.getByText('unknown metric')).toBeVisible()
    expect(screen.getByText('+3')).toBeVisible()
  })

  test('surfaces missing and failed explanation loads without crashing the panel', async () => {
    const user = userEvent.setup()
    vi.mocked(api.explainEntity).mockResolvedValueOnce(null as never)

    const { unmount } = render(
      <ExplainabilityPanel
        entityType="session"
        entityId="session-1"
        t={intelligenceT}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )

    expect(
      await screen.findByText(intelligenceT('explainUnavailable')),
    ).toBeVisible()
    unmount()

    vi.mocked(api.explainEntity).mockRejectedValueOnce('bridge offline')
    const second = render(
      <ExplainabilityPanel
        entityType="session"
        entityId="session-2"
        t={intelligenceT}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )

    await waitFor(() => {
      expect(screen.getByText('bridge offline')).toBeVisible()
    })
    second.unmount()

    vi.mocked(api.explainEntity).mockRejectedValueOnce(
      new Error('desktop command failed'),
    )
    render(
      <ExplainabilityPanel
        entityType="session"
        entityId="session-3"
        t={intelligenceT}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )

    await waitFor(() => {
      expect(screen.getByText('desktop command failed')).toBeVisible()
    })
  })

  test('localizes the trigger rule from the stable CODE + params, not the prose', async () => {
    const user = userEvent.setup()
    // The PRIMARY path, covering EVERY code the backend ships: the panel reads `triggerRuleCode` + the
    // structured params, so the English `triggerRule` prose is irrelevant. Most cases omit it entirely
    // (the shape a front-end-composed explanation has) and the session-gap case supplies a MISLEADING
    // sentence to prove the code path wins and nothing recites backend prose.
    const coded = [
      [
        { triggerRuleCode: 'refind-score', triggerRuleScore: 3.24 },
        intelligenceT('explainRuleRefindScore', { score: '3.2' }),
      ],
      [
        { triggerRuleCode: 'session-deep-dive' },
        intelligenceT('explainRuleSessionDeepDive'),
      ],
      [
        {
          triggerRule:
            'Visits were grouped into one session because adjacent gaps stayed within 30 minutes.',
          triggerRuleCode: 'session-gap',
          triggerRuleGapMinutes: 45,
        },
        intelligenceT('explainRuleSessionGap', { minutes: 45 }),
      ],
      [
        {
          triggerRuleCode: 'search-trail',
          // An anchor query full of quote characters: the code path passes it structurally, so the
          // quotes that used to break the prose regex are irrelevant.
          triggerRuleQuery: "it's 'quoted' \"twice\"",
        },
        intelligenceT('explainRuleSearchTrail', {
          query: "it's 'quoted' \"twice\"",
        }),
      ],
      [
        {
          triggerRuleCode: 'query-family',
          triggerRuleQuery: "anchor with 'quotes'",
        },
        intelligenceT('explainRuleQueryFamily', {
          query: "anchor with 'quotes'",
        }),
      ],
      [
        { triggerRuleCode: 'reopened-investigation' },
        intelligenceT('explainRuleReopenedInvestigation'),
      ],
      [
        // The habit TYPE arrives as its persisted code, so the localized habit NAME comes from the
        // catalog rather than from a fragment captured out of a sentence.
        {
          triggerRuleCode: 'habit-pattern',
          triggerRuleHabitType: 'daily_habit',
        },
        intelligenceT('explainRuleHabitPattern', {
          habit: intelligenceT('habitType_daily_habit'),
        }),
      ],
      [
        {
          triggerRuleCode: 'habit-pattern-interrupted',
          triggerRuleHabitType: 'weekly_habit',
        },
        intelligenceT('explainRuleHabitPatternInterrupted', {
          habit: intelligenceT('habitType_weekly_habit'),
        }),
      ],
      [
        // An unmapped habit type still degrades readably (underscores → spaces), never to a key.
        {
          triggerRuleCode: 'habit-pattern',
          triggerRuleHabitType: 'custom_habit',
        },
        intelligenceT('explainRuleHabitPattern', { habit: 'custom habit' }),
      ],
      [{ triggerRuleCode: 'path-flow' }, intelligenceT('explainRulePathFlow')],
      [
        { triggerRuleCode: 'compare-set' },
        intelligenceT('explainRuleCompareSet'),
      ],
    ] as const

    for (const [overrides, expected] of coded) {
      const { unmount } = render(
        <ExplainabilityPanel
          entityType="entity"
          entityId={overrides.triggerRuleCode}
          // `triggerRule` is deliberately ABSENT on most cases — the panel must never need it.
          explanation={{
            entityType: 'refind_page',
            entityId: 'entity-1',
            factors: [],
            participatingVisitIds: [],
            ...overrides,
          }}
          t={intelligenceT}
        />,
      )
      await user.click(
        screen.getByRole('button', { name: intelligenceT('explainTitle') }),
      )
      expect(screen.getByText(expected)).toBeVisible()
      unmount()
    }
  })

  test('missing params on a coded rule degrade to a zero/empty value, never a crash', async () => {
    const user = userEvent.setup()
    // Defensive: a code whose param the backend omitted still renders localized copy rather than
    // "undefined" or a blank rule row.
    const missingParams = [
      [
        'refind-score',
        intelligenceT('explainRuleRefindScore', { score: '0.0' }),
      ],
      ['session-gap', intelligenceT('explainRuleSessionGap', { minutes: 0 })],
      ['search-trail', intelligenceT('explainRuleSearchTrail', { query: '' })],
      ['query-family', intelligenceT('explainRuleQueryFamily', { query: '' })],
      [
        'habit-pattern',
        intelligenceT('explainRuleHabitPattern', { habit: '' }),
      ],
      [
        'habit-pattern-interrupted',
        intelligenceT('explainRuleHabitPatternInterrupted', { habit: '' }),
      ],
    ] as const

    for (const [code, expected] of missingParams) {
      const { container, unmount } = render(
        <ExplainabilityPanel
          entityType="entity"
          entityId={code}
          explanation={explanationFixture({
            triggerRule: undefined,
            triggerRuleCode: code,
          })}
          t={intelligenceT}
        />,
      )
      await user.click(
        screen.getByRole('button', { name: intelligenceT('explainTitle') }),
      )
      // Compared against the raw text rather than through `getByText`: an empty interpolation can
      // leave a double space that the query's whitespace normalizer would collapse away.
      expect(
        container.querySelector('.explainability-panel__rule-value')
          ?.textContent,
      ).toBe(expected)
      unmount()
    }
  })

  test('an UNKNOWN code falls back to the legacy prose matcher', async () => {
    const user = userEvent.setup()
    // Forward compatibility: a code this build has no copy for must not blank the row — the legacy
    // English matcher still resolves the prose the backend shipped alongside it.
    render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="unknown-code"
        explanation={explanationFixture({
          triggerRule:
            'This flow pattern recurs across session-local domain n-grams.',
          triggerRuleCode: 'teleport-rule',
        })}
        t={intelligenceT}
      />,
    )
    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )
    expect(screen.getByText(intelligenceT('explainRulePathFlow'))).toBeVisible()
  })

  test('renders an empty rule value when a payload carries neither code nor prose', async () => {
    const user = userEvent.setup()
    // `triggerRule` is optional, so the legacy matcher must guard it: no code + no prose is an empty
    // rule value, never "undefined" and never a crash.
    const { container } = render(
      <ExplainabilityPanel
        entityType="entity"
        entityId="bare"
        explanation={explanationFixture({ triggerRule: undefined })}
        t={intelligenceT}
      />,
    )
    await user.click(
      screen.getByRole('button', { name: intelligenceT('explainTitle') }),
    )
    expect(
      container.querySelector('.explainability-panel__rule-value'),
    ).toHaveTextContent('')
  })

  test('covers deterministic rule localization variants used by intelligence cards', async () => {
    const user = userEvent.setup()
    const rules = [
      [
        'Deep dive session matched the navigation-depth, domain-count, and visit-count thresholds.',
        intelligenceT('explainRuleSessionDeepDive'),
      ],
      [
        'Visits were grouped into one session because adjacent gaps stayed within 30 minutes.',
        // The legacy matcher CAPTURES the threshold from the prose, so the localized copy quotes the
        // number the backend actually applied rather than a hard-coded 30.
        intelligenceT('explainRuleSessionGap', { minutes: '30' }),
      ],
      [
        "Queries were merged into one family because their Jaccard or containment similarity matched 'pathkeep'.",
        intelligenceT('explainRuleQueryFamily', { query: 'pathkeep' }),
      ],
      [
        'This investigation reopened because the same anchor reappeared across distinct days or repeated deterministic evidence.',
        intelligenceT('explainRuleReopenedInvestigation'),
      ],
      [
        'weekly_habit cadence was detected and later crossed its interruption threshold.',
        intelligenceT('explainRuleHabitPatternInterrupted', {
          habit: intelligenceT('habitType_weekly_habit'),
        }),
      ],
      [
        'daily_habit cadence was detected from repeated cross-day visits.',
        intelligenceT('explainRuleHabitPattern', {
          habit: intelligenceT('habitType_daily_habit'),
        }),
      ],
      [
        'periodic_reference cadence was detected from repeated cross-day visits.',
        intelligenceT('explainRuleHabitPattern', {
          habit: intelligenceT('habitType_periodic_reference'),
        }),
      ],
      [
        'custom_habit cadence was detected from repeated cross-day visits.',
        intelligenceT('explainRuleHabitPattern', { habit: 'custom habit' }),
      ],
      [
        'This flow pattern recurs across session-local domain n-grams.',
        intelligenceT('explainRulePathFlow'),
      ],
      [
        'This compare set alternated between multiple comparable pages within one search trail.',
        intelligenceT('explainRuleCompareSet'),
      ],
      ['Backend fallback rule', 'Backend fallback rule'],
    ] as const

    for (const [rule, expected] of rules) {
      const { unmount } = render(
        <ExplainabilityPanel
          entityType="entity"
          entityId={rule}
          explanation={explanationFixture({ triggerRule: rule })}
          t={intelligenceT}
        />,
      )

      await user.click(
        screen.getByRole('button', { name: intelligenceT('explainTitle') }),
      )

      expect(screen.getByText(expected)).toBeVisible()
      unmount()
    }
  })

  test('does not localize near-match backend rule strings', async () => {
    const user = userEvent.setup()
    const nearMatches = [
      'prefix Refind score >= 3.2',
      'Refind score >= 3.2 trailing',
      'prefix Visits were grouped into one session because adjacent gaps stayed within 30 minutes.',
      'Visits were grouped into one session because adjacent gaps stayed within 30 minutes. trailing',
      'Visits were grouped into one session because adjacent gaps stayed within thirty minutes.',
      "prefix Search trail anchored by 'tauri v2' and extended through navigation ancestry within the session window.",
      "Search trail anchored by 'tauri v2' and extended through navigation ancestry within the session window. trailing",
      "prefix Queries were merged into one family because their Jaccard or containment similarity matched 'pathkeep'.",
      "Queries were merged into one family because their Jaccard or containment similarity matched 'pathkeep'. trailing",
      'weekly_habit cadence was detected and later crossed its interruption threshold. trailing',
      'daily_habit cadence was detected from repeated cross-day visits. trailing',
    ]

    for (const rule of nearMatches) {
      const { unmount } = render(
        <ExplainabilityPanel
          entityType="entity"
          entityId={rule}
          explanation={explanationFixture({ triggerRule: rule })}
          t={echoT}
        />,
      )

      await user.click(screen.getByRole('button', { name: 'explainTitle' }))
      expect(screen.getByText(rule)).toBeVisible()
      unmount()
    }
  })
})

function explanationFixture(overrides: Partial<Explanation> = {}): Explanation {
  return {
    entityType: 'refind_page',
    entityId: 'entity-1',
    triggerRule: 'Refind score >= 1.0',
    factors: [],
    participatingVisitIds: [],
    ...overrides,
  }
}
