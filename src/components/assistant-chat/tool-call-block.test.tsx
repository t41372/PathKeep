/**
 * @file tool-call-block.test.tsx
 * @description Coverage for the visible tool-use timeline.
 *
 * Proves: renders nothing with no calls; renders each call as a numbered step with the resolved
 * "Ran {name}" label and its arguments in a mono block; omits the args block when arguments are
 * empty (transparency without empty noise).
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { ToolCallBlock, type ToolCallBlockCopy } from './tool-call-block'

const copy: ToolCallBlockCopy = {
  label: 'Tools used',
  ranTemplate: 'Ran {name}',
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
          { id: 't1', name: 'search_bm25', arguments: '{"q":"rust"}' },
          { id: 't2', name: 'fetch_visits', arguments: '{"ids":[1,2]}' },
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
        calls={[{ id: 't1', name: 'query_visits', arguments: '{"limit":5}' }]}
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
        calls={[{ id: 't1', name: 'list_domains', arguments: '' }]}
      />,
    )
    expect(screen.getByText('Ran list_domains')).toBeVisible()
    expect(screen.queryByTestId('tools-args-0')).not.toBeInTheDocument()
  })
})
