/**
 * @file ai-stream.test.ts
 * @description Focused coverage for the streaming chat event subscription helper (W-AI-1).
 * @module lib/ipc
 *
 * ## Responsibilities
 * - Verify the helper subscribes to the exact desktop event channel.
 * - Verify only chunks for the requested run id are forwarded, and empty payloads are ignored.
 * - Verify transport failures degrade to a no-op unsubscribe.
 *
 * ## Not responsible for
 * - Re-testing chat route rendering or chunk-to-UI mapping.
 * - Re-testing the Tauri event implementation itself.
 *
 * ## Dependencies
 * - Mocks `@tauri-apps/api/event` at the module boundary.
 *
 * ## Performance notes
 * - Pure unit test; no desktop process or IO.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { subscribeToAiChatStream } from './ai-stream'
import type { AiChatStreamChunk } from '../types'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToAiChatStream', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop ai-stream channel and forwards matching-run chunks', async () => {
    const unsubscribe = vi.fn()
    const received: AiChatStreamChunk[] = []
    listen.mockImplementation((_event, handler) => {
      // A reasoning chunk and a token for our run, plus a chunk for another run and an empty
      // payload that must both be ignored, then a terminal done.
      handler({
        payload: {
          runId: 'run-1',
          chunk: { kind: 'reasoning', text: 'thinking' },
        },
      })
      handler({
        payload: { runId: 'run-2', chunk: { kind: 'token', text: 'other' } },
      })
      handler({
        payload: { runId: 'run-1', chunk: { kind: 'token', text: 'hello' } },
      })
      handler({ payload: null })
      handler({ payload: { runId: 'run-1', chunk: { kind: 'done' } } })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToAiChatStream('run-1', (chunk) => {
      received.push(chunk)
    })

    expect(listen).toHaveBeenCalledWith(
      'pathkeep://ai-stream',
      expect.any(Function),
    )
    expect(received).toEqual([
      { kind: 'reasoning', text: 'thinking' },
      { kind: 'token', text: 'hello' },
      { kind: 'done' },
    ])
    expect(result).toBe(unsubscribe)
  })

  test('returns a noop unsubscribe when the desktop event bridge is unavailable', async () => {
    const listener = vi.fn()
    listen.mockRejectedValueOnce(new Error('event bridge unavailable'))

    const result = await subscribeToAiChatStream('run-x', listener)

    expect(result()).toBeUndefined()
    expect(listener).not.toHaveBeenCalled()
  })
})
