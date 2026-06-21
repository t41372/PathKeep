/**
 * @file streaming-markdown.test.tsx
 * @description Coverage for the streamdown wrapper.
 *
 * `streamdown` is mocked at the module boundary so the test asserts OUR contract: the wrapper
 * passes the accumulated content through, sets `mode`/`parseIncompleteMarkdown`/`controls`
 * sensibly, applies the paper `assistant-prose` skin, and reflects the streaming flag. (The real
 * streamdown is exercised in the live visual QA pass and the production build, not unit tests.)
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'

const streamdownSpy = vi.fn()
vi.mock('streamdown', () => ({
  Streamdown: (props: Record<string, unknown>) => {
    streamdownSpy(props)
    return <div data-testid="streamdown-stub">{String(props.children)}</div>
  },
}))

import { StreamingMarkdown } from './streaming-markdown'

describe('StreamingMarkdown', () => {
  test('renders the content and marks the streaming state', () => {
    render(
      <StreamingMarkdown
        content={'# Title\n\n```js\nconst x ='}
        streaming
        testId="answer"
      />,
    )
    const wrapper = screen.getByTestId('answer')
    expect(wrapper).toHaveClass('assistant-prose')
    expect(wrapper).toHaveAttribute('data-streaming', 'true')
    expect(screen.getByTestId('streamdown-stub')).toHaveTextContent('# Title')

    const props = streamdownSpy.mock.calls.at(-1)?.[0]
    expect(props?.mode).toBe('streaming')
    expect(props?.parseIncompleteMarkdown).toBe(true)
    expect(props?.controls).toBe(false)
    expect(props?.children).toBe('# Title\n\n```js\nconst x =')
  })

  test('uses static mode when not streaming', () => {
    render(<StreamingMarkdown content="final answer" testId="answer" />)
    expect(screen.getByTestId('answer')).toHaveAttribute(
      'data-streaming',
      'false',
    )
    const props = streamdownSpy.mock.calls.at(-1)?.[0]
    expect(props?.mode).toBe('static')
    // No caret unless explicitly requested.
    expect(screen.queryByTestId('answer-caret')).not.toBeInTheDocument()
  })

  test('renders the blinking caret when showCaret is set', () => {
    render(<StreamingMarkdown content="streaming…" showCaret testId="answer" />)
    const caret = screen.getByTestId('answer-caret')
    expect(caret).toBeInTheDocument()
    expect(caret).toHaveAttribute('aria-hidden', 'true')
  })

  test('omits the caret testId when no testId is supplied', () => {
    const { container } = render(
      <StreamingMarkdown content="streaming…" showCaret />,
    )
    // The caret still renders (aria-hidden span) but carries no testId.
    expect(container.querySelector('.pk-stream-caret')).not.toBeNull()
  })
})
