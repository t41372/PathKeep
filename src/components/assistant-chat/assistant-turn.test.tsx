/**
 * @file assistant-turn.test.tsx
 * @description Coverage for the per-message renderer (user bubble vs assistant turn).
 *
 * Proves: the user variant renders the prompt in the user bubble; the assistant variant composes
 * byline → reasoning → tools → streaming answer → evidence in order; the typing indicator shows
 * only before any content; the error state renders for a failed turn; evidence rows route to the
 * select handler.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'

vi.mock('streamdown', () => ({
  Streamdown: (props: { children?: unknown }) => (
    <div data-testid="streamdown-stub">{String(props.children)}</div>
  ),
}))

import { AssistantTurn, type AssistantTurnCopy } from './assistant-turn'
import type { ChatMessage } from './use-ai-chat-stream'

const copy: AssistantTurnCopy = {
  assistantByline: 'Local · model',
  userByline: 'You',
  typingLabel: 'Thinking…',
  evidenceLabel: 'Sources · {count} records',
  errorGeneric: "The assistant couldn't finish this answer.",
  stoppedLabel: 'Generation stopped',
  retryLabel: 'Try again',
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
    toggleLabel: 'Toggle the thought process',
  },
  toolCalls: {
    label: 'Tools used',
    ranTemplate: 'Ran {name}',
    runningLabel: 'Running…',
    doneLabel: 'Done',
    failedLabel: 'Failed',
    resultToggleLabel: 'Toggle tool result',
  },
}

function userMessage(content: string): ChatMessage {
  return { id: 'u1', role: 'user', content }
}

describe('AssistantTurn', () => {
  test('renders a user prompt in the user bubble with its byline', () => {
    render(<AssistantTurn message={userMessage('hello there')} copy={copy} />)
    const turn = screen.getByTestId('assistant-turn-u1')
    expect(turn).toHaveAttribute('data-role', 'user')
    expect(turn).toHaveTextContent('hello there')
    expect(screen.queryByText('You')).not.toBeInTheDocument() // user byline is hidden for user role
  })

  test('shows the typing indicator before any content arrives', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a1',
          role: 'assistant',
          content: '',
          status: 'streaming',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-typing-indicator')).toHaveAttribute(
      'aria-label',
      'Thinking…',
    )
    expect(screen.queryByTestId('assistant-answer-a1')).not.toBeInTheDocument()
  })

  test('composes reasoning, tools, and the streaming answer for an active turn', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a2',
          role: 'assistant',
          content: 'The answer is 42.',
          reasoning: 'thinking hard',
          toolCalls: [{ id: 't1', name: 'search_bm25', arguments: '{}' }],
          status: 'streaming',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-reasoning-a2')).toBeVisible()
    expect(screen.getByTestId('assistant-tools-a2')).toBeVisible()
    expect(screen.getByText('Ran search_bm25')).toBeVisible()
    const answer = screen.getByTestId('assistant-answer-a2')
    expect(answer).toHaveAttribute('data-streaming', 'true')
    expect(answer).toHaveTextContent('The answer is 42.')
    // No typing indicator once content exists.
    expect(
      screen.queryByTestId('assistant-typing-indicator'),
    ).not.toBeInTheDocument()
  })

  test('renders the error state for a failed turn', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a3',
          role: 'assistant',
          content: 'partial',
          status: 'error',
          error: 'provider unreachable',
        }}
        copy={copy}
      />,
    )
    const error = screen.getByTestId('assistant-error-a3')
    expect(error).toHaveAttribute('role', 'alert')
    expect(error).toHaveTextContent('provider unreachable')
  })

  test('renders evidence rows and routes clicks to the select handler', () => {
    const onSelect = vi.fn()
    render(
      <AssistantTurn
        message={{
          id: 'a4',
          role: 'assistant',
          content: 'done',
          status: 'done',
        }}
        copy={copy}
        evidence={[
          {
            id: 'e1',
            date: '2026-04-05',
            title: 'A page',
            domain: 'example.com',
            url: 'https://example.com/a',
          },
        ]}
        onSelectEvidence={onSelect}
      />,
    )
    expect(screen.getByText('Sources · 1 records')).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-assistant-evidence-e1'))
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'e1', url: 'https://example.com/a' }),
    )
  })

  test('renders a finished answer without the streaming flag', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a5',
          role: 'assistant',
          content: 'final answer',
          status: 'done',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-answer-a5')).toHaveAttribute(
      'data-streaming',
      'false',
    )
    // No streaming caret on a finished answer.
    expect(
      screen.queryByTestId('assistant-answer-a5-caret'),
    ).not.toBeInTheDocument()
  })

  test('appends a blinking caret while the answer streams', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a6',
          role: 'assistant',
          content: 'streaming…',
          status: 'streaming',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-answer-a6-caret')).toBeInTheDocument()
  })

  test('falls back to the generic error copy when the turn carries no message', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a7',
          role: 'assistant',
          content: '',
          status: 'error',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-error-a7')).toHaveTextContent(
      "The assistant couldn't finish this answer.",
    )
  })

  test('wires Try again on an error turn to the retry handler', () => {
    const onRetry = vi.fn()
    render(
      <AssistantTurn
        message={{
          id: 'a8',
          role: 'assistant',
          content: 'partial',
          status: 'error',
          error: 'provider unreachable',
        }}
        copy={copy}
        onRetry={onRetry}
      />,
    )
    fireEvent.click(screen.getByTestId('assistant-retry-a8'))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  test('omits the retry button on an error turn when no handler is provided', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a9',
          role: 'assistant',
          content: 'partial',
          status: 'error',
          error: 'boom',
        }}
        copy={copy}
      />,
    )
    expect(screen.queryByTestId('assistant-retry-a9')).not.toBeInTheDocument()
  })

  test('shows the stopped affordance and a retry on a cancelled turn', () => {
    const onRetry = vi.fn()
    render(
      <AssistantTurn
        message={{
          id: 'a10',
          role: 'assistant',
          content: 'half an answer',
          status: 'cancelled',
        }}
        copy={copy}
        onRetry={onRetry}
      />,
    )
    expect(screen.getByTestId('assistant-stopped-a10')).toHaveTextContent(
      'Generation stopped',
    )
    // The partial answer is still rendered above the stopped affordance.
    expect(screen.getByTestId('assistant-answer-a10')).toHaveTextContent(
      'half an answer',
    )
    fireEvent.click(screen.getByTestId('assistant-retry-a10'))
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  test('omits the retry button on a cancelled turn when no handler is provided', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a11',
          role: 'assistant',
          content: 'half',
          status: 'cancelled',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-stopped-a11')).toBeVisible()
    expect(screen.queryByTestId('assistant-retry-a11')).not.toBeInTheDocument()
  })

  test('shows the no-answer fallback for an empty completed turn', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a12',
          role: 'assistant',
          content: '',
          status: 'done',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-no-answer-a12')).toHaveTextContent(
      'No answer was returned.',
    )
  })

  test('does not show the no-answer fallback when the done turn has reasoning only', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a13',
          role: 'assistant',
          content: '',
          reasoning: 'I thought about it',
          status: 'done',
        }}
        copy={copy}
      />,
    )
    expect(
      screen.queryByTestId('assistant-no-answer-a13'),
    ).not.toBeInTheDocument()
  })

  test('renders the token-usage footer when the turn carries usage', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a14',
          role: 'assistant',
          content: 'answer',
          usage: { promptTokens: 120, completionTokens: 35 },
          status: 'done',
        }}
        copy={copy}
      />,
    )
    expect(screen.getByTestId('assistant-usage-a14')).toHaveTextContent(
      '120 prompt · 35 completion tokens',
    )
  })

  test('omits the usage footer when the turn has no usage', () => {
    render(
      <AssistantTurn
        message={{
          id: 'a15',
          role: 'assistant',
          content: 'answer',
          status: 'done',
        }}
        copy={copy}
      />,
    )
    expect(screen.queryByTestId('assistant-usage-a15')).not.toBeInTheDocument()
  })

  test('forwards evidence-row star props to the message bubble', () => {
    const onToggleStar = vi.fn()
    render(
      <AssistantTurn
        message={{
          id: 'a16',
          role: 'assistant',
          content: 'answer',
          status: 'done',
        }}
        copy={copy}
        evidence={[
          {
            id: 'cite-1',
            date: '2026-01-01',
            title: 'A',
            domain: 'a.example',
            url: 'https://a.example/x',
            canonicalUrl: 'https://a.example/x',
          },
        ]}
        isEvidenceStarred={() => false}
        onToggleEvidenceStar={onToggleStar}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-assistant-evidence-star-cite-1'))
    expect(onToggleStar).toHaveBeenCalledTimes(1)
    expect(onToggleStar).toHaveBeenCalledWith('https://a.example/x')
  })

  describe('coarse aria-live milestones', () => {
    function milestone(message: ChatMessage) {
      render(<AssistantTurn message={message} copy={copy} />)
      return screen.getByTestId(`assistant-live-${message.id}`)
    }

    test('announces Thinking… while reasoning streams without an answer', () => {
      expect(
        milestone({
          id: 'm1',
          role: 'assistant',
          content: '',
          reasoning: 'pondering',
          status: 'streaming',
        }),
      ).toHaveTextContent('Thinking…')
    })

    test('announces the tool name while a tool runs before any answer', () => {
      expect(
        milestone({
          id: 'm2',
          role: 'assistant',
          content: '',
          toolCalls: [{ id: 't1', name: 'search_bm25', arguments: '{}' }],
          status: 'streaming',
        }),
      ).toHaveTextContent('Using tool: search_bm25')
    })

    test('announces Answering… once answer tokens arrive', () => {
      expect(
        milestone({
          id: 'm3',
          role: 'assistant',
          content: 'partial answer',
          status: 'streaming',
        }),
      ).toHaveTextContent('Answering…')
    })

    test('announces the typing label before anything arrives', () => {
      expect(
        milestone({
          id: 'm4',
          role: 'assistant',
          content: '',
          status: 'streaming',
        }),
      ).toHaveTextContent('Thinking…')
    })

    test('announces Answer complete on a non-empty done turn', () => {
      expect(
        milestone({
          id: 'm5',
          role: 'assistant',
          content: 'the answer',
          status: 'done',
        }),
      ).toHaveTextContent('Answer complete')
    })

    test('announces the no-answer fallback on an empty done turn', () => {
      expect(
        milestone({
          id: 'm6',
          role: 'assistant',
          content: '',
          status: 'done',
        }),
      ).toHaveTextContent('No answer was returned.')
    })

    test('announces the stopped label on a cancelled turn', () => {
      expect(
        milestone({
          id: 'm7',
          role: 'assistant',
          content: 'half',
          status: 'cancelled',
        }),
      ).toHaveTextContent('Generation stopped')
    })

    test('announces the specific error message on an error turn', () => {
      expect(
        milestone({
          id: 'm8',
          role: 'assistant',
          content: '',
          status: 'error',
          error: 'provider unreachable',
        }),
      ).toHaveTextContent('provider unreachable')
    })

    test('announces the generic error when an error turn has no message', () => {
      expect(
        milestone({
          id: 'm9',
          role: 'assistant',
          content: '',
          status: 'error',
        }),
      ).toHaveTextContent("The assistant couldn't finish this answer.")
    })

    test('announces nothing for a turn with no status (user-side default branch)', () => {
      expect(
        milestone({
          id: 'm10',
          role: 'assistant',
          content: '',
        }),
      ).toBeEmptyDOMElement()
    })
  })
})
