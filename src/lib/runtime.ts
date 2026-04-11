import { isTauri } from '@tauri-apps/api/core'

export type AppRuntime = 'tauri' | 'browser-desktop-bridge' | 'browser-preview'

export const DEV_IPC_URL_ENV = 'VITE_PATHKEEP_DEV_IPC_URL'

export function resolveDevIpcBridgeUrl() {
  const raw = import.meta.env.VITE_PATHKEEP_DEV_IPC_URL
  if (typeof raw !== 'string') {
    return null
  }

  const trimmed = raw.trim().replace(/\/+$/, '')
  return trimmed.length > 0 ? trimmed : null
}

export function resolveAppRuntime(): AppRuntime {
  if (isTauri()) {
    return 'tauri'
  }

  return resolveDevIpcBridgeUrl() ? 'browser-desktop-bridge' : 'browser-preview'
}

export function hasDesktopCommandTransport() {
  return resolveAppRuntime() !== 'browser-preview'
}

export function hasTauriGuestApi() {
  return isTauri()
}
