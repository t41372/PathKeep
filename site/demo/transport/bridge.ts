/**
 * Stands in for `src/lib/ipc/bridge.ts` in the demo build: commands go to
 * the demo's backend instead of the desktop app.
 */
import type * as Original from '@/lib/ipc/bridge'
import { answer } from '../backend'

export type CommandPayload = Original.CommandPayload

export const invokeCommand: typeof Original.invokeCommand = async <TResponse>(
  command: string,
  payload?: CommandPayload,
) => (await answer(command, payload)) as TResponse
