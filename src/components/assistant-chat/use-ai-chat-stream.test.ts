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
      }) => Promise<{ runId: string }>,
      cancelChat: cancelChat as (
        runId: string,
      ) => Promise<{ cancelled: boolean }>,
      subscribe,
      systemPrompt: options?.systemPrompt,
      providerId: options?.providerId,
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
})
