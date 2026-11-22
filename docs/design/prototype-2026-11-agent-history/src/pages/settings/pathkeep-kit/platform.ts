/**
 * Prototype-only: which operating system the shared settings sections act as
 * if they run on. The real app reads it from the host (`isMacOsHost()` and the
 * backend's reports); here a switch in 一般, 安全 and 儲存 sets it, so
 * the rows and words that differ by system can be seen. Also the invented
 * paths, laid out the way the backend lays them out (`vault-core/src/config.rs`).
 */
import { useSyncExternalStore } from 'react'

export type Platform = 'macos' | 'windows' | 'linux'

export const platformNames: Record<Platform, string> = { macos: 'macOS', windows: 'Windows', linux: 'Linux' }

let current: Platform = 'macos'
const listeners = new Set<() => void>()

export function setPlatform(next: Platform) {
  current = next
  listeners.forEach((listener) => listener())
}

export function usePlatform() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => current,
  )
}

const homes: Record<Platform, string> = {
  macos: '/Users/mei',
  windows: 'C:\\Users\\Mei',
  linux: '/home/mei',
}

const roots: Record<Platform, string> = {
  macos: '/Users/mei/Library/Application Support/com.yi-ting.pathkeep',
  windows: 'C:\\Users\\Mei\\AppData\\Local\\yi-ting\\pathkeep\\data',
  linux: '/home/mei/.local/share/pathkeep',
}

/** Joins path parts with the platform's separator. */
export function pathIn(platform: Platform, ...parts: string[]) {
  const sep = platform === 'windows' ? '\\' : '/'
  return parts.join(sep)
}

/** The app root and the folders Settings shows under it. */
export function appPaths(platform: Platform) {
  const root = roots[platform]
  const at = (...parts: string[]) => pathIn(platform, root, ...parts)
  return {
    root,
    downloads: pathIn(platform, homes[platform], 'Downloads'),
    desktop: pathIn(platform, homes[platform], 'Desktop'),
    exports: at('exports'),
    logs: at('logs'),
    rustLog: at('logs', 'rust.log'),
    frontendLog: at('logs', 'frontend.log'),
    crashReports: at('diagnostics', 'crash-reports'),
    /** What the rekey preview names; a password change keeps no copy and names none. */
    rekeySnapshot: at('raw-snapshots', 'maintenance', 'rekey-<id>'),
    at,
  }
}

/** `keyringStatus.backend` as the backend names it. */
export const keyringBackends: Record<Platform, string> = {
  macos: 'macOS Keychain',
  windows: 'Windows Credential Manager',
  linux: 'Linux Secret Service / keyutils',
}

/** What the scheduler reports installed, for 刪除所有資料; Linux sets it up by hand, so nothing. */
export function scheduleItems(platform: Platform): string[] {
  if (platform === 'macos') return [pathIn(platform, homes.macos, 'Library', 'LaunchAgents', 'com.yi-ting.pathkeep.backup.plist')]
  if (platform === 'windows') return ['com.yi-ting.pathkeep.backup']
  return []
}
