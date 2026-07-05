/**
 * @file use-ai-chat-stream.ts
 * @description The streaming-chat engine hook: turns a `sendChat` run into a live,
 *              60fps-bounded assistant turn without ever freezing the main thread.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Own the conversation message list and the lifecycle of one in-flight streaming turn.
 * - Drive `sendChat` → `subscribeToAiChatStream`, accumulate `token` / `reasoning` /
 *   `toolCall` chunks, and finalize on `done` / `error`.
 * - Guarantee fluidity: incoming chunks land in a ref buffer and are flushed to React
 *   state on a single `requestAnimationFrame`, so hundreds of chunks coalesce into at most
 *   one re-render per frame (never one setState per token).
 * - Unsubscribe and stop the rAF loop on terminal chunk, on Cancel, and on unmount.
 *
 * ## Not responsible for
 * - Rendering — callers map the returned messages onto chat UI.
 * - Markdown parsing — that is `StreamingMarkdown`'s job.
 * - Provider selection / availability gating — the route owns that and only calls
 *   `send` when a provider is configured.
 *
 * ## Why this exists
 * A prior AI surface called `setState` per streamed token; with gemma emitting hundreds of
 * reasoning chunks that produced a re-render storm (and a streamdown re-parse per render),
 * freezing the app for ~10s. The ref-buffer + rAF flush is the structural fix: the buffer
 * absorbs event volume off the render path, and React only sees one batched update per frame.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  AiAgentNote,
  AiChatStreamChunk,
  AiChatMessage,
  AiChatCitation,
  HostCallRecord,
  LimitsHit,
} from '../../lib/types'

/** One cited history page the agent grounded its answer on (W-AI-7). Re-export of the IPC shape. */
export type AssistantCitation = AiChatCitation

/** Stable id factory for messages; collisions are practically impossible within one session. */
function nextMessageId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

/** Build a fresh, streaming assistant message (the empty turn that a run fills in). */
function freshAssistantMessage(id: string): ChatMessage {
  return {
    id,
    role: 'assistant',
    content: '',
    reasoning: '',
    toolCalls: [],
    citations: [],
    status: 'streaming',
  }
}

/** Lifecycle of one tool call in the transparency timeline (W-AI-7). */
export type AssistantToolCallStatus = 'pending' | 'success' | 'error'

/** One requested tool call surfaced in the transparency timeline. */
export interface AssistantToolCall {
  id: string
  name: string
  arguments: string
  /**
   * Provider correlation id (W-AI-7 agent path). A streamed `toolResult` is matched back to its
   * `toolCall` by this id; absent on the plain W-AI-1 path (those calls stay `pending` forever,
   * which the UI renders without a result — honest, since no result is coming).
   */
  callId?: string
  /**
   * Lifecycle: `pending` from the call until its result lands, then `success`/`error` (W-AI-7).
   * Absent (treated as `pending`) on the plain path that never streams a result.
   */
  status?: AssistantToolCallStatus
  /** The executed tool result text (W-AI-7), present once the matching `toolResult` arrives. */
  result?: string
  /** True when the matching `toolResult` reported a failure (honest error state). */
  isError?: boolean
  /**
   * The verbatim `run_code` script the assistant wrote and ran (W-AI-8 WU-5). Present ONLY on a
   * code-mode step; its presence is what marks a tool call as a code run for the renderer. Absent for
   * the search tools, so a search step renders exactly as W-AI-7. Per the transparency contract the
   * user must see the EXACT source that ran — it is never truncated.
   */
  codeSource?: string
  /** The code run's host-call timeline (W-AI-8 WU-5); absent/empty for the search tools. */
  hostCalls?: HostCallRecord[]
  /** Which hard sandbox limit bounded the code run, if any (W-AI-8 WU-5). */
  limitsHit?: LimitsHit
}

/** Running token accounting for one assistant turn, summed from `usage` chunks (W-AI-7). */
export interface AssistantUsage {
  promptTokens: number
  completionTokens: number
}

/** Terminal status of an assistant turn, used to drive finalized UI affordances. */
export type AssistantTurnStatus = 'streaming' | 'done' | 'error' | 'cancelled'

