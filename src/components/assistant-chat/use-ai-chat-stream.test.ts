/**
 * @file use-ai-chat-stream.test.ts
 * @description Behavior + fluidity-contract coverage for the streaming chat engine hook.
 * @module components/assistant-chat
 *
 * ## What this proves
 * - Token / reasoning / tool-call chunks accumulate into the active assistant message.
 * - The terminal `done` / `error` chunks finalize status and unsubscribe.
 * - Cancel calls `cancelChat`, finalizes immediately, and ignores late chunks.
 * - Chunks are coalesced through `requestAnimationFrame` (the no-freeze guarantee): a burst of
 *   chunks before a frame produces exactly ONE flush, not one render per chunk.
 * - Subscriptions tear down on terminal chunk, on cancel, and on unmount; superseded turns
 *   (a second send before the first resolves) drop the stale listener.
 * - sendChat failure surfaces as an error turn; the transcript carries prior context + system.
 */

import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { useAiChatStream } from './use-ai-chat-stream'
import type { AiChatStreamChunk, AiChatMessage } from '../../lib/types'

/** Controllable rAF: queued callbacks fire only when `runFrame()` is called. */
let frameQueue: FrameRequestCallback[] = []
function runFrame() {
  const queued = frameQueue
  frameQueue = []
  queued.forEach((cb) => cb(performance.now()))
}

