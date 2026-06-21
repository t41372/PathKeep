/**
 * @file reasoning-block.test.tsx
 * @description Coverage for the collapsible reasoning ("thinking") panel.
 *
 * Proves: renders nothing when empty; auto-expanded + live-pulse while streaming; auto-collapsed
 * when done; user can toggle open/closed; the thinking vs thought labels switch on the streaming
 * flag; the body shows the accumulated reasoning text verbatim.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { ReasoningBlock, type ReasoningBlockCopy } from './reasoning-block'

const copy: ReasoningBlockCopy = {
  thinkingLabel: 'Thinking…',
  thoughtLabel: 'Thought process',
  toggleLabel: 'Toggle the thought process',
}

describe('ReasoningBlock', () => {
  test('renders nothing when there is no reasoning text', () => {
    const { container } = render(
      <ReasoningBlock text="" streaming copy={copy} testId="reasoning" />,
    )
    expect(container.firstChild).toBeNull()
  })

  test('is auto-expanded with the thinking label while streaming', () => {
    render(
      <ReasoningBlock
        text="step one\nstep two"
        streaming
        copy={copy}
        testId="reasoning"
      />,
    )
    expect(screen.getByTestId('reasoning')).toHaveAttribute(
      'data-streaming',
      'true',
    )
    expect(screen.getByTestId('reasoning')).toHaveAttribute('data-open', 'true')
    expect(screen.getByText('Thinking…')).toBeVisible()
    expect(screen.getByTestId('reasoning-body')).toHaveTextContent('step one')
    expect(
      screen.getByRole('button', { name: copy.toggleLabel }),
    ).toHaveAttribute('aria-expanded', 'true')
  })

  test('is auto-collapsed with the thought label once finished', () => {
    render(
      <ReasoningBlock
        text="done thinking"
        streaming={false}
        copy={copy}
        testId="reasoning"
      />,
    )
    expect(screen.getByTestId('reasoning')).toHaveAttribute(
      'data-open',
      'false',
    )
    expect(screen.getByText('Thought process')).toBeVisible()
    expect(screen.queryByTestId('reasoning-body')).not.toBeInTheDocument()
  })

  test('renders without a testId (no test ids emitted)', () => {
    render(<ReasoningBlock text="anon reasoning" streaming copy={copy} />)
    expect(screen.getByText('Thinking…')).toBeVisible()
    expect(screen.getByText('anon reasoning')).toBeVisible()
    expect(screen.getByRole('button', { name: copy.toggleLabel })).toBeVisible()
  })

  test('pins the body to its bottom while reasoning streams', () => {
    const { rerender } = render(
      <ReasoningBlock
        text="line one"
        streaming
        copy={copy}
        testId="reasoning"
      />,
    )
    const body = screen.getByTestId('reasoning-body')
    Object.defineProperty(body, 'scrollHeight', {
      value: 800,
      configurable: true,
    })
    body.scrollTop = 0
    rerender(
      <ReasoningBlock
        text="line one\nline two\nline three"
        streaming
        copy={copy}
        testId="reasoning"
      />,
    )
    expect(body.scrollTop).toBe(800)
  })

  test('does not pin the body once streaming has finished', () => {
    const { rerender } = render(
      <ReasoningBlock
        text="thinking"
        streaming
        copy={copy}
        testId="reasoning"
      />,
    )
    // User re-opens the finished panel; scroll position must be left alone.
    rerender(
      <ReasoningBlock
        text="thinking"
        streaming={false}
        copy={copy}
        testId="reasoning"
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: copy.toggleLabel }))
    const body = screen.getByTestId('reasoning-body')
    Object.defineProperty(body, 'scrollHeight', {
      value: 800,
      configurable: true,
    })
    body.scrollTop = 10
    // Re-render with the same text; the pin effect is gated on `streaming` so nothing moves.
    rerender(
      <ReasoningBlock
        text="thinking"
        streaming={false}
        copy={copy}
        testId="reasoning"
      />,
    )
    expect(body.scrollTop).toBe(10)
  })

  test('lets the user expand a finished panel and collapse it again', () => {
    render(
      <ReasoningBlock
        text="hidden detail"
        streaming={false}
        copy={copy}
        testId="reasoning"
      />,
    )
    const toggle = screen.getByRole('button', { name: copy.toggleLabel })
    fireEvent.click(toggle)
    expect(screen.getByTestId('reasoning-body')).toHaveTextContent(
      'hidden detail',
    )
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    fireEvent.click(toggle)
    expect(screen.queryByTestId('reasoning-body')).not.toBeInTheDocument()
  })
})
