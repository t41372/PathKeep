/**
 * @file use-chat-history.test.ts
 * @description Coverage for the conversation-persistence controller: list load, save-on-finalize
 *              (mint id + refresh), open (hydrate / not-found / error), delete (active clear), new
 *              chat, retry, the dormant (disabled) path, and message<->agent projection edges.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { useChatHistory, type ChatHistoryBackend } from './use-chat-history'
import type { ChatMessage } from './use-ai-chat-stream'
import type {
  AgentConversationDetail,
  AgentConversationSummary,
} from '../../lib/types'

function summary(
  overrides: Partial<AgentConversationSummary> = {},
): AgentConversationSummary {
  return {
    id: 'conv-1',
    title: 'A chat',
    providerId: 'llm-local',
    createdAt: '2026-06-20T10:00:00Z',
    updatedAt: '2026-06-20T10:05:00Z',
    messageCount: 2,
    ...overrides,
  }
}

function makeBackend(overrides: Partial<ChatHistoryBackend> = {}) {
  const backend = {
    listConversations: vi
      .fn()
      .mockResolvedValue({ conversations: [summary()] }),
    saveConversation: vi.fn().mockResolvedValue(summary()),
    loadConversation: vi.fn().mockResolvedValue(null),
    deleteConversation: vi.fn().mockResolvedValue({ deleted: true }),
    renameConversation: vi.fn().mockResolvedValue(summary()),
    ...overrides,
  }
  // Returned with the mock types intact (no `satisfies`, which widens to the plain function type)
  // so tests can read `.mock.calls`; the object still structurally matches `ChatHistoryBackend`.
  return backend as typeof backend & ChatHistoryBackend
}

const userMessage: ChatMessage = {
  id: 'm1',
  role: 'user',
  content: 'hello',
}
const assistantMessage: ChatMessage = {
  id: 'm2',
  role: 'assistant',
  content: 'hi there',
  reasoning: 'thinking',
  toolCalls: [{ id: 't1', name: 'search_bm25', arguments: '{}' }],
  status: 'done',
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('useChatHistory', () => {
  test('loads the conversation list on mount when enabled', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(result.current.conversations).toHaveLength(1))
    expect(backend.listConversations).toHaveBeenCalledTimes(1)
    expect(result.current.loading).toBe(false)
    expect(result.current.error).toBe(false)
  })

  test('stays dormant and does not list when disabled', () => {
    const backend = makeBackend()
    renderHook(() => useChatHistory({ backend, enabled: false }))
    expect(backend.listConversations).not.toHaveBeenCalled()
  })

  test('surfaces an error when the list load fails, and retries via refresh', async () => {
    const listConversations = vi
      .fn()
      .mockRejectedValueOnce(new Error('disk busy'))
      .mockResolvedValueOnce({ conversations: [summary()] })
    const backend = makeBackend({ listConversations })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(result.current.error).toBe(true))

    act(() => result.current.refresh())
    await waitFor(() => expect(result.current.conversations).toHaveLength(1))
    expect(result.current.error).toBe(false)
  })

  test('persistTurn mints a conversation id, saves, and refreshes', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() =>
      useChatHistory({ backend, providerId: 'llm-local' }),
    )
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())

    act(() => result.current.persistTurn([userMessage, assistantMessage]))
    await waitFor(() =>
      expect(backend.saveConversation).toHaveBeenCalledTimes(1),
    )

    const request = vi.mocked(backend.saveConversation).mock.calls[0][0]
    expect(request.id).toMatch(/^conv-/)
    expect(request.providerId).toBe('llm-local')
    expect(request.messages).toHaveLength(2)
    // Tool calls are serialized to JSON; reasoning + status are carried through.
    expect(request.messages[1].toolCallsJson).toContain('search_bm25')
    expect(request.messages[1].reasoning).toBe('thinking')
    expect(request.messages[1].status).toBe('done')
    // The active id is now set to the minted id.
    expect(result.current.activeId).toBe(request.id)
    // The save triggers a list refresh (initial + post-save).
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(2),
    )
  })

  test('persistTurn ignores an empty transcript', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())
    act(() => result.current.persistTurn([]))
    expect(backend.saveConversation).not.toHaveBeenCalled()
  })

  test('persistTurn reuses an existing active id and swallows save failures', async () => {
    const detail: AgentConversationDetail = {
      ...summary({ id: 'conv-existing' }),
      messages: [
        {
          id: 'm1',
          role: 'user',
          content: 'old',
          reasoning: null,
          toolCallsJson: null,
          status: null,
        },
      ],
    }
    const saveConversation = vi
      .fn()
      .mockRejectedValue(new Error('write failed'))
    const backend = makeBackend({
      loadConversation: vi.fn().mockResolvedValue(detail),
      saveConversation,
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())

    // Open sets the active id; the next save reuses it.
    await act(async () => {
      await result.current.openConversation('conv-existing')
    })
    expect(result.current.activeId).toBe('conv-existing')

    act(() => result.current.persistTurn([userMessage]))
    await waitFor(() => expect(saveConversation).toHaveBeenCalledTimes(1))
    expect(saveConversation.mock.calls[0][0].id).toBe('conv-existing')
    // A failed save does not throw or flip the error flag (best-effort persistence).
    expect(result.current.error).toBe(false)
  })

  test('openConversation hydrates messages and parses tool calls', async () => {
    const detail: AgentConversationDetail = {
      ...summary({ id: 'conv-h' }),
      messages: [
        {
          id: 'm1',
          role: 'user',
          content: 'q',
          reasoning: null,
          toolCallsJson: null,
          status: null,
        },
        {
          id: 'm2',
          role: 'assistant',
          content: 'a',
          reasoning: 'r',
          toolCallsJson: '[{"id":"t1","name":"search_bm25","arguments":"{}"}]',
          status: 'done',
        },
      ],
    }
    const backend = makeBackend({
      loadConversation: vi.fn().mockResolvedValue(detail),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())

    let hydrated: ChatMessage[] | null = null
    await act(async () => {
      hydrated = await result.current.openConversation('conv-h')
    })
    expect(hydrated).toHaveLength(2)
    const assistant = hydrated![1]
    expect(assistant.role).toBe('assistant')
    expect(assistant.reasoning).toBe('r')
    expect(assistant.toolCalls?.[0].name).toBe('search_bm25')
    expect(assistant.status).toBe('done')
  })

  test('openConversation tolerates a corrupt tool-calls blob', async () => {
    const detail: AgentConversationDetail = {
      ...summary({ id: 'conv-bad' }),
      messages: [
        {
          id: 'm1',
          role: 'assistant',
          content: 'a',
          reasoning: null,
          toolCallsJson: '{not json',
          status: null,
        },
        {
          id: 'm2',
          role: 'assistant',
          content: 'b',
          reasoning: null,
          // Valid JSON but not an array → also dropped.
          toolCallsJson: '{"x":1}',
          status: null,
        },
      ],
    }
    const backend = makeBackend({
      loadConversation: vi.fn().mockResolvedValue(detail),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())

    let hydrated: ChatMessage[] | null = null
    await act(async () => {
      hydrated = await result.current.openConversation('conv-bad')
    })
    expect(hydrated![0].toolCalls).toBeUndefined()
    expect(hydrated![1].toolCalls).toBeUndefined()
  })

  test('openConversation drops a not-found conversation from the list', async () => {
    const backend = makeBackend({
      listConversations: vi
        .fn()
        .mockResolvedValue({ conversations: [summary({ id: 'conv-1' })] }),
      loadConversation: vi.fn().mockResolvedValue(null),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(result.current.conversations).toHaveLength(1))

    let hydrated: ChatMessage[] | null = [
      'placeholder',
    ] as unknown as ChatMessage[]
    await act(async () => {
      hydrated = await result.current.openConversation('conv-1')
    })
    expect(hydrated).toBeNull()
    expect(result.current.conversations).toHaveLength(0)
  })

  test('openConversation returns null when the load throws', async () => {
    const backend = makeBackend({
      loadConversation: vi.fn().mockRejectedValue(new Error('boom')),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())
    let hydrated: ChatMessage[] | null = ['x'] as unknown as ChatMessage[]
    await act(async () => {
      hydrated = await result.current.openConversation('conv-x')
    })
    expect(hydrated).toBeNull()
  })

  test('deleteConversation clears the active id when the active one is deleted', async () => {
    const detail: AgentConversationDetail = {
      ...summary({ id: 'conv-del' }),
      messages: [],
    }
    const backend = makeBackend({
      loadConversation: vi.fn().mockResolvedValue(detail),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())

    await act(async () => {
      await result.current.openConversation('conv-del')
    })
    expect(result.current.activeId).toBe('conv-del')

    await act(async () => {
      await result.current.deleteConversation('conv-del')
    })
    expect(backend.deleteConversation).toHaveBeenCalledWith('conv-del')
    expect(result.current.activeId).toBeNull()
  })

  test('deleteConversation refreshes even when the delete call fails', async () => {
    const backend = makeBackend({
      deleteConversation: vi.fn().mockRejectedValue(new Error('locked')),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(1),
    )
    await act(async () => {
      await result.current.deleteConversation('conv-other')
    })
    // Active id (none) is untouched; the list still refreshes to reflect real state.
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(2),
    )
  })

  test('renameConversation calls the backend with a trimmed title and refreshes', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(1),
    )
    await act(async () => {
      await result.current.renameConversation('conv-1', '  New title  ')
    })
    expect(backend.renameConversation).toHaveBeenCalledWith({
      id: 'conv-1',
      title: 'New title',
    })
    // The rename triggers a list refresh so the new title shows.
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(2),
    )
  })

  test('renameConversation skips a blank title (no backend call)', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())
    await act(async () => {
      await result.current.renameConversation('conv-1', '   ')
    })
    expect(backend.renameConversation).not.toHaveBeenCalled()
  })

  test('renameConversation refreshes even when the rename call fails', async () => {
    const backend = makeBackend({
      renameConversation: vi.fn().mockRejectedValue(new Error('locked')),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(1),
    )
    await act(async () => {
      await result.current.renameConversation('conv-1', 'whatever')
    })
    await waitFor(() =>
      expect(backend.listConversations).toHaveBeenCalledTimes(2),
    )
  })

  test('startNewChat clears the active selection', async () => {
    const detail: AgentConversationDetail = {
      ...summary({ id: 'conv-1' }),
      messages: [],
    }
    const backend = makeBackend({
      loadConversation: vi.fn().mockResolvedValue(detail),
    })
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())
    await act(async () => {
      await result.current.openConversation('conv-1')
    })
    expect(result.current.activeId).toBe('conv-1')
    act(() => result.current.startNewChat())
    expect(result.current.activeId).toBeNull()
  })

  test('a superseded load is dropped without clobbering a newer one', async () => {
    // Each call returns a deferred promise; the mount load (call #1) is held, a refresh (call #2)
    // supersedes it and resolves first, then the stale call #1 resolves and must be ignored.
    const deferred: Array<{
      resolve: (value: { conversations: AgentConversationSummary[] }) => void
    }> = []
    const listConversations = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          deferred.push({ resolve })
        }),
    )
    const backend = makeBackend({ listConversations })
    const { result } = renderHook(() => useChatHistory({ backend }))

    // Wait for the mount load to register call #1.
    await waitFor(() => expect(deferred).toHaveLength(1))
    // Supersede it with a refresh (call #2).
    act(() => result.current.refresh())
    await waitFor(() => expect(deferred).toHaveLength(2))

    // The newer load (call #2) resolves first → its result wins.
    act(() =>
      deferred[1].resolve({ conversations: [summary({ id: 'fresh' })] }),
    )
    await waitFor(() =>
      expect(result.current.conversations).toEqual([
        expect.objectContaining({ id: 'fresh' }),
      ]),
    )
    // The stale mount load (call #1) resolves late and must NOT overwrite the fresh result.
    act(() =>
      deferred[0].resolve({ conversations: [summary({ id: 'stale' })] }),
    )
    await act(async () => {
      await Promise.resolve()
    })
    expect(result.current.conversations).toEqual([
      expect.objectContaining({ id: 'fresh' }),
    ])
  })

  test('a superseded failing load does not flip the error flag', async () => {
    // The mount load is held, a refresh supersedes it and succeeds, then the stale mount load
    // rejects late; the stale rejection must be ignored so the error flag stays clean.
    const deferred: Array<{
      resolve: (value: { conversations: AgentConversationSummary[] }) => void
      reject: (reason: unknown) => void
    }> = []
    const listConversations = vi.fn().mockImplementation(
      () =>
        new Promise((resolve, reject) => {
          deferred.push({ resolve, reject })
        }),
    )
    const backend = makeBackend({ listConversations })
    const { result } = renderHook(() => useChatHistory({ backend }))

    await waitFor(() => expect(deferred).toHaveLength(1))
    act(() => result.current.refresh())
    await waitFor(() => expect(deferred).toHaveLength(2))

    act(() => deferred[1].resolve({ conversations: [summary({ id: 'ok' })] }))
    await waitFor(() => expect(result.current.conversations).toHaveLength(1))
    // The stale mount load rejects late; the guard drops it (no error flip).
    act(() => deferred[0].reject(new Error('stale failure')))
    await act(async () => {
      await Promise.resolve()
    })
    expect(result.current.error).toBe(false)
  })

  test('coalesces rapid refreshes so only the latest load reaches the backend list', async () => {
    // Several refreshes fired before the first clears its paint wait: every earlier generation is
    // superseded during the paint await, so only the final generation calls listConversations.
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    act(() => {
      result.current.refresh()
      result.current.refresh()
      result.current.refresh()
    })
    await waitFor(() => expect(result.current.conversations).toHaveLength(1))
    // The mount load + 3 refreshes share one paint window; superseded generations bail before the
    // backend call, so the backend list runs far fewer times than the number of refresh calls.
    expect(
      vi.mocked(backend.listConversations).mock.calls.length,
    ).toBeLessThanOrEqual(2)
  })

  test('persistTurn omits a tool-calls blob when there are no tool calls', async () => {
    const backend = makeBackend()
    const { result } = renderHook(() => useChatHistory({ backend }))
    await waitFor(() => expect(backend.listConversations).toHaveBeenCalled())
    act(() =>
      result.current.persistTurn([
        userMessage,
        { id: 'm2', role: 'assistant', content: 'no tools', status: 'done' },
      ]),
    )
    await waitFor(() => expect(backend.saveConversation).toHaveBeenCalled())
    const request = vi.mocked(backend.saveConversation).mock.calls[0][0]
    expect(request.messages[1].toolCallsJson).toBeNull()
    expect(request.messages[1].reasoning).toBeNull()
  })
})
