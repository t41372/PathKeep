/**
 * @file use-chat-history.ts
 * @description Conversation-persistence controller: bridges the streaming chat hook to the agent
 *              sidecar (list / save-on-finalize / open / delete / new) without ever blocking the
 *              stream. The route wires this to the backend-client and to `useAiChatStream`.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Own the conversation-list read model (newest-first) and the active conversation id.
 * - Persist a conversation on each finished turn (`persistTurn`) off the main thread — fire-and-
 *   forget so the save never janks the stream — then refresh the list.
 * - Open a past conversation (load → return its messages so the route can `reset` the chat hook).
 * - Delete a conversation (and clear the active id when the active one is deleted).
 * - Start a new chat (mint a fresh id; clear the active selection).
 *
 * ## Not responsible for
 * - The chat transcript / streaming (that is `useAiChatStream`).
 * - Rendering — the explorer component renders the list this hook exposes.
 *
 * ## Fluidity
 * - All backend calls are async and fire off the render path. `persistTurn` is intentionally
 *   fire-and-forget (it is called from the chat hook's finalize microtask): a slow disk write can
 *   never delay the UI; a refreshed list arrives whenever the save resolves.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'
import type {
  AgentConversationDetail,
  AgentConversationListResponse,
  AgentConversationSummary,
  AgentMessage,
  DeleteAgentConversationResult,
  RenameAgentConversationRequest,
  SaveAgentConversationRequest,
} from '@/lib/types'
import type { ChatMessage } from './use-ai-chat-stream'

/** Backend operations the controller needs, injected so it stays unit-testable. */
export interface ChatHistoryBackend {
  listConversations: () => Promise<AgentConversationListResponse>
  saveConversation: (
    request: SaveAgentConversationRequest,
  ) => Promise<AgentConversationSummary>
  loadConversation: (id: string) => Promise<AgentConversationDetail | null>
  deleteConversation: (id: string) => Promise<DeleteAgentConversationResult>
  renameConversation: (
    request: RenameAgentConversationRequest,
  ) => Promise<AgentConversationSummary | null>
}

export interface UseChatHistoryOptions {
  backend: ChatHistoryBackend
  /** Provider id stamped onto saved conversations (display only). */
  providerId?: string | null
  /** When false, the controller is dormant (no list load) — used by the route's gate branches. */
  enabled?: boolean
}

export interface ChatHistoryState {
  conversations: AgentConversationSummary[]
  activeId: string | null
  loading: boolean
  error: boolean
  /** Reload the list (after an external change or an error retry). */
  refresh: () => void
  /** Persist a finished turn's transcript for the active (or a freshly minted) conversation. */
  persistTurn: (messages: ChatMessage[]) => void
  /** Load a past conversation; resolves to its hydrated messages (or null when gone). */
  openConversation: (id: string) => Promise<ChatMessage[] | null>
  /** Delete a conversation; clears the active selection if it was the active one. */
  deleteConversation: (id: string) => Promise<void>
  /** Rename a conversation; refreshes the list so the new title shows. */
  renameConversation: (id: string, title: string) => Promise<void>
  /** Start a fresh conversation (clear the active id; the next save mints a new one). */
  startNewChat: () => void
}

