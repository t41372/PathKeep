/**
 * @file assistant-chat-view.test.tsx
 * @description Coverage for the composed chat surface (greeting, list, composer).
 *
 * Proves: empty state shows greeting + prompt cards (and picking one fires onPickPrompt); the
 * composer sends on click and on Enter, inserts a newline on Shift+Enter, is disabled with no
 * provider or while streaming, swaps the send button for a cancel button while streaming, and
 * surfaces the provider attribution; the list renders one row per message; evidence selection
 * and the virtualization-disable path are wired.
 */

import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import type { ChatMessage } from './use-ai-chat-stream'

/**
 * Controllable IntersectionObserver mock (mirrors paper-contact-sheet.virt.test.tsx) so the
 * virtualization branches can be driven deterministically in jsdom.
 */
function installObserverMock() {
  type Callback = (entries: IntersectionObserverEntry[]) => void
  const subscribers = new Map<Element, Callback>()
  function MockIO(this: Record<string, unknown>, callback: Callback) {
    this.observe = (node: Element) => subscribers.set(node, callback)
    this.unobserve = (node: Element) => subscribers.delete(node)
    this.disconnect = () => {
      for (const node of [...subscribers.keys()]) {
        if (subscribers.get(node) === callback) subscribers.delete(node)
      }
    }
    this.takeRecords = () => []
  }
  const previous = globalThis.IntersectionObserver
  ;(
    globalThis as { IntersectionObserver: typeof IntersectionObserver }
  ).IntersectionObserver = MockIO as unknown as typeof IntersectionObserver
  return {
    trigger(node: Element, isIntersecting: boolean) {
      subscribers.get(node)?.([
        {
          isIntersecting,
          target: node,
          boundingClientRect: node.getBoundingClientRect(),
          intersectionRatio: isIntersecting ? 1 : 0,
          intersectionRect: node.getBoundingClientRect(),
          rootBounds: null,
          time: 0,
        } as IntersectionObserverEntry,
      ])
    },
    restore() {
      ;(
        globalThis as { IntersectionObserver: typeof IntersectionObserver }
      ).IntersectionObserver = previous
    },
  }
}

vi.mock('streamdown', () => ({
  Streamdown: (props: { children?: unknown }) => (
    <div data-testid="streamdown-stub">{String(props.children)}</div>
  ),
}))

import {
  AssistantChatView,
  type AssistantChatViewCopy,
  type AssistantChatViewProps,
} from './assistant-chat-view'

const copy: AssistantChatViewCopy = {
  greetingTitle: 'Ask anything',
  greetingSubtitle: 'Grounded in your archive',
  turn: {
    assistantByline: 'Local · model',
    userByline: 'You',
    typingLabel: 'Thinking…',
    evidenceLabel: 'Sources · {count} records',
    errorGeneric: "The assistant couldn't finish this answer.",
    stoppedLabel: 'Generation stopped',
    retryLabel: 'Try again',
    copyLabel: 'Copy answer',
    copiedLabel: 'Copied',
    regenerateLabel: 'Regenerate this answer',
    noAnswerLabel: 'No answer was returned.',
    statusUsingTool: 'Using tool: {name}',
    statusAnswering: 'Answering…',
    statusComplete: 'Answer complete',
    usageLabel: '{prompt} prompt · {completion} completion tokens',
    evidenceStar: {
      starLabel: 'Star this source',
      unstarLabel: 'Unstar this source',
      status: { starred: 'Source starred', unstarred: 'Source unstarred' },
    },
    reasoning: {
      thinkingLabel: 'Thinking…',
      thoughtLabel: 'Thought process',
      toggleLabel: 'Toggle',
    },
    toolCalls: {
      label: 'Tools used',
      ranTemplate: 'Ran {name}',
      runningLabel: 'Running…',
      doneLabel: 'Done',
      failedLabel: 'Failed',
      resultToggleLabel: 'Toggle tool result',
    },
  },
  composer: {
    placeholder: 'Ask about your archive…',
    sendLabel: 'Send message',
    cancelLabel: 'Stop generating',
    attribution: 'Local · keyword only',
    keyHint: '↵ send · ⇧↵ newline',
    connectingLabel: 'Connecting to Local LLM…',
    scopeNote: 'Searches your whole archive',
  },
}

const prompts = [
  { id: 'p1', text: 'First prompt' },
  { id: 'p2', text: 'Second prompt' },
]

