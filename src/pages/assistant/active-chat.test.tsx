/**
 * @file active-chat.test.tsx
 * @description Covers the active streaming-chat surface of the rebuilt Assistant route — the
 *              W-AI-2 marquee. The deferred / setup / locked gates live in `index.test.tsx`
 *              (which runs against the real release-capabilities `false` flag); this file flips
 *              the flag to `true` so the live chat branches render.
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

import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { AiChatStreamChunk, AppSnapshot } from '../../lib/types'
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
  subscribeMock.mockReset()
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
    ).toHaveAttribute('href', '/settings')
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
    ).toHaveAttribute('href', '/settings')
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
          toolCallsJson: null,
          status: 'done',
        },
      ],
    })

    renderSurface(<AssistantPage />, { route: '/assistant', snapshot })

    // Open the (collapsed) history drawer.
    await user.click(await screen.findByTestId('assistant-chat-history-open'))
    // The list row appears once the list load resolves.
    await user.click(
      await screen.findByRole('button', {
        name: 'Open conversation: A past conversation',
      }),
    )

    // The hydrated transcript replaces the empty greeting.
    expect(await screen.findByText('old question')).toBeVisible()
    expect(screen.getByText('old hydrated answer')).toBeVisible()
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

    await user.click(await screen.findByTestId('assistant-chat-history-open'))
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

    await user.click(await screen.findByTestId('assistant-chat-history-open'))
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

    await user.click(await screen.findByTestId('assistant-chat-history-open'))
    await user.click(
      await screen.findByTestId('assistant-chat-history-row-conv-del-delete'),
    )
    await user.click(
      screen.getByTestId('assistant-chat-history-row-conv-del-confirm-delete'),
    )
    expect(deleteConversation).toHaveBeenCalledWith('conv-del')
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

    await user.click(await screen.findByTestId('assistant-chat-history-open'))
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
})
