/**
 * @file conversation-panel.test.tsx
 * @description Focused coverage for the extracted Assistant conversation shell.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Verify the empty-state prompt picker, queued inline actions, evidence links, and sending state.
 * - Keep the extracted conversation owner honest without mounting the full Assistant route.
 *
 * ## Not responsible for
 * - Re-testing provider probes, queue status cards, or archive gating.
 * - Re-testing backend request orchestration.
 *
 * ## Dependencies
 * - Depends on the shipped Assistant, Intelligence, and Common translators.
 * - Uses a memory router because evidence chips render real links.
 *
 * ## Performance notes
 * - Focused render tests avoid replaying the full Assistant route for conversation-only behavior.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator, createTranslator } from '../../lib/i18n'
import type { AiAssistantResponse } from '../../lib/types'
import {
  AssistantConversationPanel,
  type AssistantConversationMessage,
} from './conversation-panel'

const assistantT = createNamespaceTranslator('en', 'assistant')
const t = createTranslator('en')

describe('assistant conversation panel', () => {
  test('renders the empty state and lets prompt shortcuts seed the composer', async () => {
    const user = userEvent.setup()
    const onPromptPick = vi.fn()

    render(
      <MemoryRouter>
        <AssistantConversationPanel
          assistantT={assistantT}
          handleCancelJob={vi.fn()}
          handleDrainQueue={vi.fn()}
          handleLoadQueuedJob={vi.fn()}
          input=""
          language="en"
          messages={[]}
          onInputChange={vi.fn()}
          onPromptPick={onPromptPick}
          onSend={vi.fn()}
          queueAction={null}
          responseMetaFor={() => ({ label: 'Ready', tone: 'success' })}
          sending={false}
          suggestedQuestions={[
            assistantT('examplePrompt'),
            assistantT('examplePromptFocus'),
          ]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText(assistantT('emptyTitle'))).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: assistantT('loadExamplePrompt') }),
    )

    expect(onPromptPick).toHaveBeenCalledWith(assistantT('examplePrompt'))
  })

  test('renders queued responses with evidence links and inline queue actions', async () => {
    const user = userEvent.setup()
    const handleLoadQueuedJob = vi.fn().mockResolvedValue(undefined)
    const handleDrainQueue = vi.fn().mockResolvedValue(undefined)
    const handleCancelJob = vi.fn().mockResolvedValue(undefined)
    const response: AiAssistantResponse = {
      state: 'queued',
      answer: 'Queued answer',
      jobId: 77,
      runId: 42,
      providerId: 'ollama',
      embeddingProviderId: 'nomic',
      citations: [
        {
          historyId: 9,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Example article',
          visitedAt: '2026-04-10T10:00:00Z',
          score: 0.91,
        },
      ],
      notes: ['One note'],
    }
    const messages: AssistantConversationMessage[] = [
      {
        id: 'assistant-1',
        role: 'assistant',
        content: '- first bullet',
        response,
      },
    ]

    render(
      <MemoryRouter>
        <AssistantConversationPanel
          assistantT={assistantT}
          handleCancelJob={handleCancelJob}
          handleDrainQueue={handleDrainQueue}
          handleLoadQueuedJob={handleLoadQueuedJob}
          input=""
          language="en"
          messages={messages}
          onInputChange={vi.fn()}
          onPromptPick={vi.fn()}
          onSend={vi.fn()}
          queueAction={null}
          responseMetaFor={() => ({ label: 'Queued', tone: 'warning' })}
          sending={false}
          suggestedQuestions={[]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('Queued')).toBeVisible()
    expect(screen.getByText('One note')).toBeVisible()
    expect(
      screen.getByText(assistantT('evidenceLabel', { count: 1 })),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: /Example article/ }),
    ).toHaveAttribute('href', expect.stringContaining('/explorer?'))

    const buttons = {
      check: screen.getByRole('button', { name: assistantT('checkStatus') }),
      run: screen.getByRole('button', { name: assistantT('runQueuedJob') }),
      cancel: screen.getByRole('button', { name: assistantT('cancel') }),
    }

    await user.click(buttons.check)
    await user.click(buttons.run)
    await user.click(buttons.cancel)

    expect(handleLoadQueuedJob).toHaveBeenCalledWith(77)
    expect(handleDrainQueue).toHaveBeenCalledWith(77)
    expect(handleCancelJob).toHaveBeenCalledWith(77)
  })

  test('renders sparse assistant responses without queued actions', () => {
    const response: AiAssistantResponse = {
      state: 'queued',
      answer: '',
      jobId: null,
      runId: null,
      providerId: '',
      embeddingProviderId: '',
      citations: [
        {
          historyId: 10,
          profileId: '',
          url: 'https://fallback.example/source',
          title: null,
          visitedAt: 'not-a-date',
          score: 0.25,
        },
      ],
      notes: [],
    }

    render(
      <MemoryRouter>
        <AssistantConversationPanel
          assistantT={assistantT}
          handleCancelJob={vi.fn()}
          handleDrainQueue={vi.fn()}
          handleLoadQueuedJob={vi.fn()}
          input=""
          language="en"
          messages={[
            {
              id: 'assistant-sparse',
              role: 'assistant',
              content: '',
              response,
            },
          ]}
          onInputChange={vi.fn()}
          onPromptPick={vi.fn()}
          onSend={vi.fn()}
          queueAction={null}
          responseMetaFor={() => ({ label: 'Queued', tone: 'warning' })}
          sending={false}
          suggestedQuestions={[]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText(/not-a-date/)).toBeVisible()
    expect(
      screen.getByRole('link', { name: /https:\/\/fallback\.example\/source/ }),
    ).toHaveAttribute('href', expect.stringContaining('/explorer?'))
    expect(
      screen.queryByRole('button', { name: assistantT('checkStatus') }),
    ).toBeNull()
  })

  test('renders the sending state and disables composer actions while a reply is pending', async () => {
    const user = userEvent.setup()
    const onInputChange = vi.fn()
    const onSend = vi.fn()

    render(
      <MemoryRouter>
        <AssistantConversationPanel
          assistantT={assistantT}
          handleCancelJob={vi.fn()}
          handleDrainQueue={vi.fn()}
          handleLoadQueuedJob={vi.fn()}
          input="Where did I read about sqlite?"
          language="en"
          messages={[
            {
              id: 'user-1',
              role: 'user',
              content: 'Where did I read about sqlite?',
            },
          ]}
          onInputChange={onInputChange}
          onPromptPick={vi.fn()}
          onSend={onSend}
          queueAction="Running queued jobs"
          responseMetaFor={() => ({ label: 'Ready', tone: 'success' })}
          sending={true}
          suggestedQuestions={[]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText(assistantT('preparingAnswer'))).toBeVisible()
    expect(screen.getByLabelText(assistantT('inputLabel'))).toBeDisabled()
    expect(
      screen.getByRole('button', { name: assistantT('sendAction') }),
    ).toBeDisabled()

    await user.click(
      screen.getByRole('button', { name: assistantT('sendAction') }),
    )

    expect(onInputChange).not.toHaveBeenCalled()
    expect(onSend).not.toHaveBeenCalled()
  })
})