function baseProps() {
  return {
    messages: [] as ChatMessage[],
    input: '',
    streaming: false,
    awaitingFirstChunk: false,
    canSend: true,
    prompts,
    copy,
    onInputChange: vi.fn(),
    onSend: vi.fn(),
    onCancel: vi.fn(),
    onRetry: vi.fn(),
    onPickPrompt: vi.fn(),
    onSelectEvidence: vi.fn(),
    evidenceFor: undefined as AssistantChatViewProps['evidenceFor'],
    disableVirtualization: true,
    testId: 'chat',
  }
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('AssistantChatView', () => {
  test('forwards textarea changes to onInputChange', () => {
    const props = baseProps()
    render(<AssistantChatView {...props} />)
    fireEvent.change(screen.getByTestId('assistant-chat-input'), {
      target: { value: 'typed text' },
    })
    expect(props.onInputChange).toHaveBeenCalledWith('typed text')
  })

  test('renders the greeting and prompt cards when empty', () => {
    const props = baseProps()
    render(<AssistantChatView {...props} />)
    expect(screen.getByText('Ask anything')).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-assistant-prompt-p1'))
    expect(props.onPickPrompt).toHaveBeenCalledWith(prompts[0])
    expect(screen.getByTestId('assistant-chat-attribution')).toHaveTextContent(
      'Local · keyword only',
    )
    // C1-3: the whole-archive scope note rides the always-visible composer footer (persistent
    // scope honesty), not the empty-state greeting.
    expect(screen.getByTestId('assistant-chat-scope-note')).toHaveTextContent(
      'Searches your whole archive',
    )
  })

  test('keeps the scope note visible once a conversation has started (C1-3)', () => {
    const props = baseProps()
    // A non-empty transcript: the greeting is gone, but the persistent footer scope note remains.
    props.messages = [{ id: 'u1', role: 'user', content: 'a question' }]
    render(<AssistantChatView {...props} />)
    expect(screen.queryByText('Ask anything')).not.toBeInTheDocument()
    expect(screen.getByTestId('assistant-chat-scope-note')).toHaveTextContent(
      'Searches your whole archive',
    )
  })

  test('sends on the send button click when there is input', () => {
    const props = baseProps()
    props.input = 'a question'
    render(<AssistantChatView {...props} />)
    fireEvent.click(screen.getByTestId('assistant-chat-send'))
    expect(props.onSend).toHaveBeenCalledWith('a question')
  })

  test('sends on Enter and inserts a newline on Shift+Enter', () => {
    const props = baseProps()
    props.input = 'enter question'
    render(<AssistantChatView {...props} />)
    const input = screen.getByTestId('assistant-chat-input')
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
    expect(props.onSend).not.toHaveBeenCalled()
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(props.onSend).toHaveBeenCalledWith('enter question')
  })

  test('does not send blank input', () => {
    const props = baseProps()
    props.input = '   '
    render(<AssistantChatView {...props} />)
    fireEvent.submit(screen.getByTestId('assistant-chat-composer'))
    expect(props.onSend).not.toHaveBeenCalled()
  })

  test('disables the composer when no provider is configured', () => {
    const props = baseProps()
    props.input = 'blocked'
    props.canSend = false
    render(<AssistantChatView {...props} />)
    expect(screen.getByTestId('assistant-chat-input')).toBeDisabled()
    expect(screen.getByTestId('assistant-chat-send')).toBeDisabled()
    fireEvent.keyDown(screen.getByTestId('assistant-chat-input'), {
      key: 'Enter',
    })
    expect(props.onSend).not.toHaveBeenCalled()
  })

  test('shows a stop button while streaming and routes it to onCancel', () => {
    const props = baseProps()
    props.streaming = true
    props.input = 'queued next prompt'
    props.messages = [
      { id: 'u1', role: 'user', content: 'q' },
      { id: 'a1', role: 'assistant', content: 'partial', status: 'streaming' },
    ]
    render(<AssistantChatView {...props} />)
    expect(screen.queryByTestId('assistant-chat-send')).not.toBeInTheDocument()
    const cancel = screen.getByTestId('assistant-chat-cancel')
    fireEvent.click(cancel)
    expect(props.onCancel).toHaveBeenCalledTimes(1)
    // The textarea stays ENABLED while streaming (focus is never ripped to body); only the send
    // path is suppressed — Enter is a no-op mid-stream.
    const input = screen.getByTestId('assistant-chat-input')
    expect(input).toBeEnabled()
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(props.onSend).not.toHaveBeenCalled()
  })

  test('shows the connecting affordance while awaiting the first chunk', () => {
    const props = baseProps()
    props.streaming = true
    props.awaitingFirstChunk = true
    props.messages = [
      { id: 'u1', role: 'user', content: 'q' },
      { id: 'a1', role: 'assistant', content: '', status: 'streaming' },
    ]
    render(<AssistantChatView {...props} />)
    expect(screen.getByTestId('assistant-chat-connecting')).toHaveTextContent(
      'Connecting to Local LLM…',
    )
  })

  test('does not show the connecting affordance once the first chunk has arrived', () => {
    const props = baseProps()
    props.streaming = true
    props.awaitingFirstChunk = false
    props.messages = [
      { id: 'a1', role: 'assistant', content: 'partial', status: 'streaming' },
    ]
    render(<AssistantChatView {...props} />)
    expect(
      screen.queryByTestId('assistant-chat-connecting'),
    ).not.toBeInTheDocument()
  })

  test('fills and focuses the textarea with the caret at the end on prompt pick', () => {
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      cb(0)
      return 1
    })
    const props = baseProps()
    // Mirror the route: a pick sets the input. Re-render with the picked value so the focus
    // effect parks the caret at the resolved end.
    props.input = 'First prompt'
    render(<AssistantChatView {...props} />)
    const input = screen.getByTestId<HTMLTextAreaElement>(
      'assistant-chat-input',
    )
    fireEvent.click(screen.getByTestId('paper-assistant-prompt-p1'))
    expect(props.onPickPrompt).toHaveBeenCalledWith(prompts[0])
    expect(input).toHaveFocus()
    expect(input.selectionStart).toBe(input.value.length)
    expect(input.selectionEnd).toBe(input.value.length)
  })

  test('refocuses the textarea on the streaming → idle edge', () => {
    const rafCallbacks: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      rafCallbacks.push(cb)
      return rafCallbacks.length
    })
    const props = baseProps()
    props.streaming = true
    props.messages = [
      { id: 'a1', role: 'assistant', content: 'partial', status: 'streaming' },
    ]
    const { rerender } = render(<AssistantChatView {...props} />)
    // Stream finishes.
    rerender(
      <AssistantChatView
        {...props}
        streaming={false}
        messages={[
          { id: 'a1', role: 'assistant', content: 'done', status: 'done' },
        ]}
      />,
    )
    act(() => rafCallbacks.forEach((cb) => cb(0)))
    expect(screen.getByTestId('assistant-chat-input')).toHaveFocus()
  })

  test('renders one row per message and wires evidence selection', () => {
    const props = baseProps()
    props.messages = [
      { id: 'u1', role: 'user', content: 'question' },
      { id: 'a1', role: 'assistant', content: 'answer', status: 'done' },
    ]
    props.evidenceFor = (message: ChatMessage) =>
      message.id === 'a1'
        ? [
            {
              id: 'e1',
              date: '2026-04-05',
              title: 'Cited page',
              domain: 'example.com',
              url: 'https://example.com/a',
            },
          ]
        : undefined
    render(<AssistantChatView {...props} />)
    expect(screen.getByTestId('assistant-turn-u1')).toHaveTextContent(
      'question',
    )
    expect(screen.getByTestId('assistant-turn-a1')).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-assistant-evidence-e1'))
    expect(props.onSelectEvidence).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'e1' }),
    )
  })

  test('scrolls to the bottom as the streaming answer grows', () => {
    const props = baseProps()
    props.streaming = true
    props.messages = [
      { id: 'a1', role: 'assistant', content: 'tiny', status: 'streaming' },
    ]
    const { rerender } = render(<AssistantChatView {...props} />)
    const list = screen.getByTestId('assistant-chat-messages')
    // jsdom reports 0 dimensions; assert the scroll write path runs without throwing and the
    // follow heuristic (near-bottom) executes on content growth.
    Object.defineProperty(list, 'scrollHeight', {
      value: 500,
      configurable: true,
    })
    Object.defineProperty(list, 'clientHeight', {
      value: 400,
      configurable: true,
    })
    list.scrollTop = 90 // within 120px of bottom
    rerender(
      <AssistantChatView
        {...props}
        messages={[
          {
            id: 'a1',
            role: 'assistant',
            content: 'tiny but now much longer answer',
            status: 'streaming',
          },
        ]}
      />,
    )
    expect(list.scrollTop).toBe(list.scrollHeight)
  })

  test('does not yank the view down when the user has scrolled up while streaming', () => {
    const props = baseProps()
    props.streaming = true
    props.messages = [
      { id: 'a1', role: 'assistant', content: 'a', status: 'streaming' },
    ]
    const { rerender } = render(<AssistantChatView {...props} />)
    const list = screen.getByTestId('assistant-chat-messages')
    Object.defineProperty(list, 'scrollHeight', {
      value: 1000,
      configurable: true,
    })
    Object.defineProperty(list, 'clientHeight', {
      value: 400,
      configurable: true,
    })
    list.scrollTop = 100 // far from bottom (distance 500 > 120)
    rerender(
      <AssistantChatView
        {...props}
        messages={[
          {
            id: 'a1',
            role: 'assistant',
            content: 'a much longer answer now',
            status: 'streaming',
          },
        ]}
      />,
    )
    // The follow heuristic must NOT jump to the bottom.
    expect(list.scrollTop).toBe(100)
  })

  test('does not follow-scroll when exactly on the 120px boundary', () => {
    const props = baseProps()
    props.streaming = true
    props.messages = [
      { id: 'a1', role: 'assistant', content: 'a', status: 'streaming' },
    ]
    const { rerender } = render(<AssistantChatView {...props} />)
    const list = screen.getByTestId('assistant-chat-messages')
    Object.defineProperty(list, 'scrollHeight', {
      value: 1000,
      configurable: true,
    })
    Object.defineProperty(list, 'clientHeight', {
      value: 400,
      configurable: true,
    })
    // distanceFromBottom = 1000 - 480 - 400 = 120 → the `< 120` guard is exclusive, no jump.
    list.scrollTop = 480
    rerender(
      <AssistantChatView
        {...props}
        messages={[
          {
            id: 'a1',
            role: 'assistant',
            content: 'a longer answer now',
            status: 'streaming',
          },
        ]}
      />,
    )
    expect(list.scrollTop).toBe(480)
  })

  test('follows the reasoning-only phase even before the first answer token', () => {
    const props = baseProps()
    props.streaming = true
    props.messages = [
      {
        id: 'a1',
        role: 'assistant',
        content: '',
        reasoning: 'one',
        status: 'streaming',
      },
    ]
    const { rerender } = render(<AssistantChatView {...props} />)
    const list = screen.getByTestId('assistant-chat-messages')
    Object.defineProperty(list, 'scrollHeight', {
      value: 500,
      configurable: true,
    })
    Object.defineProperty(list, 'clientHeight', {
      value: 400,
      configurable: true,
    })
    list.scrollTop = 90 // near bottom
    rerender(
      <AssistantChatView
        {...props}
        messages={[
          {
            id: 'a1',
            role: 'assistant',
            content: '',
            reasoning: 'one two three more thinking',
            status: 'streaming',
          },
        ]}
      />,
    )
    // followKey tracks reasoning length, so the effect re-runs and pins to bottom.
    expect(list.scrollTop).toBe(list.scrollHeight)
  })

  test('virtualizes a finished off-screen row and keeps the active row pinned', () => {
    const observer = installObserverMock()
    try {
      const props = baseProps()
      props.disableVirtualization = false
      props.streaming = true
      props.messages = [
        { id: 'u1', role: 'user', content: 'older question' },
        {
          id: 'a1',
          role: 'assistant',
          content: 'live answer',
          status: 'streaming',
        },
      ]
      render(<AssistantChatView {...props} />)

      // Both rows render initially (jsdom initialInView = true).
      const olderTurn = screen.getByTestId('assistant-turn-u1')
      expect(olderTurn).toBeVisible()
      const olderRow = olderTurn.parentElement as HTMLElement
      // Give the row a measurable height so the placeholder path runs.
      Object.defineProperty(olderRow, 'getBoundingClientRect', {
        value: () => ({ height: 80 }) as DOMRect,
        configurable: true,
      })

      // Scroll the finished (non-last) row out of view → it collapses to a placeholder.
      act(() => observer.trigger(olderRow, false))
      expect(screen.queryByTestId('assistant-turn-u1')).not.toBeInTheDocument()
      expect(olderRow).toHaveAttribute('data-virtualized', 'true')
      expect(olderRow.style.minHeight).toBe('80px')

      // The active streaming row is pinned and always rendered.
      expect(screen.getByTestId('assistant-turn-a1')).toBeVisible()

      // Scrolling it back into view remounts the real content.
      act(() => observer.trigger(olderRow, true))
      expect(screen.getByTestId('assistant-turn-u1')).toBeVisible()
    } finally {
      observer.restore()
    }
  })
})
