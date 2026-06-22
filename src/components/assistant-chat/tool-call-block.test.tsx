/**
 * @file tool-call-block.test.tsx
 * @description Coverage for the inline, live tool-use timeline.
 *
 * Proves: renders nothing with no calls; renders each call as a numbered step with the resolved
 * "Ran {name}" label and its arguments in a mono block; omits the args block when arguments are
 * empty; shows a live "Running…" badge while pending; an honest "Done"/"Failed" word + collapsible
 * result once the result lands; the error result defaults OPEN (a failure is never hidden); the
 * `statusOf` fallback for a call missing an explicit status.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { ToolCallBlock, type ToolCallBlockCopy } from './tool-call-block'

const copy: ToolCallBlockCopy = {
  label: 'Tools used',
  ranTemplate: 'Ran {name}',
  runningLabel: 'Running…',
  doneLabel: 'Done',
  failedLabel: 'Failed',
  resultToggleLabel: 'Toggle tool result',
}

describe('ToolCallBlock', () => {
  test('renders nothing when there are no tool calls', () => {
    const { container } = render(<ToolCallBlock calls={[]} copy={copy} />)
    expect(container.firstChild).toBeNull()
  })

  test('renders each call as a numbered step with name and arguments', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{"q":"rust"}',
            status: 'pending',
          },
          {
            id: 't2',
            name: 'fetch_visits',
            arguments: '{"ids":[1,2]}',
            status: 'pending',
          },
        ]}
      />,
    )
    expect(screen.getByText('Tools used')).toBeVisible()
    expect(screen.getByText('Ran search_bm25')).toBeVisible()
    expect(screen.getByText('Ran fetch_visits')).toBeVisible()
    expect(screen.getByTestId('tools-args-0')).toHaveTextContent('{"q":"rust"}')
    expect(screen.getByTestId('tools-args-1')).toHaveTextContent(
      '{"ids":[1,2]}',
    )
    expect(screen.getByTestId('tools-step-0')).toBeVisible()
    expect(screen.getByTestId('tools-step-1')).toBeVisible()
    // The visible ordinal is 1-based: a mutant `index + 1` → `index` (rendering "0.") must fail.
    expect(screen.getByTestId('tools-step-0')).toHaveTextContent('1.')
    expect(screen.getByTestId('tools-step-1')).toHaveTextContent('2.')
    expect(screen.getByTestId('tools-step-0')).not.toHaveTextContent('0.')
  })

  test('renders without a testId', () => {
    render(
      <ToolCallBlock
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'query_visits',
            arguments: '{"limit":5}',
            status: 'pending',
          },
        ]}
      />,
    )
    expect(screen.getByText('Ran query_visits')).toBeVisible()
    expect(screen.getByText('{"limit":5}')).toBeVisible()
  })

  test('omits the arguments block when a call has no arguments', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          { id: 't1', name: 'list_domains', arguments: '', status: 'pending' },
        ]}
      />,
    )
    expect(screen.getByText('Ran list_domains')).toBeVisible()
    expect(screen.queryByTestId('tools-args-0')).not.toBeInTheDocument()
  })

  test('shows a live Running badge while a call is pending (no result yet)', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            status: 'pending',
          },
        ]}
      />,
    )
    const status = screen.getByTestId('tools-step-0-status')
    expect(status).toHaveTextContent('Running…')
    expect(status).toHaveAttribute('aria-label', 'Running…')
    // No result body yet while pending.
    expect(
      screen.queryByTestId('tools-result-0-toggle'),
    ).not.toBeInTheDocument()
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('pending')
  })

  test('a successful call shows Done and a collapsed result that toggles open', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{"q":"rust"}',
            callId: 'call-1',
            status: 'success',
            isError: false,
            result: 'search_bm25: 3 match(es).',
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent('Done')
    // Success result is collapsed by default — the body is hidden until toggled.
    expect(screen.queryByTestId('tools-result-0-body')).not.toBeInTheDocument()
    fireEvent.click(screen.getByTestId('tools-result-0-toggle'))
    expect(screen.getByTestId('tools-result-0-body')).toHaveTextContent(
      'search_bm25: 3 match(es).',
    )
    // Toggling again collapses it.
    fireEvent.click(screen.getByTestId('tools-result-0-toggle'))
    expect(screen.queryByTestId('tools-result-0-body')).not.toBeInTheDocument()
  })

  test('a failed call shows Failed and an OPEN error result (a failure is never hidden)', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'no_such_tool',
            arguments: '{}',
            callId: 'call-x',
            status: 'error',
            isError: true,
            result: 'Tool `no_such_tool` failed: unknown tool.',
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent(
      'Failed',
    )
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('error')
    // The error body is open by default so the user sees the honest failure immediately.
    expect(screen.getByTestId('tools-result-0-body')).toHaveTextContent(
      'Tool `no_such_tool` failed: unknown tool.',
    )
  })

  test('falls back to deriving status from result/isError when no explicit status is set', () => {
    // A call with a result but no `status` (defensive path): isError true → error word + open body.
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            result: 'boom',
            isError: true,
          },
          // No status and no result → treated as pending.
          { id: 't2', name: 'query_visits', arguments: '{}' },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent(
      'Failed',
    )
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('error')
    expect(screen.getByTestId('tools-step-1-status')).toHaveTextContent(
      'Running…',
    )
    expect(screen.getByTestId('tools-step-1').dataset.status).toBe('pending')
  })

  test('derives success when a result is present without an error flag or status', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          { id: 't1', name: 'search_bm25', arguments: '{}', result: 'ok rows' },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent('Done')
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('success')
  })

  test('renders a result without a testId (toggle has no test id but still works)', () => {
    render(
      <ToolCallBlock
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            status: 'success',
            isError: false,
            result: 'ok',
          },
        ]}
      />,
    )
    // The toggle is reachable by its aria-label even with no test id.
    fireEvent.click(screen.getByLabelText('Toggle tool result'))
    expect(screen.getByText('ok')).toBeVisible()
  })
})
