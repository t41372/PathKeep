/**
 * Tests for the PaperAssistantView composition.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperAssistantView,
  type PaperAssistantMessageDescriptor,
  type PaperAssistantViewCopy,
} from './index'

const COPY: PaperAssistantViewCopy = {
  greetingTitle: 'What would you like to remember?',
  greetingSubtitle: 'I can read your archive.',
  composer: {
    placeholder: 'Ask…',
    sendLabel: 'Send',
    attribution: 'Local LLM',
    keyHint: '↵',
  },
  evidenceLabel: 'Evidence · {count} records',
}

const MESSAGES: PaperAssistantMessageDescriptor[] = [
  {
    id: 'm1',
    role: 'user',
    content: 'When did I first read about Tauri?',
  },
  {
    id: 'm2',
    role: 'ai',
    byline: 'Local · llama 3.2',
    content: 'You opened Tauri on April 5, 2025.',
    evidence: [
      {
        id: 'e1',
        date: '2025-04-05',
        title: 'tauri-apps/tauri',
        domain: 'github.com',
        url: 'https://github.com/tauri-apps/tauri',
      },
    ],
  },
]

describe('PaperAssistantView', () => {
  test('renders the greeting when there are no messages', () => {
    render(
      <PaperAssistantView
        messages={[]}
        input=""
        onInputChange={() => {}}
        onSubmit={() => {}}
        prompts={[{ id: 'p1', text: 'When did I read about X?' }]}
        copy={COPY}
        testId="assist"
      />,
    )

    expect(screen.getByText('What would you like to remember?')).toBeVisible()
    expect(screen.getByText('When did I read about X?')).toBeVisible()
  })

  test('renders the messages list when messages exist', () => {
    render(
      <PaperAssistantView
        messages={MESSAGES}
        input=""
        onInputChange={() => {}}
        onSubmit={() => {}}
        copy={COPY}
        testId="assist-msg"
      />,
    )

    expect(screen.getByTestId('paper-assistant-message-m1')).toBeVisible()
    expect(screen.getByTestId('paper-assistant-message-m2')).toBeVisible()
    expect(screen.getByText('When did I first read about Tauri?')).toBeVisible()
    expect(screen.getByText('You opened Tauri on April 5, 2025.')).toBeVisible()
    expect(screen.getByText('Evidence · 1 records')).toBeVisible()
  })

  test('typing into the composer forwards onInputChange', () => {
    const onInputChange = vi.fn()
    render(
      <PaperAssistantView
        messages={MESSAGES}
        input=""
        onInputChange={onInputChange}
        onSubmit={() => {}}
        copy={COPY}
        testId="assist-typing"
      />,
    )

    fireEvent.change(screen.getByTestId('paper-assistant-input'), {
      target: { value: 'hello' },
    })
    expect(onInputChange).toHaveBeenCalledWith('hello')
  })

  test('Enter on the composer forwards onSubmit with the trimmed value', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantView
        messages={MESSAGES}
        input="  ask "
        onInputChange={() => {}}
        onSubmit={onSubmit}
        copy={COPY}
        testId="assist-submit"
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-assistant-input'), {
      key: 'Enter',
    })
    expect(onSubmit).toHaveBeenCalledWith('ask')
  })

  test('pending state disables the composer', () => {
    render(
      <PaperAssistantView
        messages={MESSAGES}
        input="anything"
        pending
        onInputChange={() => {}}
        onSubmit={() => {}}
        copy={COPY}
        testId="assist-pending"
      />,
    )

    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-assistant-input').disabled,
    ).toBe(true)
  })

  test('clicking a greeting prompt routes onPickPrompt', () => {
    const onPickPrompt = vi.fn()
    const prompts = [{ id: 'p1', text: 'Some prompt' }]
    render(
      <PaperAssistantView
        messages={[]}
        input=""
        onInputChange={() => {}}
        onSubmit={() => {}}
        prompts={prompts}
        onPickPrompt={onPickPrompt}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-assistant-prompt-p1'))
    expect(onPickPrompt).toHaveBeenCalledWith(prompts[0])
  })

  test('clicking evidence routes onSelectEvidence', () => {
    const onSelectEvidence = vi.fn()
    render(
      <PaperAssistantView
        messages={MESSAGES}
        input=""
        onInputChange={() => {}}
        onSubmit={() => {}}
        onSelectEvidence={onSelectEvidence}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-assistant-evidence-e1'))
    expect(onSelectEvidence).toHaveBeenCalledWith(MESSAGES[1].evidence?.[0])
  })
})