beforeEach(() => {
  frameQueue = []
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    frameQueue.push(cb)
    return frameQueue.length
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frameQueue[id - 1] = () => {}
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** A subscribe stub that captures the listener so the test can feed scripted chunks. */
function makeHarness(options?: {
  sendChat?: ReturnType<typeof vi.fn>
  cancelChat?: ReturnType<typeof vi.fn>
  unsubscribe?: ReturnType<typeof vi.fn>
  systemPrompt?: string | null
  providerId?: string | null
  toolsEnabled?: boolean
  conversationId?: string | null
}) {
  let feed: ((chunk: AiChatStreamChunk) => void) | null = null
  const unsubscribe = options?.unsubscribe ?? vi.fn()
  const sendChat =
    options?.sendChat ?? vi.fn(() => Promise.resolve({ runId: 'run-1' }))
  const cancelChat =
    options?.cancelChat ?? vi.fn(() => Promise.resolve({ cancelled: true }))
  const subscribe = vi.fn(
    (_runId: string, listener: (chunk: AiChatStreamChunk) => void) => {
      feed = listener
      return Promise.resolve(unsubscribe as () => void)
    },
  )
  const hook = renderHook(() =>
    useAiChatStream({
      sendChat: sendChat as (request: {
        messages: AiChatMessage[]
        providerId?: string | null
        toolsEnabled?: boolean
        conversationId?: string | null
        messageId?: string | null
      }) => Promise<{ runId: string }>,
      cancelChat: cancelChat as (
        runId: string,
      ) => Promise<{ cancelled: boolean }>,
      subscribe,
      systemPrompt: options?.systemPrompt,
      providerId: options?.providerId,
      toolsEnabled: options?.toolsEnabled,
      conversationId: options?.conversationId,
    }),
  )
  return {
    hook,
    sendChat,
    cancelChat,
    subscribe,
    unsubscribe,
    feed: (chunk: AiChatStreamChunk) => {
      if (!feed) throw new Error('not subscribed yet')
      act(() => feed?.(chunk))
    },
  }
}

/** Flush microtasks so awaited sendChat/subscribe promises settle. */
async function settle() {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

describe('useAiChatStream', () => {
  test('ignores blank sends and re-entrant sends while streaming', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('   '))
    expect(h.hook.result.current.messages).toHaveLength(0)
    expect(h.sendChat).not.toHaveBeenCalled()

    act(() => h.hook.result.current.send('first'))
    await settle()
    expect(h.sendChat).toHaveBeenCalledTimes(1)
    // Re-entrant send while streaming is dropped.
    act(() => h.hook.result.current.send('second'))
    expect(h.sendChat).toHaveBeenCalledTimes(1)
  })

  test('accumulates token, reasoning, and tool-call chunks into the active turn', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('hello'))
    await settle()

    expect(h.hook.result.current.messages).toHaveLength(2)
    expect(h.hook.result.current.streaming).toBe(true)
    expect(h.hook.result.current.awaitingFirstChunk).toBe(true)

    h.feed({ kind: 'reasoning', text: 'let me ' })
    h.feed({ kind: 'reasoning', text: 'think' })
    h.feed({ kind: 'token', text: 'The ' })
    h.feed({ kind: 'token', text: 'answer' })
    h.feed({ kind: 'toolCall', name: 'search_bm25', arguments: '{"q":"x"}' })
    // Nothing has flushed yet — all coalesced into the pending frame.
    const assistantBefore = h.hook.result.current.messages[1]
    expect(assistantBefore.content).toBe('')

    act(() => runFrame())

    const assistant = h.hook.result.current.messages[1]
    expect(assistant.content).toBe('The answer')
    expect(assistant.reasoning).toBe('let me think')
    expect(assistant.toolCalls).toHaveLength(1)
    expect(assistant.toolCalls?.[0].name).toBe('search_bm25')
    expect(assistant.toolCalls?.[0].arguments).toBe('{"q":"x"}')
    expect(h.hook.result.current.awaitingFirstChunk).toBe(false)
  })

  test('coalesces a burst of chunks into exactly one flush per frame', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('hi'))
    await settle()

    // 50 tokens before any frame fires.
    for (let i = 0; i < 50; i += 1) h.feed({ kind: 'token', text: 'x' })
    // Exactly one frame callback is queued despite 50 chunks.
    expect(frameQueue.length).toBe(1)

    act(() => runFrame())
    expect(h.hook.result.current.messages[1].content).toBe('x'.repeat(50))
  })

  test('finalizes on done and unsubscribes', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'final' })
    h.feed({ kind: 'done' })

    const assistant = h.hook.result.current.messages[1]
    expect(assistant.status).toBe('done')
    expect(assistant.content).toBe('final')
    expect(h.hook.result.current.streaming).toBe(false)
    expect(h.unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('finalizes on error with the error message', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'partial' })
    h.feed({ kind: 'error', message: 'provider exploded' })

    const assistant = h.hook.result.current.messages[1]
    expect(assistant.status).toBe('error')
    expect(assistant.error).toBe('provider exploded')
    expect(assistant.content).toBe('partial')
    expect(h.hook.result.current.streaming).toBe(false)
    expect(h.unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('cancel calls cancelChat, finalizes as cancelled, and drops late chunks', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'half' })
    act(() => runFrame())

    act(() => h.hook.result.current.cancel())
    expect(h.cancelChat).toHaveBeenCalledWith('run-1')
    expect(h.hook.result.current.messages[1].status).toBe('cancelled')
    expect(h.hook.result.current.messages[1].content).toBe('half')
    expect(h.hook.result.current.streaming).toBe(false)
    expect(h.unsubscribe).toHaveBeenCalledTimes(1)

    // A late chunk from the cancelled run is ignored (generation guard).
    h.feed({ kind: 'token', text: 'too late' })
    act(() => runFrame())
    expect(h.hook.result.current.messages[1].content).toBe('half')
  })

  test('cancel is a no-op when not streaming', () => {
    const h = makeHarness()
    act(() => h.hook.result.current.cancel())
    expect(h.cancelChat).not.toHaveBeenCalled()
  })

  test('cancel swallows a rejected cancelChat without throwing', async () => {
    const cancelChat = vi.fn().mockRejectedValue(new Error('cancel failed'))
    const h = makeHarness({ cancelChat })
    act(() => h.hook.result.current.send('q'))
    await settle()
    act(() => h.hook.result.current.cancel())
    await settle()
    expect(h.hook.result.current.messages[1].status).toBe('cancelled')
  })

  test('surfaces a sendChat failure as an error turn', async () => {
    const sendChat = vi.fn().mockRejectedValue(new Error('no provider'))
    const h = makeHarness({ sendChat })
    act(() => h.hook.result.current.send('q'))
    await settle()
    const assistant = h.hook.result.current.messages[1]
    expect(assistant.status).toBe('error')
    expect(assistant.error).toBe('no provider')
    expect(h.hook.result.current.streaming).toBe(false)
  })

  test('coerces a non-Error sendChat rejection to a string message', async () => {
    const sendChat = vi.fn().mockRejectedValue('string failure')
    const h = makeHarness({ sendChat })
    act(() => h.hook.result.current.send('q'))
    await settle()
    expect(h.hook.result.current.messages[1].error).toBe('string failure')
  })

  test('builds the transcript with system prompt and prior context', async () => {
    let captured: AiChatMessage[] = []
    const sendChat = vi.fn((request: { messages: AiChatMessage[] }) => {
      captured = request.messages
      return Promise.resolve({ runId: 'run-1' })
    })
    const h = makeHarness({ sendChat, systemPrompt: 'be terse' })

    act(() => h.hook.result.current.send('first question'))
    await settle()
    h.feed({ kind: 'token', text: 'first answer' })
    h.feed({ kind: 'done' })

    act(() => h.hook.result.current.send('second question'))
    await settle()

    expect(captured).toEqual([
      { role: 'system', content: 'be terse' },
      { role: 'user', content: 'first question' },
      { role: 'assistant', content: 'first answer' },
      { role: 'user', content: 'second question' },
    ])
  })

  test('omits an empty system prompt and skips empty assistant turns in the transcript', async () => {
    let captured: AiChatMessage[] = []
    const sendChat = vi.fn((request: { messages: AiChatMessage[] }) => {
      captured = request.messages
      return Promise.resolve({ runId: 'run-1' })
    })
    const h = makeHarness({ sendChat, systemPrompt: '   ' })

    act(() => h.hook.result.current.send('only question'))
    await settle()
    // No content produced; finalize the (empty) turn.
    h.feed({ kind: 'done' })

    act(() => h.hook.result.current.send('next question'))
    await settle()

    // Empty assistant turn is not echoed back; no system message because it was blank.
    expect(captured).toEqual([
      { role: 'user', content: 'only question' },
      { role: 'user', content: 'next question' },
    ])
  })

  test('forwards the configured providerId to sendChat', async () => {
    const h = makeHarness({ providerId: 'llm-local' })
    act(() => h.hook.result.current.send('q'))
    await settle()
    expect(h.sendChat).toHaveBeenCalledWith(
      expect.objectContaining({ providerId: 'llm-local' }),
    )
  })

  test('drops the listener when a turn is superseded before subscribe resolves', async () => {
    // sendChat resolves but subscribe is deferred so we can supersede mid-subscribe.
    let resolveSubscribe: ((unsub: () => void) => void) | null = null
    const unsubscribe = vi.fn()
    const subscribe = vi.fn(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve
        }),
    )
    const sendChat = vi.fn().mockResolvedValue({ runId: 'run-1' })
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )

    act(() => hook.result.current.send('q'))
    await settle()
    // Supersede via cancel before subscribe resolves.
    act(() => hook.result.current.cancel())
    // Now resolve the stale subscribe; its unsubscribe must be dropped immediately.
    act(() => resolveSubscribe?.(unsubscribe))
    await settle()
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('unsubscribes when a terminal chunk finalizes before subscribe resolves (no leak)', async () => {
    // The listener can fire before subscribe's promise settles. A `done` chunk then finalizes
    // the turn; when the deferred subscribe finally resolves, its unsubscribe must be dropped
    // immediately rather than stashed for an already-finished turn (a live-listener leak).
    let resolveSubscribe: ((unsub: () => void) => void) | null = null
    let feed: ((chunk: AiChatStreamChunk) => void) | null = null
    const unsubscribe = vi.fn()
    const subscribe = vi.fn(
      (_runId: string, listener: (chunk: AiChatStreamChunk) => void) => {
        feed = listener
        return new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve
        })
      },
    )
    const sendChat = vi.fn().mockResolvedValue({ runId: 'run-1' })
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )

    act(() => hook.result.current.send('q'))
    await settle()
    // Terminal chunk arrives via the captured listener BEFORE subscribe's promise resolves.
    act(() => feed?.({ kind: 'done' }))
    expect(hook.result.current.messages[1].status).toBe('done')
    expect(hook.result.current.streaming).toBe(false)

    // Now the deferred subscribe resolves; the post-finalize generation bump must make the
    // resolve drop the listener instead of leaking it.
    act(() => resolveSubscribe?.(unsubscribe))
    await settle()
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('flushes synchronously when requestAnimationFrame is unavailable', async () => {
    vi.stubGlobal('requestAnimationFrame', undefined)
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'sync' })
    // No frame needed — flush ran inline.
    expect(h.hook.result.current.messages[1].content).toBe('sync')
  })

  test('cancel before sendChat resolves finalizes without calling cancelChat', () => {
    // sendChat never resolves, so runIdRef stays null when cancel runs.
    const sendChat = vi.fn(() => new Promise<{ runId: string }>(() => {}))
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const subscribe = vi.fn().mockResolvedValue(vi.fn())
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )
    act(() => hook.result.current.send('q'))
    act(() => hook.result.current.cancel())
    // No run id yet → cancelChat is not called, but the turn is still finalized.
    expect(cancelChat).not.toHaveBeenCalled()
    expect(hook.result.current.messages[1].status).toBe('cancelled')
    expect(hook.result.current.streaming).toBe(false)
  })

  test('ignores a sendChat resolution for a superseded turn', async () => {
    // sendChat resolves only after we supersede; the run id / subscription are dropped.
    let resolveSend: ((ack: { runId: string }) => void) | null = null
    const sendChat = vi.fn(
      () =>
        new Promise<{ runId: string }>((resolve) => {
          resolveSend = resolve
        }),
    )
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const subscribe = vi.fn().mockResolvedValue(vi.fn())
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )
    act(() => hook.result.current.send('q'))
    act(() => hook.result.current.cancel())
    act(() => resolveSend?.({ runId: 'run-late' }))
    await settle()
    // Superseded: subscribe was never reached for this resolution.
    expect(subscribe).not.toHaveBeenCalled()
    expect(hook.result.current.messages[1].status).toBe('cancelled')
  })

  test('ignores a sendChat rejection for a superseded turn', async () => {
    let rejectSend: ((reason: unknown) => void) | null = null
    const sendChat = vi.fn(
      () =>
        new Promise<{ runId: string }>((_resolve, reject) => {
          rejectSend = reject
        }),
    )
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const subscribe = vi.fn().mockResolvedValue(vi.fn())
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )
    act(() => hook.result.current.send('q'))
    act(() => hook.result.current.cancel())
    act(() => rejectSend?.(new Error('late failure')))
    await settle()
    // The cancel status survives; the stale rejection does not overwrite it with an error.
    expect(hook.result.current.messages[1].status).toBe('cancelled')
    expect(hook.result.current.messages[1].error).toBeUndefined()
  })

  test('tears down subscription and pending frame on unmount', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'x' }) // schedules a frame
    expect(frameQueue.length).toBe(1)
    act(() => h.hook.unmount())
    expect(h.unsubscribe).toHaveBeenCalledTimes(1)
  })

  test('seeds the transcript from initialMessages on first mount', () => {
    const sendChat = vi.fn().mockResolvedValue({ runId: 'run-1' })
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    const subscribe = vi.fn().mockResolvedValue(vi.fn())
    const hook = renderHook(() =>
      useAiChatStream({
        sendChat,
        cancelChat,
        subscribe,
        initialMessages: [
          { id: 'h1', role: 'user', content: 'earlier prompt' },
          {
            id: 'h2',
            role: 'assistant',
            content: 'earlier answer',
            status: 'done',
          },
        ],
      }),
    )
    expect(hook.result.current.messages).toHaveLength(2)
    expect(hook.result.current.messages[0].content).toBe('earlier prompt')
    // A blank-default mount seeds an empty transcript.
    const empty = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe }),
    )
    expect(empty.result.current.messages).toHaveLength(0)
  })

  test('invokes onTurnFinalized with the finalized transcript on done', async () => {
    const onTurnFinalized = vi.fn()
    const sendChat = vi.fn().mockResolvedValue({ runId: 'run-fin' })
    const cancelChat = vi.fn().mockResolvedValue({ cancelled: true })
    let feed: ((chunk: AiChatStreamChunk) => void) | null = null
    const subscribe = vi.fn(
      (_runId: string, listener: (chunk: AiChatStreamChunk) => void) => {
        feed = listener
        return Promise.resolve(vi.fn() as () => void)
      },
    )
    const hook = renderHook(() =>
      useAiChatStream({ sendChat, cancelChat, subscribe, onTurnFinalized }),
    )

    act(() => hook.result.current.send('persist me'))
    await settle()
    act(() => feed?.({ kind: 'token', text: 'answer text' }))
    act(() => feed?.({ kind: 'done' }))
    // The microtask-scheduled callback fires after a flush.
    await settle()

    expect(onTurnFinalized).toHaveBeenCalledTimes(1)
    const lastArg = onTurnFinalized.mock.calls[0][0] as Array<{
      role: string
      content: string
      status?: string
    }>
    expect(lastArg).toHaveLength(2)
    expect(lastArg[0]).toMatchObject({ role: 'user', content: 'persist me' })
    expect(lastArg[1]).toMatchObject({
      role: 'assistant',
      content: 'answer text',
      status: 'done',
    })
  })

  test('reset clears the transcript, tears down a live turn, and cancels the run', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'partial' })
    act(() => runFrame())
    expect(h.hook.result.current.messages).toHaveLength(2)

    act(() => h.hook.result.current.reset())
    expect(h.hook.result.current.messages).toHaveLength(0)
    expect(h.hook.result.current.streaming).toBe(false)
    // The live run is best-effort cancelled and its subscription torn down.
    expect(h.cancelChat).toHaveBeenCalledWith('run-1')
    expect(h.unsubscribe).toHaveBeenCalledTimes(1)

    // A late chunk after reset is dropped (generation guard).
    h.feed({ kind: 'token', text: 'too late' })
    act(() => runFrame())
    expect(h.hook.result.current.messages).toHaveLength(0)
  })

  test('reset swallows a rejected cancelChat from the torn-down run', async () => {
    const cancelChat = vi.fn().mockRejectedValue(new Error('cancel failed'))
    const h = makeHarness({ cancelChat })
    act(() => h.hook.result.current.send('q'))
    await settle()
    // Reset on a live run best-effort cancels it; a rejected cancel must not throw.
    act(() => h.hook.result.current.reset())
    await settle()
    expect(cancelChat).toHaveBeenCalledWith('run-1')
    expect(h.hook.result.current.messages).toHaveLength(0)
  })

  test('reset hydrates a past conversation when given messages', () => {
    const h = makeHarness()
    act(() =>
      h.hook.result.current.reset([
        { id: 'p1', role: 'user', content: 'opened from history' },
        {
          id: 'p2',
          role: 'assistant',
          content: 'hydrated answer',
          status: 'done',
        },
      ]),
    )
    expect(h.hook.result.current.messages).toHaveLength(2)
    expect(h.hook.result.current.messages[1].content).toBe('hydrated answer')
    // reset on an idle hook (no live run) does not call cancelChat.
    expect(h.cancelChat).not.toHaveBeenCalled()
  })

  // ---------------------------------------------------------------------------
  // W-AI-7: agent observability chunks (toolResult / usage / citations) + the
  // tools-enabled send wiring. All still go through the SAME rAF flush.
  // ---------------------------------------------------------------------------

  test('correlates a toolResult back to its toolCall by callId and marks success', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()

    h.feed({
      kind: 'toolCall',
      name: 'search_bm25',
      arguments: '{"q":"x"}',
      callId: 'call-1',
    })
    act(() => runFrame())
    // The call starts pending (no result yet).
    expect(h.hook.result.current.messages[1].toolCalls?.[0].status).toBe(
      'pending',
    )

    h.feed({
      kind: 'toolResult',
      callId: 'call-1',
      name: 'search_bm25',
      result: '3 rows',
      isError: false,
    })
    act(() => runFrame())
    const call = h.hook.result.current.messages[1].toolCalls?.[0]
    expect(call?.status).toBe('success')
    expect(call?.result).toBe('3 rows')
    expect(call?.isError).toBe(false)
  })

  test('a code-mode toolResult populates codeSource/hostCalls/limitsHit on the matching call', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({
      kind: 'toolCall',
      name: 'run_code',
      arguments: '{"source":"…"}',
      callId: 'code-1',
    })
    h.feed({
      kind: 'toolResult',
      callId: 'code-1',
      name: 'run_code',
      result: '8 rust pages.',
      isError: false,
      codeSource:
        'const r = await query_history({ query: "rust" }); return r.length;',
      hostCalls: [
        {
          function: 'query_history',
          query: 'rust',
          plane: 'hybrid',
          limit: 8,
          argsSummary: 'query="rust" plane=hybrid limit=8',
          rowCount: 12,
        },
      ],
      limitsHit: 'output',
    })
    act(() => runFrame())
    const call = h.hook.result.current.messages[1].toolCalls?.[0]
    expect(call?.status).toBe('success')
    expect(call?.result).toBe('8 rust pages.')
    expect(call?.codeSource).toContain('query_history')
    expect(call?.hostCalls).toHaveLength(1)
    expect(call?.hostCalls?.[0]).toMatchObject({
      function: 'query_history',
      query: 'rust',
      plane: 'hybrid',
      limit: 8,
      rowCount: 12,
    })
    expect(call?.limitsHit).toBe('output')
  })

  test('a non-code toolResult leaves the code-mode fields undefined', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({
      kind: 'toolCall',
      name: 'search_bm25',
      arguments: '{"q":"x"}',
      callId: 'call-1',
    })
    h.feed({
      kind: 'toolResult',
      callId: 'call-1',
      name: 'search_bm25',
      result: '3 rows',
      isError: false,
    })
    act(() => runFrame())
    const call = h.hook.result.current.messages[1].toolCalls?.[0]
    expect(call?.status).toBe('success')
    // The search step carries no code-mode fields, so a search step renders exactly as W-AI-7.
    expect(call?.codeSource).toBeUndefined()
    expect(call?.hostCalls).toBeUndefined()
    expect(call?.limitsHit).toBeUndefined()
  })

  test('a failed toolResult marks the matching call as error with isError true', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({
      kind: 'toolCall',
      name: 'no_such_tool',
      arguments: '{}',
      callId: 'call-x',
    })
    h.feed({
      kind: 'toolResult',
      callId: 'call-x',
      name: 'no_such_tool',
      result: 'failed: unknown tool',
      isError: true,
    })
    act(() => runFrame())
    const call = h.hook.result.current.messages[1].toolCalls?.[0]
    expect(call?.status).toBe('error')
    expect(call?.isError).toBe(true)
    expect(call?.result).toBe('failed: unknown tool')
  })

  test('matches the LAST pending call with a reused callId', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    // Two calls share an id (a model reusing an id across turns); the result should resolve the
    // most-recent pending one, leaving the earlier one pending.
    h.feed({ kind: 'toolCall', name: 'a', arguments: '{}', callId: 'dup' })
    h.feed({ kind: 'toolCall', name: 'b', arguments: '{}', callId: 'dup' })
    h.feed({
      kind: 'toolResult',
      callId: 'dup',
      name: 'b',
      result: 'b done',
      isError: false,
    })
    act(() => runFrame())
    const calls = h.hook.result.current.messages[1].toolCalls
    expect(calls?.[0].status).toBe('pending')
    expect(calls?.[1].status).toBe('success')
    expect(calls?.[1].result).toBe('b done')
  })

  test('drops a toolResult that matches no pending call (no fabricated row)', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'hi' })
    act(() => runFrame())
    // A toolResult with an unknown callId is ignored: no tool rows appear.
    h.feed({
      kind: 'toolResult',
      callId: 'ghost',
      name: 'x',
      result: 'r',
      isError: false,
    })
    act(() => runFrame())
    expect(h.hook.result.current.messages[1].toolCalls).toHaveLength(0)
  })

  test('accumulates usage across multiple usage chunks', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'usage', promptTokens: 10, completionTokens: 5 })
    h.feed({ kind: 'usage', promptTokens: 7, completionTokens: 3 })
    act(() => runFrame())
    expect(h.hook.result.current.messages[1].usage).toEqual({
      promptTokens: 17,
      completionTokens: 8,
    })
  })

  test('stores the terminal citations set and seals it on done', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'token', text: 'answer' })
    h.feed({
      kind: 'citations',
      citations: [
        {
          historyId: 1,
          profileId: 'p',
          url: 'https://a.example/x',
          title: 'A',
          visitedAt: '2026-01-01T00:00:00Z',
          score: 0.9,
          canonicalUrl: 'https://a.example/x',
        },
      ],
    })
    h.feed({ kind: 'done' })
    const assistant = h.hook.result.current.messages[1]
    expect(assistant.status).toBe('done')
    expect(assistant.citations).toHaveLength(1)
    expect(assistant.citations?.[0].canonicalUrl).toBe('https://a.example/x')
  })

  test('all four W-AI-7 chunks before any frame coalesce into a single flush', async () => {
    const h = makeHarness()
    act(() => h.hook.result.current.send('q'))
    await settle()
    h.feed({ kind: 'toolCall', name: 't', arguments: '{}', callId: 'c1' })
    h.feed({
      kind: 'toolResult',
      callId: 'c1',
      name: 't',
      result: 'r',
      isError: false,
    })
    h.feed({ kind: 'usage', promptTokens: 1, completionTokens: 1 })
    h.feed({
      kind: 'citations',
      citations: [
        {
          historyId: 2,
          profileId: 'p',
          url: 'https://b.example/',
          visitedAt: '2026-02-02T00:00:00Z',
        },
      ],
    })
    // The no-freeze contract: a burst of W-AI-7 chunks queues exactly ONE frame.
    expect(frameQueue.length).toBe(1)
    act(() => runFrame())
    const assistant = h.hook.result.current.messages[1]
    expect(assistant.toolCalls?.[0].result).toBe('r')
    expect(assistant.usage).toEqual({ promptTokens: 1, completionTokens: 1 })
    expect(assistant.citations).toHaveLength(1)
  })

  type SendRequest = {
    messages: AiChatMessage[]
    providerId?: string | null
    toolsEnabled?: boolean
    conversationId?: string | null
    messageId?: string | null
  }

  test('threads toolsEnabled + conversationId + the assistant messageId into sendChat', async () => {
    const sendChat = vi.fn<
      (request: SendRequest) => Promise<{ runId: string }>
    >(() => Promise.resolve({ runId: 'run-1' }))
    const h = makeHarness({
      sendChat,
      toolsEnabled: true,
      conversationId: 'conv-7',
    })
    act(() => h.hook.result.current.send('hello'))
    await settle()
    expect(sendChat).toHaveBeenCalledTimes(1)
    const request = sendChat.mock.calls[0][0]
    expect(request.toolsEnabled).toBe(true)
    expect(request.conversationId).toBe('conv-7')
    // The messageId is the active assistant message's id, so the trace links to this turn.
    const assistantId = h.hook.result.current.messages[1].id
    expect(request.messageId).toBe(assistantId)
  })

  test('plain-chat send omits tools (toolsEnabled undefined) and carries no conversation link', async () => {
    const sendChat = vi.fn<
      (request: SendRequest) => Promise<{ runId: string }>
    >(() => Promise.resolve({ runId: 'run-1' }))
    const h = makeHarness({ sendChat })
    act(() => h.hook.result.current.send('hello'))
    await settle()
    const request = sendChat.mock.calls[0][0]
    expect(request.toolsEnabled).toBeUndefined()
    expect(request.conversationId).toBeUndefined()
  })
})
