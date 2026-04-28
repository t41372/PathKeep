/**
 * Tests for the shared Core Intelligence time-range selector.
 *
 * Why this file exists:
 * - The selector is now reused by the main intelligence route and the domain deep-dive route.
 * - These assertions keep the range contract stable without forcing route-level tests to cover every interaction detail.
 */

import { useState } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { DateRange, TimeRangePreset } from '../../lib/core-intelligence'
import { TimeRangeSelector } from './time-range-selector'

describe('TimeRangeSelector', () => {
  test('switches presets and applies custom ranges', async () => {
    const user = userEvent.setup()
    const t = createNamespaceTranslator('en', 'intelligence')
    const onPresetChange = vi.fn()
    const onCustomRange = vi.fn()

    function Harness() {
      const [preset, setPreset] = useState<TimeRangePreset>('month')
      const [dateRange, setDateRange] = useState<DateRange>({
        start: '2026-04-01',
        end: '2026-04-07',
      })

      return (
        <TimeRangeSelector
          dateRange={dateRange}
          preset={preset}
          onPresetChange={(nextPreset) => {
            onPresetChange(nextPreset)
            setPreset(nextPreset)
          }}
          onCustomRange={(nextRange) => {
            onCustomRange(nextRange)
            setDateRange(nextRange)
          }}
          t={t}
        />
      )
    }

    render(<Harness />)

    await user.click(screen.getByRole('button', { name: t('rangeWeek') }))
    expect(onPresetChange).toHaveBeenCalledWith('week')

    await user.click(screen.getByRole('button', { name: t('rangeAll') }))
    expect(onPresetChange).toHaveBeenCalledWith('all')

    await user.click(screen.getByRole('button', { name: t('rangeCustom') }))
    expect(onPresetChange).toHaveBeenCalledWith('custom')

    await user.clear(screen.getByLabelText(t('customStart')))
    await user.type(screen.getByLabelText(t('customStart')), '2026-03-01')
    await user.clear(screen.getByLabelText(t('customEnd')))
    await user.type(screen.getByLabelText(t('customEnd')), '2026-03-31')
    await user.click(screen.getByRole('button', { name: t('applyRange') }))

    expect(onCustomRange).toHaveBeenCalledWith({
      start: '2026-03-01',
      end: '2026-03-31',
    })
  })

  test.each(['en', 'zh-CN', 'zh-TW'] as const)(
    'renders the all-time preset label in %s',
    (language) => {
      const t = createNamespaceTranslator(language, 'intelligence')

      render(
        <TimeRangeSelector
          dateRange={{ start: '2026-04-01', end: '2026-04-30' }}
          preset="month"
          onPresetChange={vi.fn()}
          onCustomRange={vi.fn()}
          t={t}
        />,
      )

      expect(
        screen.getByRole('button', { name: t('rangeAll') }),
      ).toBeInTheDocument()
    },
  )
})
