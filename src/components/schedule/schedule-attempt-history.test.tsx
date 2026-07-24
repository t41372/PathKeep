import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import type { ScheduledBackupAttempt } from '../../lib/types'
import { ScheduleAttemptHistory } from './schedule-attempt-history'

const copy: Record<string, string> = {
  'schedule.attemptHistoryBody': 'Every native wake is recorded here.',
  'schedule.attemptHistoryCount': '{count} attempts',
  'schedule.attemptHistoryEmpty': 'No native wake has been recorded yet.',
  'schedule.attemptHistoryTitle': 'RECENT SCHEDULE ATTEMPTS',
  'schedule.attemptOutcome.failed': 'Failed',
  'schedule.attemptOutcome.skipped': 'Not due',
  'schedule.attemptRunId': 'Run #{id}',
}

function t(key: string, vars: Record<string, string | number> = {}) {
  return Object.entries(vars).reduce(
    (value, [name, replacement]) =>
      value.replace(`{${name}}`, String(replacement)),
    copy[key] ?? key,
  )
}

describe('ScheduleAttemptHistory', () => {
  test('distinguishes failed and skipped native wakes with durable evidence', () => {
    const attempts: ScheduledBackupAttempt[] = [
      {
        id: 'attempt-2',
        startedAt: '2026-07-23T12:00:00Z',
        finishedAt: '2026-07-23T12:00:01Z',
        outcome: 'failed',
        phase: 'keyring',
        reasonCode: 'keyring-unavailable',
        detail: 'The archive key could not be read.',
        runId: null,
      },
      {
        id: 'attempt-1',
        startedAt: '2026-07-23T11:00:00Z',
        finishedAt: '2026-07-23T11:00:00Z',
        outcome: 'skipped',
        phase: 'due-check',
        reasonCode: 'not-due',
        detail: null,
        runId: 84,
      },
      {
        id: 'attempt-running',
        startedAt: 'invalid-worker-timestamp',
        finishedAt: null,
        outcome: 'running',
        phase: 'archive-open',
        reasonCode: null,
        detail: null,
        runId: null,
      },
    ]

    render(<ScheduleAttemptHistory attempts={attempts} language="en" t={t} />)

    expect(
      screen.getByRole('heading', {
        name: 'RECENT SCHEDULE ATTEMPTS',
      }),
    ).toBeVisible()
    expect(screen.getByText('Failed')).toBeVisible()
    expect(screen.getByText('keyring-unavailable')).toBeVisible()
    expect(screen.getByText('The archive key could not be read.')).toBeVisible()
    expect(screen.getByText('Not due')).toBeVisible()
    expect(screen.getByText('Run #84')).toBeVisible()
    expect(screen.getByText('invalid-worker-timestamp')).toBeVisible()
  })

  test('renders an honest empty state', () => {
    render(<ScheduleAttemptHistory attempts={[]} language="zh-TW" t={t} />)

    expect(
      screen.getByText('No native wake has been recorded yet.'),
    ).toBeVisible()
  })
})
