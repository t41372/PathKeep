import { invokeCommand } from '../ipc/bridge'
import { hasDesktopCommandTransport } from '../runtime'

type BackendArgs = Record<string, unknown> | undefined

export async function call<T>(command: string, args?: BackendArgs): Promise<T> {
  if (hasDesktopCommandTransport()) {
    return invokeCommand<T>(command, args)
  }

  const { backendTestHarness } = await import('../backend')
  return backendTestHarness.call<T>(command, args)
}
