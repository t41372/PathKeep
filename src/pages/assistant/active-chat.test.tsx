/**
 * @file active-chat.test.tsx
 * @description Covers the active streaming-chat surface of the rebuilt Assistant route — the
 *              W-AI-2 marquee. The AI-off / setup / locked gates live in `index.test.tsx`
 *              (which runs against the real release-capabilities flag and the seeded default-OFF
 *              `config.ai.enabled`); this file mocks the flag `true` and enables AI so the live
 *              chat branches render.
 *
 * Approach: `release-capabilities` is mocked to enable optional AI; `subscribeToAiChatStream` is
 * mocked to capture the listener and feed scripted chunk sequences (real streaming only works
 * under Tauri); `backend.sendAiChat` / `backend.cancelAiChat` are spied. streamdown is stubbed
 * so the markdown answer renders deterministically as text.
 *
 * Proves: no-provider state, empty greeting + prompt seeding, send → token/reasoning/tool stream
 * → done finalize, cancel, query-param seeding, and a multi-turn transcript carrying prior context.
 */

import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as AssistantChatModule from '../../components/assistant-chat'

vi.mock('../../lib/release-capabilities', () => ({
  deferredFeatureReleaseLabel: 'v0.3',
  optionalAiFeaturesAvailable: true,
  readableContentFetchAvailable: false,
}))

vi.mock('streamdown', () => ({
  Streamdown: (props: { children?: unknown }) => (
    <div data-testid="streamdown-stub">{String(props.children)}</div>
  ),
}))

// The "Export conversation" affordance lazily imports the native save dialog; mock it so the
// export flow resolves to a spy the test owns (real dialogs only work under Tauri).
const dialogSaveMock = vi.fn<(...args: unknown[]) => Promise<unknown>>()
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]): Promise<unknown> => dialogSaveMock(...args),
}))

const subscribeMock =
  vi.fn<
    (runId: string, listener: (chunk: unknown) => void) => Promise<() => void>
  >()
vi.mock('../../lib/ipc/ai-stream', () => ({
  subscribeToAiChatStream: (
    runId: string,
    listener: (chunk: unknown) => void,
  ) => subscribeMock(runId, listener),
}))

// Capture the route's `evidenceFor` so the FE-2 memo-contract test can call the real route closure
// directly (it is an internal `useCallback`, exercised here through the spy the route passes down).
// Default mock = the real view; tests that need the capture install their own implementation.
const chatViewProps = vi.fn<(props: unknown) => void>()
vi.mock('../../components/assistant-chat', async () => {
  const actual = await vi.importActual<typeof AssistantChatModule>(
    '../../components/assistant-chat',
  )
  return {
    ...actual,
    AssistantChatView: (props: { testId?: string }) => {
      chatViewProps(props)
      return actual.AssistantChatView(props as never)
    },
  }
})

import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { AiChatStreamChunk, AppSnapshot } from '../../lib/types'
import type {
  AssistantChatViewProps,
  ChatMessage,
} from '../../components/assistant-chat'
import { AssistantPage } from './index'
import {
  createShellValue,
  enableAi,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'

const assistantT = createNamespaceTranslator('en', 'assistant')

/** Capture the chunk listener so a test can drive the stream. */
let feed: ((chunk: AiChatStreamChunk) => void) | null = null
const unsubscribe = vi.fn()

beforeEach(() => {
  resetIntelligenceSurfaceHarness()
  feed = null
  unsubscribe.mockClear()
  chatViewProps.mockClear()
  subscribeMock.mockReset()
  dialogSaveMock.mockReset()
  subscribeMock.mockImplementation(
    (_runId: string, listener: (chunk: AiChatStreamChunk) => void) => {
      feed = listener
      return Promise.resolve(unsubscribe)
    },
  )
  // Drive rAF via a queue so each scheduled flush runs after the hook has recorded the frame id
  // (real rAF is async; a synchronous stub would clobber the id-reset and drop coalesced chunks).
  frameQueue = []
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    frameQueue.push(cb)
    return frameQueue.length
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frameQueue[id - 1] = () => {}
  })
})

let frameQueue: FrameRequestCallback[] = []
function flushFrames() {
  const queued = frameQueue
  frameQueue = []
  queued.forEach((cb) => cb(performance.now()))
}

/** Feed a chunk then flush the coalescing frame so the UI reflects it. */
function emit(chunk: AiChatStreamChunk) {
  if (!feed) throw new Error('not subscribed')
  act(() => {
    feed?.(chunk)
    flushFrames()
  })
}

