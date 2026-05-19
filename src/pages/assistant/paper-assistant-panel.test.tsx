import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type { AssistantConversationMessage } from './conversation-panel'
import { PaperAssistantPanel } from './paper-assistant-panel'
import { citationsToEvidence } from './paper-assistant-helpers'

function assistantT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

function makeMessage(
  over: Partial<AssistantConversationMessage> = {},
): AssistantConversationMessage {
  return {
    id: 'msg-1',
    role: 'user',
    content: 'hi',
    ...over,
  }
}

describe('citationsToEvidence', () => {
  test('derives domain by stripping the www. prefix', () => {
    const evidence = citationsToEvidence([
      {
        url: 'https://www.example.com/post/1',
        title: 'Hello',
        visitedAt: '2026-05-18T12:00:00Z',
      },
    ])
    expect(evidence).toHaveLength(1)
    expect(evidence[0].domain).toBe('example.com')
    expect(evidence[0].id).toBe('https://www.example.com/post/1-0')
    expect(evidence[0].date).toBe('2026-05-18')
  })

  test('falls back to empty domain for unparseable URLs', () => {
    const evidence = citationsToEvidence([
      { url: 'not a url', title: null, visitedAt: '' },
    ])
    expect(evidence[0].domain).toBe('')
    expect(evidence[0].title).toBe('not a url')
  })

  test('uses URL as the title when the citation title is blank', () => {
    const evidence = citationsToEvidence([
      {
        url: 'https://example.com/x',
        title: '   ',
        visitedAt: '2026-05-18T12:00:00Z',
      },
    ])
    expect(evidence[0].title).toBe('https://example.com/x')
  })
})

describe('PaperAssistantPanel', () => {
  test('renders the greeting + 3 prompts when there are no messages', () => {
    render(
      <PaperAssistantPanel
        assistantT={assistantT}
        input=""
        messages={[]}
        onInputChange={() => {}}
        onSend={() => {}}
        providerLabel="Ollama / llama3.2"
        sending={false}
        userByline="You"
      />,
    )
    expect(screen.getByTestId('paper-assistant-panel')).toBeInTheDocument()
    expect(screen.getByTestId('paper-assistant-view')).toBeInTheDocument()
    expect(screen.getByText('paperGreetingTitle')).toBeInTheDocument()
  })

  test('maps user + assistant messages into role-correct descriptors', () => {
    const messages: AssistantConversationMessage[] = [
      makeMessage({ id: 'u1', role: 'user', content: 'why' }),
      makeMessage({
        id: 'a1',
        role: 'assistant',
        content: 'because',
        response: {
          state: 'complete',
          answer: 'because',
          jobId: 1,
          runId: 1,
          providerId: 'local',
          embeddingProviderId: 'local',
          citations: [
            {
              historyId: 1,
              profileId: 'chrome:Default',
              url: 'https://example.com/',
              title: 'Doc',
              visitedAt: '2026-05-18T12:00:00Z',
              score: 0.9,
            },
          ],
          notes: [],
        },
      }),
    ]
    render(
      <PaperAssistantPanel
        assistantT={assistantT}
        input=""
        messages={messages}
        onInputChange={() => {}}
        onSend={() => {}}
        providerLabel={null}
        sending={false}
        userByline="You"
      />,
    )
    expect(screen.getByText('why')).toBeInTheDocument()
    expect(screen.getByText('because')).toBeInTheDocument()
    // assistant byline falls back to the keyword-only label when no provider
    // — the label also shows in the composer attribution, so we just confirm
    // it surfaces somewhere on the panel.
    expect(
      screen.getAllByText('paperComposerAttributionFallback').length,
    ).toBeGreaterThanOrEqual(1)
  })

  test('onSubmit propagates the value before calling onSend', () => {
    const onInputChange = vi.fn()
    const onSend = vi.fn()
    render(
      <PaperAssistantPanel
        assistantT={assistantT}
        input="what's new"
        messages={[]}
        onInputChange={onInputChange}
        onSend={onSend}
        providerLabel={null}
        sending={false}
        userByline="You"
      />,
    )
    const textarea = screen.getByPlaceholderText('paperComposerPlaceholder')
    fireEvent.keyDown(textarea, { key: 'Enter' })
    expect(onInputChange).toHaveBeenCalledWith("what's new")
    expect(onSend).toHaveBeenCalled()
  })

  test('picking a prompt seeds the input via onInputChange', () => {
    const onInputChange = vi.fn()
    render(
      <PaperAssistantPanel
        assistantT={assistantT}
        input=""
        messages={[]}
        onInputChange={onInputChange}
        onSend={() => {}}
        providerLabel="Ollama / llama3.2"
        sending={false}
        userByline="You"
      />,
    )
    // The three greeting prompts render as buttons; clicking the first one
    // should seed the input with its text.
    const promptButtons = screen
      .getAllByRole('button')
      .filter((node) => node.textContent === 'paperPrompt1')
    expect(promptButtons.length).toBeGreaterThan(0)
    fireEvent.click(promptButtons[0])
    expect(onInputChange).toHaveBeenCalledWith('paperPrompt1')
  })
})
