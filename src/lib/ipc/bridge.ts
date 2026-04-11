import { invoke, isTauri } from '@tauri-apps/api/core'
import { resolveDevIpcBridgeUrl } from '../runtime'

export type CommandPayload = Record<string, unknown> | undefined

export async function invokeCommand<TResponse>(
  command: string,
  payload?: CommandPayload,
) {
  if (isTauri()) {
    return invoke<TResponse>(command, payload)
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
