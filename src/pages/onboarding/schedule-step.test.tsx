/**
 * @file schedule-step.test.tsx
 * @description Presentational coverage for the onboarding schedule step.
 * @module pages/onboarding
 *
 * ## Responsibilities
 * - Verify interval chip selections are forwarded to the route owner.
 * - Keep schedule preview rendering covered outside the full onboarding flow.
 *
 * ## Not responsible for
 * - Re-testing schedule preview command behavior.
 * - Re-testing the full onboarding route state machine.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider because the component reads onboarding copy directly.
 *
 * ## Performance notes
 * - Pure render and click coverage only.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { ScheduleStep } from './schedule-step'

describe('ScheduleStep', () => {
  test('forwards interval chip selections', async () => {
    const user = userEvent.setup()
    const onSelectDueAfterHours = vi.fn()

    render(
      <I18nProvider>
        <ScheduleStep
          dueAfterHours={24}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSelectDueAfterHours={onSelectDueAfterHours}
          schedulePlan={null}
          schedulePreviewError={null}
          schedulePreviewLoading={false}
        />
      </I18nProvider>,
    )

    const twelveHourChip = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('12'))
    expect(twelveHourChip).toBeInstanceOf(HTMLButtonElement)
    await user.click(twelveHourChip as HTMLButtonElement)

    expect(onSelectDueAfterHours).toHaveBeenCalledWith(12)
  })

  test('renders schedule preview loading and error states', () => {
    const { rerender } = render(
      <I18nProvider>
        <ScheduleStep
          dueAfterHours={24}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          schedulePlan={null}
          schedulePreviewError={null}
          schedulePreviewLoading
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Generating schedule preview…')).toBeVisible()

    rerender(
      <I18nProvider>
        <ScheduleStep
          dueAfterHours={24}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSelectDueAfterHours={vi.fn()}
          schedulePlan={null}
          schedulePreviewError="preview failed"
          schedulePreviewLoading={false}
        />
      </I18nProvider>,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('preview failed')
  })
})
