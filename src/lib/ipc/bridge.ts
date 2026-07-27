/**
 * This module contains reusable front-end helper logic for Ipc.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `CommandPayload`
 * - `invokeCommand`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { invoke } from '@tauri-apps/api/core'
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'
import { CommandInvokeError } from './command-error'

/**
 * Converts a backend `CommandError` envelope (`{ message, code?, actionHint?,
 * retryHint? }`) into the typed error routes consume, or `null` when the
 * rejected value has some other shape.
 */
function commandInvokeErrorFromEnvelope(
  value: unknown,
): CommandInvokeError | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>
  const message =
    typeof record.message === 'string' && record.message.length > 0
      ? record.message
      : typeof record.error === 'string' && record.error.length > 0
        ? record.error
        : null
  if (message === null) return null
  return new CommandInvokeError(message, {
    code: typeof record.code === 'string' ? record.code : null,
    actionHint:
      typeof record.actionHint === 'string' ? record.actionHint : null,
    retryHint: typeof record.retryHint === 'string' ? record.retryHint : null,
  })
}

/**
 * Defines the type-level contract for command payload.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type CommandPayload = Record<string, unknown> | undefined

/**
 * Explains how invoke command works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function invokeCommand<TResponse>(
  command: string,
  payload?: CommandPayload,
) {
  if (hasTauriGuestApi()) {
    try {
      return await invoke<TResponse>(command, payload)
    } catch (error) {
      const envelope = commandInvokeErrorFromEnvelope(error)
      if (envelope) {
        throw envelope
      }

      if (error instanceof Error) {
        throw error
      }

      if (typeof error === 'string' && error.trim().length > 0) {
        throw new Error(error)
      }

      throw new Error(`PathKeep desktop command "${command}" failed.`)
    }
  }

  const bridgeUrl = resolveDevIpcBridgeUrl()
  if (!bridgeUrl) {
    throw new Error(
      `PathKeep desktop command "${command}" is unavailable in browser preview mode.`,
    )
  }

  let response: Response
  try {
    response = await fetch(
      `${bridgeUrl}/commands/${encodeURIComponent(command)}`,
      {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
        },
        body: JSON.stringify(payload),
      },
    )
  } catch (error) {
    const detail = error instanceof Error ? ` ${error.message}` : ''
    throw new Error(
      `PathKeep desktop command "${command}" could not reach the local desktop bridge at ${bridgeUrl}.${detail}`,
    )
  }

  const raw = await response.text()
  const data = raw.length > 0 ? (JSON.parse(raw) as unknown) : null

  if (!response.ok) {
    const envelope = commandInvokeErrorFromEnvelope(data)
    if (envelope) {
      throw envelope
    }
    throw new Error(
      `PathKeep desktop command "${command}" failed with HTTP ${response.status}.`,
    )
  }

  return data as TResponse
}
