/**
 * Events the desktop backend sends (`pathkeep://…`): Tauri events in the app,
 * and through the dev bridge (E2E, `dev:demo`) the same events as server-sent
 * events on `/events`, so a browser sees what the app's window sees.
 *
 * Responsible for: one way to listen in both. Not responsible for: what an
 * event means (the `subscribeTo…` helpers next to this file).
 *
 * A listener gets what is sent after it is listening: start listening before
 * starting the work that sends the events.
 */
import { hasTauriGuestApi, resolveDevIpcBridgeUrl } from '../runtime'

let bridgeStream: Promise<EventSource> | null = null

/** The bridge's event stream, opened once, resolved when connected. */
function connectToBridge(url: string) {
  bridgeStream ??= new Promise<EventSource>((resolve, reject) => {
    const source = new EventSource(`${url}/events`)
    source.addEventListener('open', () => resolve(source), { once: true })
    source.addEventListener('error', () => {
      // It reconnects by itself unless it gave up.
      if (source.readyState !== EventSource.CLOSED) return
      bridgeStream = null
      reject(new Error(`The desktop bridge at ${url} sent no events.`))
    })
  })
  return bridgeStream
}

/** Listens to event `name` until the returned function is called. */
export async function listenDesktopEvent<T>(
  name: string,
  listener: (payload: T) => void,
): Promise<() => void> {
  try {
    if (hasTauriGuestApi()) {
      const { listen } = await import('@tauri-apps/api/event')
      return await listen<T>(name, ({ payload }) => listener(payload))
    }
    const url = resolveDevIpcBridgeUrl()
    if (!url) return () => {}
    const source = await connectToBridge(url)
    const handle = (message: MessageEvent<string>) =>
      listener(JSON.parse(message.data) as T)
    source.addEventListener(name, handle)
    return () => source.removeEventListener(name, handle)
  } catch {
    // Without events the work still runs; only its progress is not shown.
    return () => {}
  }
}
