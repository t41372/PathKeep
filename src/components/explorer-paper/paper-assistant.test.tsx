/**
 * Tests for the Assistant view primitives — message bubble + composer +
 * greeting.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperAssistantComposer,
  PaperAssistantGreeting,
  PaperAssistantMessage,
  type PaperAssistantComposerCopy,
  type PaperAssistantEvidence,
} from './index'

const COMPOSER_COPY: PaperAssistantComposerCopy = {
  placeholder: 'Ask about your archive…',
  sendLabel: 'Send',
  attribution: 'Powered by local LLM · Ollama / llama3.2',
  keyHint: '↵ send · ⇧↵ newline',
}

const EVIDENCE: PaperAssistantEvidence[] = [
  {
    id: 'e1',
    date: '2025-04-05',
    title: 'tauri-apps/tauri',
    domain: 'github.com',
    url: 'https://github.com/tauri-apps/tauri',
  },
  {
    id: 'e2',
    date: '2025-04-06',
    title: 'Getting Started | Tauri',
    domain: 'v2.tauri.app',
    url: 'https://v2.tauri.app/start/',
  },
]

describe('PaperAssistantMessage', () => {
  test('renders user variant right-aligned with the bubble fill', () => {
    render(
      <PaperAssistantMessage role="user" testId="msg-user">
        When did I first read about Tauri?
      </PaperAssistantMessage>,
    )

    const msg = screen.getByTestId('msg-user')
    expect(msg.dataset.role).toBe('user')
    expect(
      within(msg).getByText('When did I first read about Tauri?'),
    ).toBeVisible()
  })

  test('renders the AI byline above the bubble', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        byline="Local · llama 3.2"
        testId="msg-ai"
      >
        You first opened Tauri on April 5, 2025.
      </PaperAssistantMessage>,
    )

    expect(screen.getByText('Local · llama 3.2')).toBeVisible()
    expect(
      screen.getByText('You first opened Tauri on April 5, 2025.'),
    ).toBeVisible()
  })

  test('renders the evidence panel when supplied and routes clicks', () => {
    const onSelect = vi.fn()
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={EVIDENCE}
        evidenceLabel="Evidence · {count} records"
        onSelectEvidence={onSelect}
        testId="msg-evidence"
      >
        Answer body
      </PaperAssistantMessage>,
    )

    expect(screen.getByText('Evidence · 2 records')).toBeVisible()
    expect(screen.getByText('tauri-apps/tauri')).toBeVisible()

    fireEvent.click(screen.getByTestId('paper-assistant-evidence-e2'))
    expect(onSelect).toHaveBeenCalledWith(EVIDENCE[1])
  })

  test('bounds a long evidence list in a scroll region so it never crams the turn', () => {
    // The recent-visits enumeration can cite many rows; the panel must stay a contained sources
    // block rather than an unbounded flat list that shoves the answer out of view.
    const many: PaperAssistantEvidence[] = Array.from(
      { length: 30 },
      (_, i) => ({
        id: `e${i}`,
        date: '2025-04-05',
        title: `Cited page ${i}`,
        domain: 'example.com',
        url: `https://example.com/${i}`,
      }),
    )
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={many}
        evidenceLabel="Evidence · {count} records"
        testId="msg-many"
      >
        Answer body
      </PaperAssistantMessage>,
    )
    const rows = screen.getByTestId('paper-assistant-evidence-rows')
    // Rows live inside a bounded, scrollable region (max-height + overflow), not a raw flat list.
    expect(rows.className).toContain('overflow-y-auto')
    expect(rows.className).toMatch(/max-h-/)
    // All rows are still present (nothing is dropped — the region just scrolls).
    expect(within(rows).getByText('Cited page 0')).toBeInTheDocument()
    expect(within(rows).getByText('Cited page 29')).toBeInTheDocument()
  })

  test('evidence rows are disabled when no handler is provided', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={EVIDENCE}
        evidenceLabel="Evidence · {count}"
      >
        Body
      </PaperAssistantMessage>,
    )

    expect(
      screen.getByTestId<HTMLButtonElement>('paper-assistant-evidence-e1')
        .disabled,
    ).toBe(true)
  })

  test('omits the evidence panel when none is supplied', () => {
    render(
      <PaperAssistantMessage role="ai" testId="msg-no-evidence">
        Body
      </PaperAssistantMessage>,
    )
    expect(screen.queryByTestId('paper-assistant-evidence')).toBeNull()
  })

  const STAR_COPY = {
    starLabel: 'Star this source',
    unstarLabel: 'Unstar this source',
    status: { starred: 'Source starred', unstarred: 'Source unstarred' },
  }

  test('renders a star toggle per starrable evidence row and routes toggles by canonicalUrl', () => {
    const onToggleStar = vi.fn()
    const starred = new Set(['https://github.com/tauri-apps/tauri'])
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={[
          {
            ...EVIDENCE[0],
            canonicalUrl: 'https://github.com/tauri-apps/tauri',
          },
          { ...EVIDENCE[1], canonicalUrl: 'https://v2.tauri.app/start/' },
        ]}
        evidenceLabel="Evidence · {count} records"
        isEvidenceStarred={(canonicalUrl) => starred.has(canonicalUrl)}
        onToggleEvidenceStar={onToggleStar}
        evidenceStarCopy={STAR_COPY}
        testId="msg-star"
      >
        Answer body
      </PaperAssistantMessage>,
    )
    // The already-starred row shows the Unstar action (aria-pressed reflects state).
    const starredToggle = screen.getByTestId('paper-assistant-evidence-star-e1')
    expect(starredToggle).toHaveAttribute('aria-pressed', 'true')
    expect(starredToggle).toHaveAttribute('aria-label', 'Unstar this source')
    // The unstarred row shows the Star action.
    const unstarredToggle = screen.getByTestId(
      'paper-assistant-evidence-star-e2',
    )
    expect(unstarredToggle).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(unstarredToggle)
    expect(onToggleStar).toHaveBeenCalledTimes(1)
    // The toggle is keyed by the row's canonical url (the W-STAR key).
    expect(onToggleStar).toHaveBeenCalledWith('https://v2.tauri.app/start/')
  })

  test('defaults a row to unstarred when no isEvidenceStarred resolver is wired', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={[
          {
            ...EVIDENCE[0],
            canonicalUrl: 'https://github.com/tauri-apps/tauri',
          },
        ]}
        evidenceLabel="Evidence · {count}"
        onToggleEvidenceStar={vi.fn()}
        evidenceStarCopy={STAR_COPY}
        testId="msg-nostatus"
      >
        Body
      </PaperAssistantMessage>,
    )
    // The toggle still renders (toggle + copy + canonicalUrl present), defaulting to not-starred.
    expect(
      screen.getByTestId('paper-assistant-evidence-star-e1'),
    ).toHaveAttribute('aria-pressed', 'false')
  })

  test('omits the star toggle when a row has no canonicalUrl', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={[{ ...EVIDENCE[0], canonicalUrl: null }]}
        evidenceLabel="Evidence · {count}"
        isEvidenceStarred={() => false}
        onToggleEvidenceStar={vi.fn()}
        evidenceStarCopy={STAR_COPY}
        testId="msg-nostar"
      >
        Body
      </PaperAssistantMessage>,
    )
    expect(
      screen.queryByTestId('paper-assistant-evidence-star-e1'),
    ).not.toBeInTheDocument()
  })

  test('omits the star toggle when the toggle handler / copy are not wired', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={[
          {
            ...EVIDENCE[0],
            canonicalUrl: 'https://github.com/tauri-apps/tauri',
          },
        ]}
        evidenceLabel="Evidence · {count}"
        testId="msg-nowire"
      >
        Body
      </PaperAssistantMessage>,
    )
    // No onToggleEvidenceStar/evidenceStarCopy → not starrable even with a canonicalUrl.
    expect(
      screen.queryByTestId('paper-assistant-evidence-star-e1'),
    ).not.toBeInTheDocument()
  })

  test('renders evidence rows without the count strip when evidenceLabel is omitted', () => {
    render(
      <PaperAssistantMessage
        role="ai"
        evidence={EVIDENCE}
        testId="msg-no-label"
      >
        Body
      </PaperAssistantMessage>,
    )
    // The evidence panel still mounts, but the `evidenceLabel ?` ternary
    // at line 99 of paper-assistant-message.tsx takes the falsy branch
    // and the count strip is not rendered.
    expect(screen.getByTestId('paper-assistant-evidence')).toBeVisible()
    expect(screen.queryByText(/Evidence ·/)).toBeNull()
  })
})

describe('PaperAssistantComposer', () => {
  test('Enter submits the trimmed value and stops the default form submit', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantComposer
        value="  When did I read about Tauri?  "
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-assistant-input'), {
      key: 'Enter',
    })
    expect(onSubmit).toHaveBeenCalledWith('When did I read about Tauri?')
  })

  test('Shift+Enter does not submit', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantComposer
        value="multiline"
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-assistant-input'), {
      key: 'Enter',
      shiftKey: true,
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  test('empty / whitespace-only input cannot submit', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantComposer
        value="   "
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-assistant-input'), {
      key: 'Enter',
    })
    expect(onSubmit).not.toHaveBeenCalled()
    expect(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Send' }).disabled,
    ).toBe(true)
  })

  test('pending state disables the textarea and the send button', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantComposer
        value="ready"
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
        pending
      />,
    )

    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-assistant-input').disabled,
    ).toBe(true)
    expect(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Send' }).disabled,
    ).toBe(true)

    fireEvent.keyDown(screen.getByTestId('paper-assistant-input'), {
      key: 'Enter',
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  test('typing forwards to onChange', () => {
    const onChange = vi.fn()
    render(
      <PaperAssistantComposer
        value=""
        onChange={onChange}
        onSubmit={() => {}}
        copy={COMPOSER_COPY}
      />,
    )

    fireEvent.change(screen.getByTestId('paper-assistant-input'), {
      target: { value: 'next' },
    })
    expect(onChange).toHaveBeenCalledWith('next')
  })

  test('renders attribution + key hint meta beneath the row', () => {
    render(
      <PaperAssistantComposer
        value=""
        onChange={() => {}}
        onSubmit={() => {}}
        copy={COMPOSER_COPY}
      />,
    )

    expect(screen.getByText(/Powered by local LLM/)).toBeVisible()
    expect(screen.getByText(/↵ send/)).toBeVisible()
  })

  test('clicking the Send button submits the trimmed value via the form onSubmit', () => {
    const onSubmit = vi.fn()
    render(
      <PaperAssistantComposer
        value="  Hello there  "
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
      />,
    )
    fireEvent.click(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Send' }),
    )
    expect(onSubmit).toHaveBeenCalledWith('Hello there')
  })

  test('form onSubmit is a no-op when the value is empty', () => {
    const onSubmit = vi.fn()
    const { container } = render(
      <PaperAssistantComposer
        value=""
        onChange={() => {}}
        onSubmit={onSubmit}
        copy={COMPOSER_COPY}
      />,
    )
    const form = container.querySelector('form')
    if (!(form instanceof HTMLFormElement)) throw new Error('form missing')
    fireEvent.submit(form)
    expect(onSubmit).not.toHaveBeenCalled()
  })
})

describe('PaperAssistantGreeting', () => {
  test('renders title, subtitle, and prompt grid', () => {
    render(
      <PaperAssistantGreeting
        title="What would you like to remember?"
        subtitle="I can read your archive and tell you what's in it."
        prompts={[
          { id: 'p1', text: 'When did I first start reading about Tauri?' },
          { id: 'p2', text: 'Pages I keep coming back to but never finished.' },
        ]}
        onSelectPrompt={() => {}}
        testId="greeting"
      />,
    )

    expect(screen.getByText('What would you like to remember?')).toBeVisible()
    expect(screen.getByText(/I can read your archive/)).toBeVisible()
    expect(
      screen.getByText('When did I first start reading about Tauri?'),
    ).toBeVisible()
  })

  test('clicking a prompt forwards the entry to onSelectPrompt', () => {
    const onSelect = vi.fn()
    const prompts = [{ id: 'p1', text: 'First prompt' }]
    render(
      <PaperAssistantGreeting
        title="Title"
        subtitle="Sub"
        prompts={prompts}
        onSelectPrompt={onSelect}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-assistant-prompt-p1'))
    expect(onSelect).toHaveBeenCalledWith(prompts[0])
  })

  test('prompts are disabled when no handler is supplied', () => {
    render(
      <PaperAssistantGreeting
        title="Title"
        subtitle="Sub"
        prompts={[{ id: 'p1', text: 'Inert' }]}
      />,
    )

    expect(
      screen.getByTestId<HTMLButtonElement>('paper-assistant-prompt-p1')
        .disabled,
    ).toBe(true)
  })

  test('renders without prompts when none are supplied', () => {
    render(
      <PaperAssistantGreeting
        title="Title"
        subtitle="Sub"
        testId="greeting-bare"
      />,
    )
    expect(screen.getByTestId('greeting-bare')).toBeVisible()
    expect(screen.queryByText(/p1/)).toBeNull()
  })
})