/** One rendered chat message — a user prompt or an assistant turn. */
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  /** Visible answer text (assistant) or the prompt (user). */
  content: string
  /** Accumulated reasoning/thinking text (assistant only). */
  reasoning?: string
  /** Visible tool calls in request order (assistant only). */
  toolCalls?: AssistantToolCall[]
  /** Running token accounting summed from `usage` chunks (assistant agent path only). */
  usage?: AssistantUsage
  /** Cited history pages the agent grounded its answer on (assistant agent path only). */
  citations?: AssistantCitation[]
  /** Terminal status (assistant only); undefined while a user message. */
  status?: AssistantTurnStatus
  /** Error message when `status === 'error'`. */
  error?: string
}

/** Dependencies injected so the hook stays unit-testable without a live Tauri bridge. */
export interface AiChatStreamDeps {
  /** Starts a run and resolves to its run id. */
  sendChat: (request: {
    messages: AiChatMessage[]
    providerId?: string | null
    /** When true, the worker runs the tool-executing agent harness (W-AI-7). */
    toolsEnabled?: boolean
    /** Conversation this run answers — links the durable agent trace (agent path only). */
    conversationId?: string | null
    /** Message this run answers — links the durable agent trace (agent path only). */
    messageId?: string | null
  }) => Promise<{ runId: string }>
  /** Asks the worker to stop a live run. */
  cancelChat: (runId: string) => Promise<{ cancelled: boolean }>
  /** Subscribes to chunks for one run; resolves to an unsubscribe fn. */
  subscribe: (
    runId: string,
    listener: (chunk: AiChatStreamChunk) => void,
  ) => Promise<() => void>
  /** Provider id forwarded to `sendChat` (null → worker default). */
  providerId?: string | null
  /**
   * When true, every turn runs the W-AI-7 agent harness (tool-executing loop) instead of plain
   * streaming chat. The history assistant sets this so it answers OVER history via the search
   * tools; omit/false for a plain-chat surface (streams exactly as W-AI-1).
   */
  toolsEnabled?: boolean
  /**
   * Active conversation id, forwarded on the agent path so the durable trace links to the chat
   * turn it answers (the backend FK self-heals when the conversation is not yet saved).
   */
  conversationId?: string | null
  /** System prompt prepended to the transcript on each turn, when set. */
  systemPrompt?: string | null
  /**
   * Initial messages used to seed the conversation on first mount (e.g. when opening the route on
   * a hydrated past conversation). Read once at mount; later changes do not re-seed — use
   * `reset(messages)` to switch conversations after mount.
   */
  initialMessages?: ChatMessage[]
  /**
   * Called once per turn the instant it finalizes (done / error / cancelled), with the full
   * resulting message list. The route persists from here — on finalize, never per chunk — so a
   * save can never jank the stream. Invoked asynchronously (microtask) after the finalize
   * `setMessages`, so it is off the render path.
   */
  onTurnFinalized?: (messages: ChatMessage[]) => void
  /**
   * Resolves an agent-harness control note CODE (review-fix M-6) to localized, user-facing text.
   * The harness streams these control notes (max-steps / token-budget reached, tool-calling
   * unavailable) as stable CODES — never raw English — so the route binds this to its `assistant`
   * translator. When omitted (e.g. a plain-chat surface that never runs the harness), a `note` chunk
   * is dropped rather than rendered raw.
   */
  localizeAgentNote?: (code: AiAgentNote) => string
}

/** What the hook hands back to the chat view. */
export interface AiChatStreamState {
  messages: ChatMessage[]
  /** True from send until the terminal chunk / cancel. */
  streaming: boolean
  /** True after send but before the first visible/reasoning/tool chunk arrives. */
  awaitingFirstChunk: boolean
  /** Send a new user turn. No-op when blank or already streaming. */
  send: (text: string) => void
  /**
   * Regenerate the LATEST completed assistant turn IN PLACE: drop the trailing assistant answer and
   * re-stream a fresh one for the SAME existing user turn — no duplicate question is appended. The
   * model transcript for the re-run is the conversation UP TO (and including) that user turn, and the
   * finalized/persisted transcript reflects the replacement (one Q + the regenerated A), never a
   * duplicate. No-op while streaming, or when the last message is not a completed assistant turn with
   * a preceding user turn.
   */
  regenerate: () => void
  /** Cancel the in-flight turn (best-effort; UI finalizes immediately). */
  cancel: () => void
  /**
   * Replace the whole transcript: pass a hydrated message list to open a past conversation, or
   * `[]` (or nothing) to start a fresh chat. Tears down any in-flight turn first, so a switch can
   * never leave a stale stream writing into the new conversation.
   */
  reset: (messages?: ChatMessage[]) => void
}