describe('AssistantPage — active streaming chat', () => {
  test('shows the setup gate when there is no snapshot yet', async () => {
    const { snapshot } = await seedArchiveState()
    const shellValue = createShellValue(snapshot)
    shellValue.snapshot = null as unknown as AppSnapshot

    renderSurface(<AssistantPage />, {
      route: '/assistant',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(assistantT('archiveNotInitializedTitle')),
    ).toBeVisible()
  })

  test('shows the disabled state when AI is available but turned off', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    snapshot.config.ai.enabled = false

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    expect(await screen.findByText(assistantT('disabledTitle'))).toBeVisible()
    expect(screen.getByText(assistantT('disabledBody'))).toBeVisible()
    expect(screen.getByText(assistantT('emptyEyebrow'))).toBeVisible()
    expect(screen.getByText(assistantT('emptyTitle'))).toBeVisible()
    expect(screen.getByText(assistantT('emptyDescription'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: assistantT('openSettings') }),
    ).toHaveAttribute('href', '/settings#settings-ai')
  })

  test('falls back to a null system prompt when none is configured', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    snapshot.config.ai.assistantSystemPrompt = ''
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-nosys' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })
    await user.type(
      await screen.findByTestId('assistant-chat-input'),
      'no system prompt{enter}',
    )
    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(1))
    // No system message is prepended when the configured prompt is blank.
    const transcript = sendChat.mock.calls[0][0].messages
    expect(transcript.some((m) => m.role === 'system')).toBe(false)
  })

  test('shows the no-provider state when AI is on but no LLM provider is configured', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    snapshot.config.ai.llmProviderId = null
    snapshot.config.ai.llmProviders = []

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    expect(
      await screen.findByText(assistantT('chatNoProviderTitle')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: assistantT('openSettings') }),
    ).toHaveAttribute('href', '/settings#settings-ai')
  })

  test('renders the greeting and seeds a clicked prompt into the composer', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    expect(
      await screen.findByText(assistantT('chatGreetingTitle')),
    ).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-assistant-prompt-chat-prompt-1'))
    expect(screen.getByTestId('assistant-chat-input')).toHaveValue(
      assistantT('chatPrompt1'),
    )
  })

  // LAYOUT (re-review): the assistant page is a FIXED-HEIGHT chat surface — it fills the shell
  // content area and owns its OWN inner scroll (the messages list), with the composer PINNED. This
  // locks the structural containment that guarantees that contract so it cannot regress:
  //   1. the page itself never establishes a scroll (so the empty gutters of the centered column
  //      can't drag the composer off-screen via the shared `<main>` scroll),
  //   2. the messages region is the SOLE scroll surface (`flex-1 min-h-0 overflow-y-auto`),
  //   3. the composer is a `shrink-0` SIBLING of — never inside — the scrolling region.
  test('LAYOUT: messages region is the sole scroll surface and the composer is pinned outside it', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // The page fills the content area exactly and never scrolls itself — `overflow-hidden` plus a
    // bounded `h-full min-h-0` box. With this the centered (max-width) page's empty left/right
    // gutters cannot capture a page-scroll that drags the conversation, composer included, away.
    const page = await screen.findByTestId('assistant-page')
    expect(page).toHaveClass('h-full', 'min-h-0', 'overflow-hidden')

    // The messages list is the ONE scroll surface: it grows to fill (`flex-1`), is allowed to shrink
    // below its content height (`min-h-0` — without it a long chat would push the composer down and
    // overflow instead of scrolling here), and scrolls vertically (`overflow-y-auto`).
    const messages = await screen.findByTestId('assistant-chat-messages')
    expect(messages).toHaveClass('flex-1', 'min-h-0', 'overflow-y-auto')

    // The composer is PINNED: a `shrink-0` flex sibling that keeps its intrinsic height and never
    // compresses or scrolls away.
    const composer = screen.getByTestId('assistant-chat-composer')
    expect(composer).toHaveClass('shrink-0')

    // Containment: the composer is NOT inside the scrolling messages region (it must stay pinned
    // even as the messages scroll). It shares a parent with the messages list (siblings in the chat
    // view's flex column), so scrolling the messages can never move the composer.
    expect(messages.contains(composer)).toBe(false)
    expect(composer.parentElement).toBe(messages.parentElement)

    // The composer must NOT itself be a scroll surface — it has no `overflow-y-auto` of its own, so
    // the messages list is unambiguously the only scrollable region.
    expect(composer).not.toHaveClass('overflow-y-auto')
  })

  test('streams a full turn: reasoning, tool call, tokens, then done', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-42' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'when did I read about tauri?')
    await user.click(screen.getByTestId('assistant-chat-send'))

    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(1))
    expect(sendChat).toHaveBeenCalledWith(
      expect.objectContaining({
        providerId: 'llm-local',
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: 'when did I read about tauri?',
          }),
        ]),
      }),
    )
    await waitFor(() =>
      expect(subscribeMock).toHaveBeenCalledWith(
        'run-42',
        expect.any(Function),
      ),
    )

    emit({ kind: 'reasoning', text: 'looking through visits' })
    emit({ kind: 'toolCall', name: 'search_bm25', arguments: '{"q":"tauri"}' })
    emit({ kind: 'token', text: 'You first read ' })
    emit({ kind: 'token', text: 'about Tauri in April.' })

    expect(
      await screen.findByText('looking through visits'),
    ).toBeInTheDocument()
    expect(screen.getByText('Ran search_bm25')).toBeVisible()
    expect(
      screen.getByText('You first read about Tauri in April.'),
    ).toBeVisible()
    // A harness control note (review-fix M-6) is resolved to LOCALIZED copy through the route's
    // assistant translator and appended to the visible answer — never the raw English code/sentence.
    emit({ kind: 'note', code: { code: 'maxStepsReached' } })
    expect(
      await screen.findByText(/answering from the evidence gathered so far/),
    ).toBeVisible()
    // Cancel button is shown while streaming.
    expect(screen.getByTestId('assistant-chat-cancel')).toBeVisible()

    emit({ kind: 'done' })
    await waitFor(() => expect(unsubscribe).toHaveBeenCalledTimes(1))
    // Send button returns after finalize.
    expect(await screen.findByTestId('assistant-chat-send')).toBeVisible()
  })

  test('cancel calls cancelAiChat and finalizes the turn', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-99' })
    const cancelChat = vi
      .spyOn(backend, 'cancelAiChat')
      .mockResolvedValue({ cancelled: true })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'a long question{enter}')
    await waitFor(() =>
      expect(subscribeMock).toHaveBeenCalledWith(
        'run-99',
        expect.any(Function),
      ),
    )
    emit({ kind: 'token', text: 'partial...' })
    expect(await screen.findByText('partial...')).toBeVisible()

    await user.click(screen.getByTestId('assistant-chat-cancel'))
    expect(cancelChat).toHaveBeenCalledWith('run-99')
    expect(await screen.findByTestId('assistant-chat-send')).toBeVisible()
  })

  test('shows the connecting affordance until the first chunk arrives', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-conn' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'cold model question{enter}')
    await waitFor(() =>
      expect(subscribeMock).toHaveBeenCalledWith(
        'run-conn',
        expect.any(Function),
      ),
    )
    // Before any chunk: the connecting affordance is shown.
    expect(await screen.findByTestId('assistant-chat-connecting')).toBeVisible()

    emit({ kind: 'token', text: 'first token' })
    // Once the first chunk lands, the affordance disappears.
    await waitFor(() =>
      expect(
        screen.queryByTestId('assistant-chat-connecting'),
      ).not.toBeInTheDocument(),
    )
  })

  test('Try again on an errored turn re-sends the last user prompt', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-retry' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'will fail first{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'error', message: 'provider unreachable' })

    const retry = await screen.findByText(assistantT('chatTryAgain'))
    expect(retry).toBeVisible()
    await user.click(retry)

    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(2))
    const secondTranscript = sendChat.mock.calls[1][0].messages
    expect(secondTranscript).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: 'user', content: 'will fail first' }),
      ]),
    )
  })

  test('seeds the composer from the question query param', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    renderSurface(<AssistantPage />, {
      route: '/assistant?question=seeded%20question',
      snapshot,
    })
    expect(await screen.findByTestId('assistant-chat-input')).toHaveValue(
      'seeded question',
    )
  })

  test('carries prior context into a second turn', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-multi' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'first question{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'first answer' })
    emit({ kind: 'done' })
    expect(await screen.findByText('first answer')).toBeVisible()

    await user.type(
      screen.getByTestId('assistant-chat-input'),
      'second question{enter}',
    )
    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(2))

    const secondTranscript = sendChat.mock.calls[1][0].messages
    expect(secondTranscript).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: 'user', content: 'first question' }),
        expect.objectContaining({ role: 'assistant', content: 'first answer' }),
        expect.objectContaining({ role: 'user', content: 'second question' }),
      ]),
    )
  })

  test('persists a finished turn to the conversation store, then refreshes the list', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-save' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    const saveConversation = vi
      .spyOn(backend, 'saveAiConversation')
      .mockResolvedValue({
        id: 'conv-saved',
        title: 'persist this',
        providerId: 'llm-local',
        createdAt: '2026-06-20T12:00:00Z',
        updatedAt: '2026-06-20T12:00:00Z',
        messageCount: 2,
      })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'persist this{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'an answer worth saving' })
    emit({ kind: 'done' })

    // The finalize microtask persists the transcript with both turns.
    await waitFor(() => expect(saveConversation).toHaveBeenCalledTimes(1))
    const saved = saveConversation.mock.calls[0][0]
    expect(saved.messages).toHaveLength(2)
    expect(saved.messages[0]).toMatchObject({
      role: 'user',
      content: 'persist this',
    })
    expect(saved.messages[1]).toMatchObject({
      role: 'assistant',
      content: 'an answer worth saving',
      status: 'done',
    })
    expect(saved.providerId).toBe('llm-local')
  })

  test('opens a saved conversation from the explorer and hydrates the transcript', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [
        {
          id: 'conv-past',
          title: 'A past conversation',
          providerId: 'llm-local',
          createdAt: '2026-06-19T09:00:00Z',
          updatedAt: '2026-06-19T09:30:00Z',
          messageCount: 2,
        },
      ],
    })
    vi.spyOn(backend, 'loadAiConversation').mockResolvedValue({
      id: 'conv-past',
      title: 'A past conversation',
      providerId: 'llm-local',
      createdAt: '2026-06-19T09:00:00Z',
      updatedAt: '2026-06-19T09:30:00Z',
      messageCount: 2,
      messages: [
        {
          id: 'pm1',
          role: 'user',
          content: 'old question',
          reasoning: null,
          toolCallsJson: null,
          status: null,
        },
        {
          id: 'pm2',
          role: 'assistant',
          content: 'old hydrated answer',
          reasoning: null,
          toolCallsJson:
            '[{"id":"t1","name":"search_history","arguments":"{}","status":"success","callId":"c1","result":"[10] https://tauri.app/"}]',
          status: 'done',
          // W-AI-7 WU-7: the durable trace reconstructed on reopen — the run's pinned evidence rows
          // (with the canonical_url star key) + token usage, so the reopened turn renders the SAME
          // evidence + stars + footer the live turn streamed.
          citations: [
            {
              historyId: 10,
              profileId: '',
              url: 'https://tauri.app/',
              title: 'Tauri',
              visitedAt: '2026-04-01T00:00:00Z',
              score: 0.9,
              canonicalUrl: 'https://tauri.app/',
            },
          ],
          usage: { promptTokens: 128, completionTokens: 64 },
        },
      ],
    })
    // The reopened evidence row hydrates its star status; mark it starred so the reconstructed star
    // toggle resolves is_starred by canonicalUrl.
    vi.spyOn(backend, 'getStarStatus').mockResolvedValue({
      'https://tauri.app/': true,
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // Open the (collapsed) history drawer.
    await user.click(await screen.findByTestId('assistant-history-doorway'))
    // The list row appears once the list load resolves.
    await user.click(
      await screen.findByRole('button', {
        name: 'Open conversation: A past conversation',
      }),
    )

    // The hydrated transcript replaces the empty greeting.
    expect(await screen.findByText('old question')).toBeVisible()
    expect(screen.getByText('old hydrated answer')).toBeVisible()
    // The reconstructed tool-use timeline renders via the same tool-call-block as the live path.
    expect(screen.getByTestId('assistant-tools-pm2')).toBeVisible()
    // The reconstructed evidence row renders (keyed by the citation's history id) and is starrable
    // by its canonical url — identical to the live turn.
    expect(
      await screen.findByTestId('paper-assistant-evidence-cite-10'),
    ).toBeVisible()
    expect(
      screen.getByTestId('paper-assistant-evidence-star-cite-10'),
    ).toBeVisible()
    // The reconstructed per-turn usage footer renders.
    expect(screen.getByTestId('assistant-usage-pm2')).toBeVisible()
  })

  test('opening a conversation that vanished leaves the transcript untouched', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [
        {
          id: 'conv-gone',
          title: 'Vanished chat',
          providerId: 'llm-local',
          createdAt: '2026-06-19T09:00:00Z',
          updatedAt: '2026-06-19T09:30:00Z',
          messageCount: 1,
        },
      ],
    })
    // The load resolves to null (deleted elsewhere): the chat hook must not be reset/hydrated.
    vi.spyOn(backend, 'loadAiConversation').mockResolvedValue(null)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    await user.click(await screen.findByTestId('assistant-history-doorway'))
    await user.click(
      await screen.findByRole('button', {
        name: 'Open conversation: Vanished chat',
      }),
    )
    // Still on the empty greeting (the missing conversation produced no hydration).
    expect(
      await screen.findByText(assistantT('chatGreetingTitle')),
    ).toBeVisible()
  })

  test('New chat clears the transcript back to the greeting', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-new' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    vi.spyOn(backend, 'saveAiConversation').mockResolvedValue({
      id: 'conv-x',
      title: 'something',
      providerId: 'llm-local',
      createdAt: '2026-06-20T12:00:00Z',
      updatedAt: '2026-06-20T12:00:00Z',
      messageCount: 2,
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'something{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'an answer' })
    emit({ kind: 'done' })
    expect(await screen.findByText('an answer')).toBeVisible()

    await user.click(await screen.findByTestId('assistant-history-doorway'))
    await user.click(screen.getByTestId('assistant-chat-history-new-chat'))

    // Back to the greeting; the prior answer is gone.
    expect(
      await screen.findByText(assistantT('chatGreetingTitle')),
    ).toBeVisible()
    expect(screen.queryByText('an answer')).not.toBeInTheDocument()
  })

  test('deletes a saved conversation from the explorer with confirmation', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [
        {
          id: 'conv-del',
          title: 'Deletable chat',
          providerId: 'llm-local',
          createdAt: '2026-06-19T09:00:00Z',
          updatedAt: '2026-06-19T09:30:00Z',
          messageCount: 1,
        },
      ],
    })
    const deleteConversation = vi
      .spyOn(backend, 'deleteAiConversation')
      .mockResolvedValue({ deleted: true })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    await user.click(await screen.findByTestId('assistant-history-doorway'))
    await user.click(
      await screen.findByTestId('assistant-chat-history-row-conv-del-delete'),
    )
    await user.click(
      screen.getByTestId('assistant-chat-history-row-conv-del-confirm-delete'),
    )
    expect(deleteConversation).toHaveBeenCalledWith('conv-del')
  })

  test('runs the agent path: threads toolsEnabled + the assistant messageId into sendAiChat', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-agent' })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'what did I read about tauri?{enter}')
    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(1))
    const request = sendChat.mock.calls[0][0]
    // The history assistant runs WITH tools by default (the agent harness answers over history).
    expect(request.toolsEnabled).toBe(true)
    // A messageId links the durable agent trace to this turn (no conversation saved yet → undefined).
    expect(typeof request.messageId).toBe('string')
    expect(request.conversationId == null).toBe(true)
  })

  test('renders cited evidence rows, hydrates their star status, and stars by canonicalUrl', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-cite' })
    const getStarStatus = vi
      .spyOn(backend, 'getStarStatus')
      .mockResolvedValue({ 'https://a.example/x': false })
    const setStar = vi.spyOn(backend, 'setStar').mockResolvedValue(undefined)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'cite something{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'Here is the answer.' })
    emit({
      kind: 'citations',
      citations: [
        {
          historyId: 7,
          profileId: 'p',
          url: 'https://a.example/x',
          title: 'Cited page',
          visitedAt: '2026-04-05T10:00:00Z',
          score: 0.9,
          canonicalUrl: 'https://a.example/x',
        },
      ],
    })
    emit({ kind: 'done' })

    // The evidence row renders (title + the mono date column = first 10 chars of visitedAt).
    expect(await screen.findByText('Cited page')).toBeVisible()
    expect(screen.getByText('2026-04-05')).toBeVisible()
    // Star status hydrates for just this row's canonical key.
    await waitFor(() =>
      expect(getStarStatus).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKeys: ['https://a.example/x'],
      }),
    )
    // Starring writes through keyed by the canonical url (the W-STAR key).
    await user.click(screen.getByTestId('paper-assistant-evidence-star-cite-7'))
    expect(setStar).toHaveBeenCalledWith({
      entityKind: 'url',
      entityKey: 'https://a.example/x',
    })
  })

  test('clicking an evidence row deep-links to Explorer search (transparency contract)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-link' })
    vi.spyOn(backend, 'getStarStatus').mockResolvedValue({})

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'link it{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'answer' })
    emit({
      kind: 'citations',
      // A url that does NOT parse exercises the domain-label fallback, and a NULL title exercises
      // the `title ?? url` fallback in citationsToEvidence.
      citations: [
        {
          historyId: 3,
          profileId: 'p',
          url: 'not a url',
          title: null,
          visitedAt: '2026-03-03T00:00:00Z',
          canonicalUrl: null,
        },
      ],
    })
    emit({ kind: 'done' })

    const row = await screen.findByTestId('paper-assistant-evidence-cite-3')
    // The fallback uses the raw url as both the title and the domain label (no throw, no crash).
    expect(row).toHaveTextContent('not a url')
    // Clicking routes to Explorer search (the navigate call is exercised).
    await user.click(row)
    // No star toggle for a citation without a canonical url.
    expect(
      screen.queryByTestId('paper-assistant-evidence-star-cite-3'),
    ).not.toBeInTheDocument()
  })

  test('renames a saved conversation from the explorer', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [
        {
          id: 'conv-rn',
          title: 'Old title',
          providerId: 'llm-local',
          createdAt: '2026-06-19T09:00:00Z',
          updatedAt: '2026-06-19T09:30:00Z',
          messageCount: 1,
        },
      ],
    })
    const renameConversation = vi
      .spyOn(backend, 'renameAiConversation')
      .mockResolvedValue({
        id: 'conv-rn',
        title: 'New title',
        providerId: 'llm-local',
        createdAt: '2026-06-19T09:00:00Z',
        updatedAt: '2026-06-19T09:35:00Z',
        messageCount: 1,
      })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    await user.click(await screen.findByTestId('assistant-history-doorway'))
    await user.click(
      await screen.findByTestId('assistant-chat-history-row-conv-rn-rename'),
    )
    const input = await screen.findByTestId(
      'assistant-chat-history-row-conv-rn-rename-input',
    )
    await user.clear(input)
    await user.type(input, 'New title')
    await user.click(
      screen.getByTestId('assistant-chat-history-row-conv-rn-rename-save'),
    )
    expect(renameConversation).toHaveBeenCalledWith({
      id: 'conv-rn',
      title: 'New title',
    })
  })

  // FE-2: `evidenceFor` runs inside `messages.map(...)` on every render and feeds `memo`'d ChatRow /
  // AssistantTurn rows. A fresh `evidence` array identity per call would defeat the memo, re-rendering
  // every on-screen finalized cited turn on every streaming frame. The route caches the projection on
  // the stable (finalized) message object via a WeakMap; this locks that contract so it can't regress.
  test('evidenceFor caches the projection per message object (memo-stable identity)', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'getStarStatus').mockResolvedValue({})

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // The mock records every AssistantChatView render; grab the live `evidenceFor` closure.
    await waitFor(() => expect(chatViewProps).toHaveBeenCalled())
    const evidenceFor = (
      chatViewProps.mock.calls.at(-1)![0] as AssistantChatViewProps
    ).evidenceFor!
    expect(typeof evidenceFor).toBe('function')

    // A finalized assistant turn that HAS a citation. The hook returns this exact object unchanged
    // across frames once finalized, so the WeakMap keyed on it must hand back ONE array reference.
    const cited: ChatMessage = {
      id: 'm-cited',
      role: 'assistant',
      content: 'an answer',
      status: 'done',
      citations: [
        {
          historyId: 11,
          profileId: 'p',
          url: 'https://example.com/page',
          title: 'A cited page',
          visitedAt: '2026-05-01T12:00:00Z',
          score: 0.9,
          canonicalUrl: 'https://example.com/page',
        },
      ],
    }

    // Cache-miss then cache-hit on the SAME object: identical array reference (the memo contract).
    const first = evidenceFor(cited)
    const second = evidenceFor(cited)
    expect(first).toBeDefined()
    expect(second).toBe(first)

    // A DIFFERENT message object (e.g. the actively-streaming turn, a fresh ref each flush) must
    // re-project — a distinct array — so streaming content is never stale.
    const otherCited: ChatMessage = { ...cited, id: 'm-cited-2' }
    const third = evidenceFor(otherCited)
    expect(third).not.toBe(first)
    expect(third).toEqual(first)

    // No-citations turn: stable `undefined`, cached so repeated calls stay referentially equal.
    const plain: ChatMessage = {
      id: 'm-plain',
      role: 'assistant',
      content: 'no sources',
      status: 'done',
    }
    expect(evidenceFor(plain)).toBeUndefined()
    expect(evidenceFor(plain)).toBeUndefined()
  })

  // CH-1: a discoverable doorway to past conversations in the chat header (not just the bare drawer
  // toggle). The header button must open the same history drawer and stay accessible.
  test('CH-1: a labeled header doorway opens the conversation drawer', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const doorway = await screen.findByTestId('assistant-history-doorway')
    // Action-oriented + accessible: the aria-label and title align on "Show conversations" (a verb,
    // not the bare noun), and the button reports its collapsed state.
    expect(doorway).toHaveAccessibleName(assistantT('historyOpen'))
    expect(doorway).toHaveAttribute('title', assistantT('historyOpen'))
    expect(doorway).toHaveAttribute('aria-expanded', 'false')
    expect(doorway).toHaveTextContent(assistantT('historyDoorway'))

    // The drawer starts collapsed; the doorway opens it (the panel becomes visible).
    expect(
      screen.queryByTestId('assistant-chat-history'),
    ).not.toBeInTheDocument()
    await user.click(doorway)
    expect(await screen.findByTestId('assistant-chat-history')).toBeVisible()
    expect(doorway).toHaveAttribute('aria-expanded', 'true')

    // Clicking again collapses it (the existing toggle behavior is preserved).
    await user.click(doorway)
    await waitFor(() =>
      expect(
        screen.queryByTestId('assistant-chat-history'),
      ).not.toBeInTheDocument(),
    )
  })

  // C1-2: exactly ONE open affordance. The header doorway drives the drawer (externalOpenControl),
  // so the drawer must NOT also render its own collapsed icon-only open-button while closed. The
  // in-drawer close button still works once the drawer is open.
  test('C1-2: only the header doorway opens the drawer (no duplicate collapsed toggle)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // The header doorway exists; the drawer's own collapsed open-button does NOT (suppressed).
    expect(
      await screen.findByTestId('assistant-history-doorway'),
    ).toBeInTheDocument()
    expect(
      screen.queryByTestId('assistant-chat-history-open'),
    ).not.toBeInTheDocument()

    // Open via the doorway, then the in-drawer close button collapses it (close button preserved).
    await user.click(screen.getByTestId('assistant-history-doorway'))
    expect(await screen.findByTestId('assistant-chat-history')).toBeVisible()
    await user.click(screen.getByTestId('assistant-chat-history-close'))
    await waitFor(() =>
      expect(
        screen.queryByTestId('assistant-chat-history'),
      ).not.toBeInTheDocument(),
    )
    // After closing, the collapsed open-button is STILL suppressed — only the doorway reopens it.
    expect(
      screen.queryByTestId('assistant-chat-history-open'),
    ).not.toBeInTheDocument()
  })

  // CH-2: an honest, transient "saved" signal after a real successful persist — visible badge plus a
  // polite aria-live announcement, with no heavy toast system.
  test('CH-2: shows a transient saved signal only after a successful persist', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-saved' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    vi.spyOn(backend, 'saveAiConversation').mockResolvedValue({
      id: 'conv-saved',
      title: 'saved one',
      providerId: 'llm-local',
      createdAt: '2026-06-20T12:00:00Z',
      updatedAt: '2026-06-20T12:00:00Z',
      messageCount: 2,
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // No saved signal before any turn lands.
    expect(
      screen.queryByTestId('assistant-saved-signal'),
    ).not.toBeInTheDocument()

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'save me{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'kept answer' })
    emit({ kind: 'done' })

    // After the real save resolves, the transient badge + the aria-live announcement appear.
    const signal = await screen.findByTestId('assistant-saved-signal')
    expect(signal).toHaveTextContent(assistantT('chatSavedAnnouncement'))
    expect(screen.getByTestId('assistant-saved-announcer')).toHaveTextContent(
      assistantT('chatSavedAnnouncement'),
    )

    // The signal is transient: after its window the badge + announcement clear on their own
    // (proving the auto-clear timer callback runs, not just the arm path).
    await waitFor(
      () =>
        expect(
          screen.queryByTestId('assistant-saved-signal'),
        ).not.toBeInTheDocument(),
      { timeout: 3000 },
    )
    expect(
      screen.getByTestId('assistant-saved-announcer'),
    ).toBeEmptyDOMElement()
  })

  test('CH-2: does NOT show the saved signal when the persist fails', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-fail' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    const saveConversation = vi
      .spyOn(backend, 'saveAiConversation')
      .mockRejectedValue(new Error('disk full'))

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'try save{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'answer' })
    emit({ kind: 'done' })

    await waitFor(() => expect(saveConversation).toHaveBeenCalledTimes(1))
    // The save rejected, so the UI must never claim "saved".
    await Promise.resolve()
    expect(
      screen.queryByTestId('assistant-saved-signal'),
    ).not.toBeInTheDocument()
  })

  // CH-3: an honest "opening…" indicator on the chat canvas while a reopened conversation loads.
  test('CH-3: shows an opening indicator on the canvas while a reopened conversation loads', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'getStarStatus').mockResolvedValue({})
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [
        {
          id: 'conv-slow',
          title: 'A slow conversation',
          providerId: 'llm-local',
          createdAt: '2026-06-19T09:00:00Z',
          updatedAt: '2026-06-19T09:30:00Z',
          messageCount: 2,
        },
      ],
    })
    // A deferred load lets us observe the in-flight "opening…" state on the canvas.
    type LoadResult = Awaited<ReturnType<typeof backend.loadAiConversation>>
    let resolveLoad: ((detail: LoadResult) => void) | null = null
    const loadPromise = new Promise<LoadResult>((resolve) => {
      resolveLoad = resolve
    })
    vi.spyOn(backend, 'loadAiConversation').mockReturnValue(loadPromise)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    await user.click(await screen.findByTestId('assistant-history-doorway'))
    await user.click(
      await screen.findByRole('button', {
        name: 'Open conversation: A slow conversation',
      }),
    )

    // While the load is in flight the canvas shows the honest opening indicator.
    expect(
      await screen.findByTestId('assistant-opening-conversation'),
    ).toHaveTextContent(assistantT('chatOpeningConversation'))

    // Resolve the load → the indicator clears.
    await act(async () => {
      resolveLoad?.({
        id: 'conv-slow',
        title: 'A slow conversation',
        providerId: 'llm-local',
        createdAt: '2026-06-19T09:00:00Z',
        updatedAt: '2026-06-19T09:30:00Z',
        messageCount: 1,
        messages: [
          {
            id: 'sm1',
            role: 'user',
            content: 'reopened question',
            reasoning: null,
            toolCallsJson: null,
            status: null,
          },
        ],
      })
      // Let the open-conversation promise chain (load → reset → finally) settle within act().
      await loadPromise
    })
    await waitFor(() =>
      expect(
        screen.queryByTestId('assistant-opening-conversation'),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByText('reopened question')).toBeVisible()
  })

  // M-9 / ASSIST-2: Regenerate on the latest completed turn replaces the answer IN PLACE — it does
  // NOT append a duplicate question or leave the stale answer behind. This drives the real
  // `useAiChatStream` end-to-end and asserts the END STATE (rendered transcript + the persisted
  // transcript shape), not merely that a second `sendAiChat` fired.
  test('M-9: Regenerate replaces the latest answer in place (no duplicate question, old answer gone)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const sendChat = vi
      .spyOn(backend, 'sendAiChat')
      .mockResolvedValue({ runId: 'run-regen' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    const saveConversation = vi
      .spyOn(backend, 'saveAiConversation')
      .mockResolvedValue({
        id: 'conv-regen',
        title: 'regen',
        providerId: 'llm-local',
        createdAt: '2026-06-20T12:00:00Z',
        updatedAt: '2026-06-20T12:00:00Z',
        messageCount: 2,
      })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'first question{enter}')
    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'the first answer' })
    emit({ kind: 'done' })

    // The first turn persisted exactly one Q + one A.
    await waitFor(() => expect(saveConversation).toHaveBeenCalledTimes(1))
    expect(screen.getByText('first question')).toBeVisible()
    expect(screen.getByText('the first answer')).toBeVisible()

    // The per-message actions appear on the finalized answer.
    const copyButton = await screen.findByLabelText(
      assistantT('chatCopyAnswer'),
    )
    expect(copyButton).toBeVisible()
    const regenerate = screen.getByLabelText(assistantT('chatRegenerateAnswer'))
    expect(regenerate).toBeVisible()

    // Regenerate the latest completed turn: a fresh run re-answers the SAME existing user turn.
    await user.click(regenerate)
    await waitFor(() => expect(sendChat).toHaveBeenCalledTimes(2))

    // The re-run's model transcript ends with the SAME user question — and carries NO duplicate of
    // it (exactly one user message in the request transcript).
    const secondTurn = sendChat.mock.calls[1][0]
    expect(secondTurn.messages.at(-1)).toMatchObject({
      role: 'user',
      content: 'first question',
    })
    expect(
      secondTurn.messages.filter(
        (m) => m.role === 'user' && m.content === 'first question',
      ),
    ).toHaveLength(1)

    // Stream the regenerated answer and finalize it.
    emit({ kind: 'token', text: 'a regenerated answer' })
    emit({ kind: 'done' })

    // END STATE in the rendered transcript: the question is NOT duplicated, the old answer is gone,
    // and only the regenerated answer remains.
    expect(screen.getAllByText('first question')).toHaveLength(1)
    expect(screen.getByText('a regenerated answer')).toBeVisible()
    expect(screen.queryByText('the first answer')).not.toBeInTheDocument()

    // The PERSISTED transcript reflects the replacement: still exactly one user turn + one assistant
    // turn (the regenerated answer), never a duplicated question or a stale answer.
    await waitFor(() => expect(saveConversation).toHaveBeenCalledTimes(2))
    const firstPersisted = saveConversation.mock.calls[0][0]
    const persisted = saveConversation.mock.calls[1][0]
    // Same conversation row — the regeneration replaces the transcript in place, not a new chat.
    expect(persisted.id).toBe(firstPersisted.id)
    expect(persisted.messages).toHaveLength(2)
    expect(persisted.messages.filter((m) => m.role === 'user')).toHaveLength(1)
    expect(persisted.messages[0]).toMatchObject({
      role: 'user',
      content: 'first question',
    })
    expect(persisted.messages[1]).toMatchObject({
      role: 'assistant',
      content: 'a regenerated answer',
      status: 'done',
    })
  })

  // C1-1/F1: the saved signal fires only on a conversation's FIRST persist. A SECOND turn of the
  // SAME conversation still saves (durable) but must NOT re-show the badge/announcement — otherwise
  // the per-turn ceremony stacks on each answer's "Answer complete" milestone (the a11y chatter the
  // guardrails reject). We wait for the first signal to auto-clear, then prove a second turn of the
  // same conversation leaves it cleared.
  test('C1-1: does NOT re-show the saved signal on a subsequent turn of the same conversation', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-quiet' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    vi.spyOn(backend, 'saveAiConversation').mockResolvedValue({
      id: 'conv-quiet',
      title: 'quiet',
      providerId: 'llm-local',
      createdAt: '2026-06-20T12:00:00Z',
      updatedAt: '2026-06-20T12:00:00Z',
      messageCount: 4,
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    // First turn mints the conversation → the signal fires once.
    await user.type(input, 'first save{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'answer one' })
    emit({ kind: 'done' })
    await screen.findByTestId('assistant-saved-signal')
    // Let the first signal's window elapse so it clears on its own.
    await waitFor(
      () =>
        expect(
          screen.queryByTestId('assistant-saved-signal'),
        ).not.toBeInTheDocument(),
      { timeout: 3000 },
    )

    // A second turn of the SAME conversation re-saves silently — the badge stays absent.
    await user.type(input, 'second save{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(2))
    emit({ kind: 'token', text: 'answer two' })
    emit({ kind: 'done' })
    await waitFor(() =>
      expect(backend.saveAiConversation).toHaveBeenCalledTimes(2),
    )
    // The save landed (durable), but the first-persist-only signal does not re-fire.
    await Promise.resolve()
    expect(
      screen.queryByTestId('assistant-saved-signal'),
    ).not.toBeInTheDocument()
    expect(
      screen.getByTestId('assistant-saved-announcer'),
    ).toBeEmptyDOMElement()
  })

  // C1-1: a NEW conversation (after New chat resets the active id) re-arms the signal. When the
  // second mint lands while the first 2200ms window is still pending, `handleSaved` clears the
  // pending timer first (the truthy `if (savedTimerRef.current)` branch) then re-shows — so the
  // badge is visible for the new conversation too.
  test('C1-1: a NEW conversation re-arms the saved signal (clears the pending timer)', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-rearm' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    vi.spyOn(backend, 'saveAiConversation').mockResolvedValue({
      id: 'conv-rearm',
      title: 'rearm',
      providerId: 'llm-local',
      createdAt: '2026-06-20T12:00:00Z',
      updatedAt: '2026-06-20T12:00:00Z',
      messageCount: 2,
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'first save{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'answer one' })
    emit({ kind: 'done' })
    await screen.findByTestId('assistant-saved-signal')

    // Start a NEW chat (resets the active id), then send again while the first 2200ms window is
    // still open → the next persist mints a fresh id → the signal re-arms (timer cleared + re-shown).
    await user.click(screen.getByTestId('assistant-history-doorway'))
    await user.click(
      await screen.findByTestId('assistant-chat-history-new-chat'),
    )
    await user.type(
      screen.getByTestId('assistant-chat-input'),
      'second save{enter}',
    )
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(2))
    emit({ kind: 'token', text: 'answer two' })
    emit({ kind: 'done' })
    // The badge is visible for the new conversation (re-armed via the clear-pending-timer branch).
    expect(
      await screen.findByTestId('assistant-saved-signal'),
    ).toBeInTheDocument()
  })

  // CH-2: unmounting with a pending saved-signal timer must clear it (no late setState / no throw).
  test('CH-2: clears the pending saved-signal timer on unmount', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-unmount' })
    vi.spyOn(backend, 'listAiConversations').mockResolvedValue({
      conversations: [],
    })
    vi.spyOn(backend, 'saveAiConversation').mockResolvedValue({
      id: 'conv-unmount',
      title: 'unmount',
      providerId: 'llm-local',
      createdAt: '2026-06-20T12:00:00Z',
      updatedAt: '2026-06-20T12:00:00Z',
      messageCount: 2,
    })

    const view = renderSurface(<AssistantPage />, {
      route: '/assistant',
      snapshot,
    })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'save then leave{enter}')
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'kept' })
    emit({ kind: 'done' })
    // Wait for the real save to arm the signal timer, then unmount with it still pending.
    await screen.findByTestId('assistant-saved-signal')
    expect(() => view.unmount()).not.toThrow()
  })

  test('hides the export affordance until there is a conversation, then exports as Markdown', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-export' })
    const writeFile = vi
      .spyOn(backend, 'exportConversationFile')
      .mockResolvedValue(64)
    dialogSaveMock.mockResolvedValue('/tmp/pathkeep-conversation-2026.md')

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // Empty transcript → the export trigger is disabled (honest: nothing to export).
    expect(await screen.findByTestId('assistant-export-trigger')).toBeDisabled()

    // Produce a one-turn conversation.
    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'when did I read about tauri?')
    await user.click(screen.getByTestId('assistant-chat-send'))
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'You read about Tauri in April.' })
    emit({ kind: 'citations', citations: [] })
    emit({ kind: 'done' })

    // Now there are messages → the export trigger is enabled.
    await waitFor(() =>
      expect(screen.getByTestId('assistant-export-trigger')).toBeEnabled(),
    )

    await user.click(screen.getByTestId('assistant-export-trigger'))
    await user.click(await screen.findByTestId('assistant-export-markdown'))

    // The save dialog was offered the Markdown default name + extension filter.
    await waitFor(() => expect(dialogSaveMock).toHaveBeenCalledTimes(1))
    const saveArgs = dialogSaveMock.mock.calls[0][0] as {
      defaultPath: string
      filters: { extensions: string[] }[]
    }
    expect(saveArgs.defaultPath).toMatch(/^pathkeep-conversation-.*\.md$/)
    expect(saveArgs.filters[0].extensions).toEqual(['md'])

    // The serialized Markdown transcript was written to the chosen path.
    await waitFor(() => expect(writeFile).toHaveBeenCalledTimes(1))
    const [targetPath, contents] = writeFile.mock.calls[0]
    expect(targetPath).toBe('/tmp/pathkeep-conversation-2026.md')
    expect(contents).toContain('# PathKeep conversation')
    expect(contents).toContain('## You')
    expect(contents).toContain('when did I read about tauri?')
    expect(contents).toContain('## Assistant')
    expect(contents).toContain('You read about Tauri in April.')

    // Success is announced.
    await waitFor(() =>
      expect(screen.getByTestId('assistant-export-status')).toHaveTextContent(
        assistantT('exportSuccess'),
      ),
    )
  })

  test('exports as JSON and stays silent when the save dialog is cancelled', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    vi.spyOn(backend, 'sendAiChat').mockResolvedValue({ runId: 'run-json' })
    const writeFile = vi
      .spyOn(backend, 'exportConversationFile')
      .mockResolvedValue(32)
    // First a JSON export with a real path, then a cancelled (null) dialog.
    dialogSaveMock
      .mockResolvedValueOnce('/tmp/pathkeep-conversation-2026.json')
      .mockResolvedValueOnce(null)

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    const input = await screen.findByTestId('assistant-chat-input')
    await user.type(input, 'summary please')
    await user.click(screen.getByTestId('assistant-chat-send'))
    await waitFor(() => expect(subscribeMock).toHaveBeenCalledTimes(1))
    emit({ kind: 'token', text: 'Here is a summary.' })
    emit({ kind: 'done' })
    await waitFor(() =>
      expect(screen.getByTestId('assistant-export-trigger')).toBeEnabled(),
    )

    // JSON export: lossless document written to the chosen .json path.
    await user.click(screen.getByTestId('assistant-export-trigger'))
    await user.click(await screen.findByTestId('assistant-export-json'))
    await waitFor(() => expect(writeFile).toHaveBeenCalledTimes(1))
    const [, jsonContents] = writeFile.mock.calls[0]
    const parsed = JSON.parse(jsonContents) as {
      messages: { role: string; content: string }[]
    }
    expect(parsed.messages[0]).toMatchObject({
      role: 'user',
      content: 'summary please',
    })
    expect(parsed.messages[1]).toMatchObject({
      role: 'assistant',
      content: 'Here is a summary.',
    })

    // Cancelled save dialog: no second write, no success claim.
    await user.click(screen.getByTestId('assistant-export-trigger'))
    await user.click(await screen.findByTestId('assistant-export-markdown'))
    await waitFor(() => expect(dialogSaveMock).toHaveBeenCalledTimes(2))
    expect(writeFile).toHaveBeenCalledTimes(1)
  })
})
