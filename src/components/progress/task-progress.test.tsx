/**
 * @file task-progress.test.tsx
 * @description Render coverage for shared task progress UI primitives.
 * @module components/progress
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import type { ShellTask } from '../../app/shell-tasks'
import { ProgressMeter, TaskConsole, TaskProgressCard } from './task-progress'

describe('task progress components', () => {
  test('renders determinate, indeterminate, compact, and empty progress states', () => {
    const empty = render(<ProgressMeter value={null} label={null} />)
    expect(empty.container.firstChild).toBeNull()
    empty.unmount()

    const determinate = render(
      <ProgressMeter value={142.2} label="3 / 4 records" />,
    )
    expect(screen.getByText('3 / 4 records')).toBeVisible()
    expect(screen.getByText('100%')).toBeVisible()
    expect(
      determinate.container.querySelector('.task-progress-meter__fill'),
    ).toHaveStyle({ width: '100%' })
    determinate.unmount()

    const indeterminate = render(<ProgressMeter compact label="Scanning" />)
    expect(screen.getByText('Scanning')).toBeVisible()
    expect(
      indeterminate.container.querySelector(
        '.task-progress-meter__track--indeterminate',
      ),
    ).toBeInTheDocument()
    indeterminate.unmount()

    const unlabeled = render(<ProgressMeter value={33} label={null} />)
    expect(screen.getByText('33%')).toBeVisible()
    expect(
      unlabeled.container.querySelector('.task-progress-meter__meta span'),
    ).toHaveTextContent('')
  })

  test('renders task console timestamps, source labels, severity, and empty state', () => {
    const empty = render(
      <TaskConsole entries={[]} emptyLabel="No logs yet" language="en" />,
    )
    expect(screen.getByText('No logs yet')).toBeVisible()
    empty.unmount()

    render(
      <TaskConsole
        compact
        language="en"
        emptyLabel="No logs yet"
        entries={[
          {
            id: 'entry-1',
            timestamp: '2026-04-27T10:00:00.000Z',
            level: 'success',
            code: 'import.complete',
            sourceLabel: 'Chrome Default',
            message: 'Import complete',
          },
        ]}
      />,
    )

    expect(screen.getByRole('log')).toHaveClass('task-console--compact')
    expect(screen.getByText('success')).toBeVisible()
    expect(screen.getByText('[Chrome Default] Import complete')).toBeVisible()
  })

  test('renders full task cards with metadata, records, console, and actions', () => {
    render(
      <TaskProgressCard
        task={taskFixture()}
        language="en"
        labels={{
          started: 'Started',
          updated: 'Updated',
          records: 'records',
          console: 'Console',
          noLogs: 'No logs yet',
        }}
        actions={<button type="button">Open result</button>}
      />,
    )

    expect(screen.getByText('import · running')).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Import Chrome' })).toBeVisible()
    expect(
      screen.getByText('[Chrome Default] Writing archive records'),
    ).toBeVisible()
    expect(screen.getAllByText('3 / 12 records')).toHaveLength(2)
    expect(screen.getByText('Console')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open result' })).toBeVisible()
  })

  test('renders compact cards with partial record counts and progress-label fallbacks', () => {
    const partial = render(
      <TaskProgressCard
        compact
        task={{
          ...taskFixture(),
          id: 'task-partial',
          processedRecords: 5,
          totalRecords: null,
          progressLabel: null,
          progressValue: null,
          logEntries: [],
        }}
        language="en"
        labels={{
          started: 'Started',
          updated: 'Updated',
          records: 'records',
          console: 'Console',
          noLogs: 'No logs yet',
        }}
      />,
    )

    expect(partial.container.firstChild).toHaveClass(
      'task-progress-card--compact',
    )
    expect(screen.getAllByText('5 records')).toHaveLength(2)
    partial.unmount()

    render(
      <TaskProgressCard
        task={{
          ...taskFixture(),
          id: 'task-progress-label',
          processedRecords: null,
          totalRecords: null,
          progressLabel: 'Waiting for parser',
          progressValue: null,
          logEntries: [],
        }}
        language="en"
        labels={{
          started: 'Started',
          updated: 'Updated',
          records: 'records',
          console: 'Console',
          noLogs: 'No logs yet',
        }}
      />,
    )

    expect(screen.getAllByText('Waiting for parser')).toHaveLength(2)

    const noSummary = render(
      <TaskProgressCard
        task={{
          ...taskFixture(),
          id: 'task-no-summary',
          processedRecords: null,
          totalRecords: null,
          progressLabel: null,
          progressValue: 15,
          logEntries: [],
        }}
        language="en"
        labels={{
          started: 'Started',
          updated: 'Updated',
          records: 'records',
          console: 'Console',
          noLogs: 'No logs yet',
        }}
      />,
    )
    expect(
      noSummary.container.querySelector('.task-progress-meter'),
    ).toBeInTheDocument()
  })
})

function taskFixture(): ShellTask {
  return {
    id: 'task-import',
    kind: 'import',
    state: 'running',
    title: 'Import Chrome',
    detail: 'Writing archive records',
    startedAt: '2026-04-27T10:00:00.000Z',
    updatedAt: '2026-04-27T10:01:00.000Z',
    finishedAt: null,
    sourceLabel: 'Chrome Default',
    profileLabel: 'Default',
    progressLabel: '3 / 12',
    progressValue: 25,
    current: 3,
    total: 12,
    processedRecords: 3,
    totalRecords: 12,
    importedRecords: 2,
    duplicateRecords: 1,
    skippedRecords: 0,
    logEntries: [
      {
        id: 'log-1',
        timestamp: '2026-04-27T10:01:00.000Z',
        level: 'info',
        code: 'import.records',
        sourceLabel: 'Chrome Default',
        message: 'Writing archive records',
      },
    ],
    resultLink: null,
    error: null,
  }
}
