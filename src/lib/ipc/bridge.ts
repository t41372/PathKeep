import { invoke } from '@tauri-apps/api/core'

export type CommandPayload = Record<string, unknown> | undefined

export async function invokeCommand<TResponse>(
  command: string,
  payload?: CommandPayload,
) {
  return invoke<TResponse>(command, payload)
}
