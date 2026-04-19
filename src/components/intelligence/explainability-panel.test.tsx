/**
 * Verifies that explainability copy is localized instead of leaking backend rule strings verbatim.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test } from 'vitest'
import { ExplainabilityPanel } from './explainability-panel'
import { createNamespaceTranslator } from '../../lib/i18n'

const intelligenceT = createNamespaceTranslator('zh-CN', 'intelligence')

describe('ExplainabilityPanel', () => {
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
})
