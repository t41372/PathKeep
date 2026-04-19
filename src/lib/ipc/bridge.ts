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
    const message =
      typeof data === 'object' &&
      data !== null &&
      'error' in data &&
      typeof data.error === 'string'
        ? data.error
        : `PathKeep desktop command "${command}" failed with HTTP ${response.status}.`
    throw new Error(message)
  }

  return data as TResponse
}
