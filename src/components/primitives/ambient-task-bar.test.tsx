/**
 * Tests for the ambient bottom-bar strip — the shell-level "something is running" indicator.
 *
 * Why this file exists:
 * - AmbientTaskBar is presentational (it receives an already-selected + localized model) but owns
 *   its own determinate/indeterminate rendering, the single-vs-summary layout, and the click-through
 *   affordance. Those branches are pinned here independently of the shell that mounts it.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import type { AmbientTask, AmbientTasksModel } from '@/app/shell-ambient-tasks'
import { AmbientTaskBar } from './ambient-task-bar'

function model(tasks: AmbientTask[]): AmbientTasksModel {
  return { count: tasks.length, primary: tasks[0] ?? null, tasks }
}

function baseProps() {
  return {
    onOpenActivity: vi.fn(),
    summaryLabel: '2 tasks running',
    viewActivityLabel: 'View background activity',
  }
}

describe('AmbientTaskBar', () => {
  test('a single determinate task shows its label and percentage', () => {
    render(
      <AmbientTaskBar
        model={model([
          {
            id: 't1',
            label: 'Importing history',
            progressValue: 45,
            progressLabel: null,
          },
        ])}
        {...baseProps()}
      />,
    )
    expect(screen.getByText('Importing history')).toBeVisible()
    expect(screen.getByText('45%')).toBeVisible()
  })

  test('a single indeterminate task shows no percentage and an indeterminate bar', () => {
    const { container } = render(
      <AmbientTaskBar
        model={model([
          {
            id: 't1',
            label: 'Backing up',
            progressValue: null,
            progressLabel: null,
          },
        ])}
        {...baseProps()}
      />,
    )
    expect(container.textContent).not.toMatch(/%/)
    expect(container.querySelector('.pk-indeterminate-bar')).not.toBeNull()
  })

  test('multiple tasks show the summary label plus the primary task label', () => {
    render(
      <AmbientTaskBar
        model={{
          count: 2,
          primary: {
            id: 't1',
            label: 'Manual backup',
            progressValue: null,
            progressLabel: null,
          },
          tasks: [
            {
              id: 't1',
              label: 'Manual backup',
              progressValue: null,
              progressLabel: null,
            },
            {
              id: 't2',
              label: 'Building smart-search index',
              progressValue: null,
              progressLabel: null,
            },
          ],
        }}
        {...baseProps()}
      />,
    )
    expect(screen.getByText('2 tasks running')).toBeVisible()
    expect(screen.getByText('Manual backup')).toBeVisible()
  })

  test('renders the progress label when present', () => {
    render(
      <AmbientTaskBar
        model={model([
          {
            id: 't1',
            label: 'Importing history',
            progressValue: 45,
            progressLabel: '450 / 1000',
          },
        ])}
        {...baseProps()}
      />,
    )
    expect(screen.getByText('450 / 1000')).toBeVisible()
  })

  test('clicking the bar calls onOpenActivity', async () => {
    const user = userEvent.setup()
    const props = baseProps()
    render(
      <AmbientTaskBar
        model={model([
          {
            id: 't1',
            label: 'Importing history',
            progressValue: null,
            progressLabel: null,
          },
        ])}
        {...props}
      />,
    )
    await user.click(screen.getByTestId('ambient-task-bar'))
    expect(props.onOpenActivity).toHaveBeenCalledTimes(1)
  })

  test('renders nothing when there is no primary task', () => {
    const { container } = render(
      <AmbientTaskBar model={model([])} {...baseProps()} />,
    )
    expect(container.firstChild).toBeNull()
    expect(screen.queryByTestId('ambient-task-bar')).not.toBeInTheDocument()
  })
})
