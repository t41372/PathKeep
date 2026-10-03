/**
 * One chat's live state: sends a message, folds the streamed chunks into the
 * transcript, and saves it when a turn ends.
 *
 * Responsibilities: the `ai_chat_send` / stream / `ai_chat_cancel` protocol,
 * frame-batched updates so token bursts never block the main thread, and
 * persisting each finished turn (the backend does not save conversations by
 * itself). Not responsible for the conversation list or any rendering.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { subscribeToAiChatStream } from '@/lib/ipc/ai-stream'
import { describeError } from '@/lib/errors'
import type { AiChatStreamChunk } from '@/lib/types'
import {
  assistantTurn,
  buildTranscript,
  newId,
  toAgentMessage,
  userTurn,
  type ChatTurn,
  type TurnStatus,
} from './chat-model'

interface ChatOptions {
  providerId: string | null
  systemPrompt: string
  /** The current title, so re-saving a renamed conversation keeps its name. */
  titleFor: (conversationId: string) => string | undefined
  onSaved: () => void
}

interface ActiveRun {
  generation: number
  runId: string | null
  turn: ChatTurn
  unsubscribe: (() => void) | null
  frame: number | null
}

export function useChat(options: ChatOptions) {
  const [conversationId, setConversationId] = useState(() => newId('conv'))
  const [turns, setTurns] = useState<ChatTurn[]>([])
  const [streaming, setStreaming] = useState(false)

  const turnsRef = useRef<ChatTurn[]>([])
  const conversationRef = useRef(conversationId)
  const runRef = useRef<ActiveRun | null>(null)
  const generationRef = useRef(0)
  const optionsRef = useRef(options)
  useEffect(() => {
    optionsRef.current = options
  })

  const commit = useCallback((next: ChatTurn[]) => {
    turnsRef.current = next
    setTurns(next)
  }, [])

  const persist = useCallback((saved: ChatTurn[], id: string) => {
    const { providerId, titleFor, onSaved } = optionsRef.current
    void intelligenceClient
      .saveConversation({
        id,
        title: titleFor(id) ?? null,
        providerId,
        messages: saved.map(toAgentMessage),
      })
      .then(onSaved)
      .catch(() => undefined)
  }, [])

  const flush = useCallback(
    (run: ActiveRun) => {
      run.frame = null
      const snapshot = { ...run.turn }
      commit(
        turnsRef.current.map((turn) =>
          turn.id === snapshot.id ? snapshot : turn,
        ),
      )
    },
    [commit],
  )

  const scheduleFlush = useCallback(
    (run: ActiveRun) => {
      if (run.frame === null)
        run.frame = requestAnimationFrame(() => flush(run))
    },
    [flush],
  )

  const finish = useCallback(
    (run: ActiveRun, status: TurnStatus, error?: string) => {
      if (runRef.current !== run) return
      if (run.frame !== null) cancelAnimationFrame(run.frame)
      run.unsubscribe?.()
      runRef.current = null
      generationRef.current += 1
      const finished: ChatTurn = { ...run.turn, status, error }
      const next = turnsRef.current.map((turn) =>
        turn.id === finished.id ? finished : turn,
      )
      commit(next)
      setStreaming(false)
      persist(next, conversationRef.current)
    },
    [commit, persist],
  )

  const applyChunk = useCallback(
    (run: ActiveRun, chunk: AiChatStreamChunk) => {
      if (runRef.current !== run) return
      const turn = run.turn
      switch (chunk.kind) {
        case 'token':
          turn.content += chunk.text
          break
        case 'reasoning':
          turn.reasoning += chunk.text
          break
        case 'toolCall':
          turn.tools = [
            ...turn.tools,
            {
              id: newId('tool'),
              callId: chunk.callId,
              name: chunk.name,
              arguments: chunk.arguments,
              pending: true,
            },
          ]
          break
        case 'toolResult':
          turn.tools = turn.tools.map((step) =>
            step.pending &&
            (step.callId
              ? step.callId === chunk.callId
              : step.name === chunk.name)
              ? {
                  ...step,
                  pending: false,
                  result: chunk.result,
                  isError: chunk.isError,
                  codeSource: chunk.codeSource,
                }
              : step,
          )
          break
        case 'usage':
          turn.usage = {
            promptTokens: chunk.promptTokens,
            completionTokens: chunk.completionTokens,
          }
          break
        case 'citations':
          turn.citations = chunk.citations
          break
        case 'note':
          turn.notes = [...turn.notes, chunk.code]
          break
        case 'done':
          finish(run, 'done')
          return
        case 'error':
          finish(run, 'error', chunk.message)
          return
      }
      scheduleFlush(run)
    },
    [finish, scheduleFlush],
  )

  const startRun = useCallback(
    (history: ChatTurn[]) => {
      const { providerId, systemPrompt } = optionsRef.current
      const assistant = assistantTurn()
      const run: ActiveRun = {
        generation: ++generationRef.current,
        runId: null,
        turn: assistant,
        unsubscribe: null,
        frame: null,
      }
      runRef.current = run
      commit([...history, assistant])
      setStreaming(true)

      intelligenceClient
        .sendChat({
          messages: buildTranscript(history, systemPrompt),
          providerId,
          toolsEnabled: true,
          conversationId: conversationRef.current,
          messageId: assistant.id,
        })
        .then(async (ack) => {
          if (runRef.current !== run) {
            void intelligenceClient.cancelChat(ack.runId).catch(() => undefined)
            return
          }
          run.runId = ack.runId
          const unsubscribe = await subscribeToAiChatStream(
            ack.runId,
            (chunk) => applyChunk(run, chunk),
          )
          if (runRef.current === run) run.unsubscribe = unsubscribe
          else unsubscribe()
        })
        .catch((reason: unknown) => finish(run, 'error', describeError(reason)))
    },
    [applyChunk, commit, finish],
  )

  const send = useCallback(
    (text: string) => {
      const content = text.trim()
      if (!content || runRef.current) return
      startRun([...turnsRef.current, userTurn(content)])
    },
    [startRun],
  )

  const stop = useCallback(() => {
    const run = runRef.current
    if (!run) return
    if (run.runId)
      void intelligenceClient.cancelChat(run.runId).catch(() => undefined)
    finish(run, 'cancelled')
  }, [finish])

  /** Re-asks the last question, replacing a failed or stopped answer. */
  const retry = useCallback(() => {
    if (runRef.current) return
    const current = turnsRef.current
    const last = current[current.length - 1]
    if (last?.role !== 'assistant') return
    startRun(current.slice(0, -1))
  }, [startRun])

  const open = useCallback(
    (id: string, loaded: ChatTurn[]) => {
      stop()
      conversationRef.current = id
      setConversationId(id)
      commit(loaded)
    },
    [commit, stop],
  )

  const startNew = useCallback(() => open(newId('conv'), []), [open])

  useEffect(
    () => () => {
      const run = runRef.current
      if (run?.runId)
        void intelligenceClient.cancelChat(run.runId).catch(() => undefined)
      if (run) finish(run, 'cancelled')
    },
    [finish],
  )

  return { conversationId, turns, streaming, send, stop, retry, open, startNew }
}