/** Stable conversation-id factory; collisions are practically impossible within one session. */
function nextConversationId(): string {
  return `conv-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
}

/** Projects an in-memory chat message into the persisted `AgentMessage` shape. */
function toAgentMessage(message: ChatMessage): AgentMessage {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    reasoning: message.reasoning ?? null,
    toolCallsJson:
      message.toolCalls && message.toolCalls.length > 0
        ? JSON.stringify(message.toolCalls)
        : null,
    status: message.status ?? null,
  }
}

/** Projects a persisted `AgentMessage` back into the in-memory chat-message shape. */
function toChatMessage(message: AgentMessage): ChatMessage {
  let toolCalls: ChatMessage['toolCalls']
  if (message.toolCallsJson) {
    try {
      const parsed = JSON.parse(
        message.toolCallsJson,
      ) as ChatMessage['toolCalls']
      if (Array.isArray(parsed)) toolCalls = parsed
    } catch {
      // A corrupt tool-calls blob must never break opening a conversation; drop it silently and
      // still render the message text + reasoning.
      toolCalls = undefined
    }
  }
  // Reconstruct the durable agent trace (W-AI-7 WU-7): the backend joins the message's run → its
  // pinned citations + token tally, so a reopened turn renders the SAME evidence rows + star keys +
  // usage footer the live turn streamed. Mirrors the toolCalls reconstruction above: shape-compatible
  // (the backend `AgentCitation`/`AgentUsage` are the camelCase twins of `AssistantCitation`/
  // `AssistantUsage`), so the rendered turn reuses the live assistant-turn path with no special case.
  const citations =
    message.citations && message.citations.length > 0
      ? message.citations.map((citation) => ({
          historyId: citation.historyId,
          profileId: citation.profileId,
          url: citation.url,
          title: citation.title ?? null,
          visitedAt: citation.visitedAt,
          score: citation.score ?? null,
          canonicalUrl: citation.canonicalUrl ?? null,
        }))
      : undefined
  const usage = message.usage
    ? {
        promptTokens: message.usage.promptTokens,
        completionTokens: message.usage.completionTokens,
      }
    : undefined
  return {
    id: message.id,
    role: message.role === 'assistant' ? 'assistant' : 'user',
    content: message.content,
    reasoning: message.reasoning ?? undefined,
    toolCalls,
    citations,
    usage,
    status:
      (message.status as ChatMessage['status'] | null | undefined) ?? undefined,
  }
}

/**
 * The conversation-persistence controller. See file header for the no-jank contract.
 */
export function useChatHistory(
  options: UseChatHistoryOptions,
): ChatHistoryState {
  const { backend, providerId = null, enabled = true } = options

  const [conversations, setConversations] = useState<
    AgentConversationSummary[]
  >([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(false)

  // Latest deps kept in a ref so callbacks stay stable and a slow async resolve from a superseded
  // load cannot clobber a newer one (generation guard, like the streaming hook).
  const backendRef = useRef(backend)
  const providerIdRef = useRef(providerId)
  const activeIdRef = useRef<string | null>(activeId)
  const loadGenRef = useRef(0)
  useEffect(() => {
    backendRef.current = backend
    providerIdRef.current = providerId
  }, [backend, providerId])
  useEffect(() => {
    activeIdRef.current = activeId
  }, [activeId])

  // Core list load shared by the imperative `refresh` and the mount effect. The first setState is
  // deferred behind a paint so neither caller sets state synchronously (which would cascade
  // renders); the one-paint deferral is imperceptible for a list load and keeps the loading flag
  // honest. Stored in a ref (not a useCallback dep) so the mount effect can call it without the
  // lint heuristic seeing a synchronous-setState callback in its dependency list.
  const runLoadRef = useRef(async () => {
    const generation = ++loadGenRef.current
    await waitForNextPaint()
    if (generation !== loadGenRef.current) return
    setLoading(true)
    setError(false)
    try {
      const response = await backendRef.current.listConversations()
      if (generation !== loadGenRef.current) return
      setConversations(response.conversations)
      setLoading(false)
    } catch {
      if (generation !== loadGenRef.current) return
      // The saved chats are still on disk; surface a retry instead of a hard failure.
      setError(true)
      setLoading(false)
    }
  })

  const refresh = useCallback(() => {
    void runLoadRef.current()
  }, [])

  // Initial / re-enabled load. Dormant when the route's gate is closed so a disabled assistant
  // page never touches the agent plane. The load defers its first setState behind a paint, so the
  // effect body itself sets no state synchronously.
  useEffect(() => {
    if (!enabled) return
    void runLoadRef.current()
  }, [enabled])

  const persistTurn = useCallback(
    (messages: ChatMessage[]) => {
      // Nothing to persist for an empty transcript (e.g. a cancelled turn before any user message).
      if (messages.length === 0) return
      let id = activeIdRef.current
      if (!id) {
        id = nextConversationId()
        activeIdRef.current = id
        setActiveId(id)
      }
      const request: SaveAgentConversationRequest = {
        id,
        title: null,
        providerId: providerIdRef.current,
        messages: messages.map(toAgentMessage),
      }
      // Fire-and-forget: the save runs off the main thread; a refreshed list arrives when it lands.
      // A failed save must never throw into the stream's finalize microtask.
      backendRef.current
        .saveConversation(request)
        .then(() => {
          refresh()
        })
        .catch(() => {
          // Best-effort persistence; the live transcript is unaffected and a later turn retries.
        })
    },
    [refresh],
  )

  const openConversation = useCallback(
    async (id: string): Promise<ChatMessage[] | null> => {
      try {
        const detail = await backendRef.current.loadConversation(id)
        if (!detail) {
          // Opened a conversation that was deleted elsewhere: drop it from the list and bail.
          setConversations((current) => current.filter((c) => c.id !== id))
          return null
        }
        activeIdRef.current = id
        setActiveId(id)
        return detail.messages.map(toChatMessage)
      } catch {
        return null
      }
    },
    [],
  )

  const deleteConversation = useCallback(
    async (id: string): Promise<void> => {
      try {
        await backendRef.current.deleteConversation(id)
      } catch {
        // Even if the delete failed, fall through to a refresh so the list reflects real state.
      }
      if (activeIdRef.current === id) {
        activeIdRef.current = null
        setActiveId(null)
      }
      refresh()
    },
    [refresh],
  )

  const renameConversation = useCallback(
    async (id: string, title: string): Promise<void> => {
      const trimmed = title.trim()
      // A blank title is rejected by the store; never round-trip one.
      if (!trimmed) return
      try {
        await backendRef.current.renameConversation({ id, title: trimmed })
      } catch {
        // Best-effort: fall through to a refresh so the list reflects real on-disk state.
      }
      refresh()
    },
    [refresh],
  )

  const startNewChat = useCallback(() => {
    activeIdRef.current = null
    setActiveId(null)
  }, [])

  return {
    conversations,
    activeId,
    loading,
    error,
    refresh,
    persistTurn,
    openConversation,
    deleteConversation,
    renameConversation,
    startNewChat,
  }
}
