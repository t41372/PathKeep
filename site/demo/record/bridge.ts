/**
 * Recording build: the app's own bridge to the real backend, with every
 * command and its answer (or error) logged.
 */
import * as original from '@/lib/ipc/bridge'
import { CommandInvokeError } from '@/lib/ipc/command-error'
import { log } from './log'

export type CommandPayload = original.CommandPayload

export const invokeCommand: typeof original.invokeCommand = async <TResponse>(
  command: string,
  payload?: CommandPayload,
) => {
  try {
    const response = await original.invokeCommand<TResponse>(command, payload)
    log.calls.push({ command, args: payload ?? null, status: 200, response })
    return response
  } catch (error) {
    if (error instanceof CommandInvokeError)
      log.calls.push({
        command,
        args: payload ?? null,
        status: 500,
        response: {
          message: error.message,
          code: error.code,
          actionHint: error.actionHint,
          retryHint: error.retryHint,
        },
      })
    throw error
  }
}