/**
 * The streaming chat engine. See file header for the fluidity contract.
 *
 * The hook keeps the *finalized* messages in `messages` state and the *in-flight* turn's
 * accumulating buffers in refs; a single rAF flush copies the live buffers into the last
 * assistant message so only that one message re-renders per frame.
 */
export function useAiChatStream(deps: AiChatStreamDeps): AiChatStreamState {
  // Seed from `initialMessages` once (lazy initializer); later prop changes do NOT re-seed —
  // switching conversations after mount goes through `reset` so an in-flight turn is torn down.
  const [messages, setMessages] = useState<ChatMessage[]>(
    () => deps.initialMessages ?? [],
  )
  const [streaming, setStreaming] = useState(false)
  const [awaitingFirstChunk, setAwaitingFirstChunk] = useState(false)

  // Mirror of `messages` so `send` can read the prior transcript synchronously without
  // building it inside a setState updater (updaters must stay pure / side-effect free).
  // Synced in an effect (refs must not be written during render); `send` runs in an event
  // handler, i.e. after commit + effects, so the ref is always current when it is read.
  const messagesRef = useRef<ChatMessage[]>(messages)
  useEffect(() => {
    messagesRef.current = messages
  }, [messages])

  // Live-turn accumulators. Kept in refs so chunk arrival never triggers a render directly.
  // `usage`/`citations` are W-AI-7 additions: usage sums across the turn's `usage` chunks; citations
  // is the run's terminal evidence set (replaced wholesale by the single `citations` chunk).
  const bufferRef = useRef({
    content: '',
    reasoning: '',
    toolCalls: [] as AssistantToolCall[],
    usage: null as AssistantUsage | null,
    citations: [] as AssistantCitation[],
    sawFirstChunk: false,
  })
  const activeIdRef = useRef<string | null>(null)
  const runIdRef = useRef<string | null>(null)
  const unsubscribeRef = useRef<(() => void) | null>(null)
  const rafRef = useRef<number | null>(null)
  // Generation guard: bumped on every send/cancel/unmount so a late async resolve
  // (sendChat or subscribe) from a superseded turn cannot mutate the current one.
  const genRef = useRef(0)

  // Keep the latest deps in a ref so callbacks stay stable (no re-subscribe churn). Synced in
  // an effect; turns start from event handlers, so the ref is current by the time it is read.
  const depsRef = useRef(deps)
  useEffect(() => {
    depsRef.current = deps
  }, [deps])

  const stopRaf = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current)
      rafRef.current = null
    }
  }, [])

  const teardownSubscription = useCallback(() => {
    if (unsubscribeRef.current) {
      unsubscribeRef.current()
      unsubscribeRef.current = null
    }
  }, [])

  /**
   * Copy the live buffer into the active assistant message (one batched setState).
   *
   * Invariant: only ever runs via a frame scheduled by `scheduleFlush` while a turn is active,
   * and `finalize`/`send` cancel any pending frame before clearing `activeIdRef`, so the active
   * id is always present here. If no message matches the id (already removed) the map is a no-op.
   */
  const flush = useCallback(() => {
    rafRef.current = null
    const buffer = bufferRef.current
    const id = activeIdRef.current
    const snapshot = {
      content: buffer.content,
      reasoning: buffer.reasoning,
      toolCalls: buffer.toolCalls.slice(),
      usage: buffer.usage,
      citations: buffer.citations.slice(),
    }
    setMessages((current) =>
      current.map((message) =>
        message.id === id
          ? {
              ...message,
              content: snapshot.content,
              reasoning: snapshot.reasoning,
              toolCalls: snapshot.toolCalls,
              usage: snapshot.usage ?? undefined,
              citations: snapshot.citations,
            }
          : message,
      ),
    )
  }, [])

  /** Schedule a flush on the next frame; coalesces a burst of chunks into one render. */
  const scheduleFlush = useCallback(() => {
    if (rafRef.current !== null) return
    if (typeof requestAnimationFrame === 'undefined') {
      // No rAF (non-browser): flush synchronously so behavior stays correct.
      flush()
      return
    }
    rafRef.current = requestAnimationFrame(flush)
  }, [flush])

  /**
   * Finalize the active assistant turn with a terminal status, then tear down.
   *
   * Only ever called via the generation-guarded chunk handler, the cancel handler, or a
   * sendChat rejection — all while a turn is active — so `activeIdRef` is set; an id that no
   * longer matches any message makes the map a harmless no-op.
   */
  const finalize = useCallback(
    (status: AssistantTurnStatus, error?: string) => {
      stopRaf()
      teardownSubscription()
      // Bump the generation so any subscribe-resolve still in flight for THIS turn (a terminal
      // chunk that finalized before `subscribe`'s promise settled) fails its generation check and
      // self-unsubscribes, instead of stashing a live listener for an already-finished turn.
      genRef.current += 1
      const buffer = bufferRef.current
      const id = activeIdRef.current
      const snapshot = {
        content: buffer.content,
        reasoning: buffer.reasoning,
        toolCalls: buffer.toolCalls.slice(),
        usage: buffer.usage,
        citations: buffer.citations.slice(),
      }
      // The finalized list to persist: the committed messages (from the ref, which holds the
      // transcript that `send` set + every flush) with the active assistant turn sealed with its
      // terminal status. Computed once here so the persistence callback fires with the exact
      // transcript the user sees, without reading inside the setState updater.
      const finalized = messagesRef.current.map((message) =>
        message.id === id
          ? {
              ...message,
              content: snapshot.content,
              reasoning: snapshot.reasoning,
              toolCalls: snapshot.toolCalls,
              usage: snapshot.usage ?? undefined,
              citations: snapshot.citations,
              status,
              error,
            }
          : message,
      )
      setMessages(finalized)
      activeIdRef.current = null
      runIdRef.current = null
      setStreaming(false)
      setAwaitingFirstChunk(false)
      // Persist-on-finalize, off the render path: fire the callback on a microtask so the save
      // never runs synchronously inside the finalize setState (which would extend the commit and
      // could jank the very last frame of the stream).
      const onFinalized = depsRef.current.onTurnFinalized
      if (onFinalized) {
        queueMicrotask(() => onFinalized(finalized))
      }
    },
    [stopRaf, teardownSubscription],
  )

  const handleChunk = useCallback(
    (chunk: AiChatStreamChunk, generation: number) => {
      if (generation !== genRef.current) return
      const buffer = bufferRef.current
      if (!buffer.sawFirstChunk) {
        buffer.sawFirstChunk = true
        setAwaitingFirstChunk(false)
      }
      switch (chunk.kind) {
        case 'token':
          buffer.content += chunk.text
          scheduleFlush()
          break
        case 'reasoning':
          buffer.reasoning += chunk.text
          scheduleFlush()
          break
        case 'toolCall':
          buffer.toolCalls = [
            ...buffer.toolCalls,
            {
              id: nextMessageId('tool'),
              name: chunk.name,
              arguments: chunk.arguments,
              callId: chunk.callId,
              // The agent path will stream a matching `toolResult` (correlated by callId); start
              // `pending`. The plain path has no callId and never resolves — that stays honest.
              status: 'pending',
            },
          ]
          scheduleFlush()
          break
        case 'toolResult': {
          // Correlate the executed result back to its pending call by the provider `callId`. Match
          // the LAST still-pending call with that id (ids are unique per provider turn, but matching
          // the latest pending one is robust to a model reusing an id across turns). If no call
          // matches (a result with no prior call — unexpected), drop it rather than fabricate a row.
          let matched = false
          buffer.toolCalls = buffer.toolCalls
            .slice()
            .reverse()
            .map((call): AssistantToolCall => {
              if (!matched && call.callId === chunk.callId) {
                matched = true
                return {
                  ...call,
                  result: chunk.result,
                  isError: chunk.isError,
                  status: chunk.isError ? 'error' : 'success',
                  // W-AI-8 WU-5 code-mode transparency fields ride the SAME buffer as the existing
                  // result fields (no new render path). They are present only on a `run_code` step;
                  // for the search tools the chunk omits them, so a search step is unchanged.
                  codeSource: chunk.codeSource,
                  hostCalls: chunk.hostCalls,
                  limitsHit: chunk.limitsHit,
                }
              }
              return call
            })
            .reverse()
          if (matched) scheduleFlush()
          break
        }
        case 'usage': {
          // Accumulate a running prompt/completion total across the turn's usage chunks (the agent
          // emits one per model turn). The footer shows the summed budget for the whole turn.
          const prior = buffer.usage ?? { promptTokens: 0, completionTokens: 0 }
          buffer.usage = {
            promptTokens: prior.promptTokens + chunk.promptTokens,
            completionTokens: prior.completionTokens + chunk.completionTokens,
          }
          scheduleFlush()
          break
        }
        case 'citations':
          // The terminal evidence set, emitted once right before `done`. Replace wholesale.
          buffer.citations = chunk.citations.slice()
          scheduleFlush()
          break
        case 'note': {
          // A harness control note (review-fix M-6) carried as a stable CODE — resolve it to
          // localized text and append it as an italic line to the visible answer, mirroring the old
          // raw-English `_note_` token but now in the user's locale. Dropped (never rendered raw)
          // when the surface did not bind a localizer.
          const localize = depsRef.current.localizeAgentNote
          if (localize) {
            buffer.content += `\n\n_${localize(chunk.code)}_`
            scheduleFlush()
          }
          break
        }
        case 'done':
          finalize('done')
          break
        case 'error':
          finalize('error', chunk.message)
          break
      }
    },
    [finalize, scheduleFlush],
  )

  // `streaming` mirrored into a ref so `send`/`cancel` can guard re-entry without depending on
  // it. Synced in an effect; both callers run from event handlers (post-commit).
  const streamingRef = useRef(false)
  useEffect(() => {
    streamingRef.current = streaming
  }, [streaming])

  /** Kick off the desktop run + subscription for a built transcript. */
  const startRun = useCallback(
    (transcript: AiChatMessage[], generation: number, messageId: string) => {
      const { sendChat, subscribe, providerId, toolsEnabled, conversationId } =
        depsRef.current
      sendChat({
        messages: transcript,
        providerId,
        // The history assistant runs WITH tools by default (toolsEnabled true) so it answers over
        // history via the search tools; a plain-chat caller omits it and the worker streams exactly
        // as W-AI-1. The conversation/message ids link the durable agent trace to this turn.
        toolsEnabled,
        conversationId,
        messageId,
      })
        .then((ack) => {
          if (generation !== genRef.current) return
          runIdRef.current = ack.runId
          return subscribe(ack.runId, (chunk) =>
            handleChunk(chunk, generation),
          ).then((unsubscribe) => {
            if (generation !== genRef.current) {
              // Turn was superseded while subscribing; drop the listener.
              unsubscribe()
              return
            }
            unsubscribeRef.current = unsubscribe
          })
        })
        .catch((reason: unknown) => {
          if (generation !== genRef.current) return
          finalize(
            'error',
            reason instanceof Error ? reason.message : String(reason),
          )
        })
    },
    [finalize, handleChunk],
  )

  /**
   * Project an in-memory chat-message list into the model transcript: the optional system prompt
   * followed by every user turn and every non-empty assistant turn, in order. Shared by `send` and
   * `regenerate` so both build the request identically — the only difference is WHICH slice of the
   * transcript each passes in (send: the whole prior history; regenerate: history up to and including
   * the user turn being re-answered, with the stale assistant answer excluded).
   */
  const buildModelTranscript = useCallback(
    (history: readonly ChatMessage[]): AiChatMessage[] => {
      const transcript: AiChatMessage[] = []
      const system = depsRef.current.systemPrompt
      if (system && system.trim()) {
        transcript.push({ role: 'system', content: system })
      }
      for (const message of history) {
        if (message.role === 'user') {
          transcript.push({ role: 'user', content: message.content })
        } else if (message.content) {
          transcript.push({ role: 'assistant', content: message.content })
        }
      }
      return transcript
    },
    [],
  )

  /**
   * Common turn bootstrap shared by `send` and `regenerate`: supersede any prior turn, reset the
   * live buffer, mint a fresh assistant message id, flip the stream flags, and start the run. The
   * caller supplies the next committed message list (with the fresh `streaming` assistant turn
   * already appended) and the model transcript to send.
   */
  const beginTurn = useCallback(
    (params: {
      nextMessages: ChatMessage[]
      assistantId: string
      transcript: AiChatMessage[]
    }) => {
      const generation = ++genRef.current
      stopRaf()
      teardownSubscription()
      bufferRef.current = {
        content: '',
        reasoning: '',
        toolCalls: [],
        usage: null,
        citations: [],
        sawFirstChunk: false,
      }
      activeIdRef.current = params.assistantId
      setMessages(params.nextMessages)
      setStreaming(true)
      setAwaitingFirstChunk(true)
      // The assistant message id doubles as the agent run's `messageId` so the durable trace links
      // to the exact turn it answers.
      startRun(params.transcript, generation, params.assistantId)
    },
    [startRun, stopRaf, teardownSubscription],
  )

  const send = useCallback(
    (text: string) => {
      const prompt = text.trim()
      if (!prompt) return
      if (streamingRef.current) return

      const userMessage: ChatMessage = {
        id: nextMessageId('user'),
        role: 'user',
        content: prompt,
      }
      const assistantId = nextMessageId('assistant')
      const assistantMessage = freshAssistantMessage(assistantId)

      // The model transcript is the prior history (read from the ref so the setState updater stays
      // side-effect free) plus this new prompt.
      const prior = messagesRef.current
      const transcript = buildModelTranscript(prior)
      transcript.push({ role: 'user', content: prompt })

      beginTurn({
        nextMessages: [...prior, userMessage, assistantMessage],
        assistantId,
        transcript,
      })
    },
    [beginTurn, buildModelTranscript],
  )

  const regenerate = useCallback(() => {
    if (streamingRef.current) return
    const prior = messagesRef.current
    // Regenerate is only valid on the LATEST completed turn: the last message must be an assistant
    // turn that has finished (done / error / cancelled), with a user turn immediately before it.
    const lastIndex = prior.length - 1
    if (lastIndex < 1) return
    const last = prior[lastIndex]
    if (last.role !== 'assistant' || last.status === 'streaming') return
    const precedingUser = prior[lastIndex - 1]
    if (precedingUser.role !== 'user') return

    // Drop the stale assistant answer; keep the existing user turn in place (no duplicate question).
    // The history up to and including that user turn is both the new committed prefix and the basis
    // for the model transcript, so the re-run answers the SAME question without re-appending it.
    const historyThroughUser = prior.slice(0, lastIndex)
    const transcript = buildModelTranscript(historyThroughUser)

    const assistantId = nextMessageId('assistant')
    const assistantMessage = freshAssistantMessage(assistantId)

    beginTurn({
      nextMessages: [...historyThroughUser, assistantMessage],
      assistantId,
      transcript,
    })
  }, [beginTurn, buildModelTranscript])

  const cancel = useCallback(() => {
    if (!streamingRef.current) return
    const runId = runIdRef.current
    // Supersede first so in-flight chunks for this run are ignored.
    genRef.current += 1
    if (runId) {
      void depsRef.current.cancelChat(runId).catch(() => {
        // Best-effort: UI already finalized; a failed cancel must not throw.
      })
    }
    finalize('cancelled')
  }, [finalize])

  /**
   * Replace the transcript wholesale (open a past conversation or start a new chat).
   *
   * Supersedes any live turn (generation bump + teardown) and best-effort-cancels its backend run
   * so a switch can never leave a stale stream writing into the new conversation, then drops the
   * stream flags and the buffer. Does NOT call `onTurnFinalized` — switching is not a finished
   * turn; the caller already holds whatever it needs to persist.
   */
  const reset = useCallback(
    (next?: ChatMessage[]) => {
      const runId = runIdRef.current
      genRef.current += 1
      stopRaf()
      teardownSubscription()
      if (runId) {
        void depsRef.current.cancelChat(runId).catch(() => {
          // Best-effort: the UI is moving on regardless of whether the backend acks the cancel.
        })
      }
      bufferRef.current = {
        content: '',
        reasoning: '',
        toolCalls: [],
        usage: null,
        citations: [],
        sawFirstChunk: false,
      }
      activeIdRef.current = null
      runIdRef.current = null
      setMessages(next ?? [])
      setStreaming(false)
      setAwaitingFirstChunk(false)
    },
    [stopRaf, teardownSubscription],
  )

  // Tear everything down on unmount so a late chunk can't touch a dead component.
  useEffect(() => {
    return () => {
      genRef.current += 1
      stopRaf()
      teardownSubscription()
    }
  }, [stopRaf, teardownSubscription])

  return {
    messages,
    streaming,
    awaitingFirstChunk,
    send,
    regenerate,
    cancel,
    reset,
  }
}
