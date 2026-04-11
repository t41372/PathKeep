/**
 * This module detects whether the front-end is running inside Tauri, the browser desktop bridge, or the lightweight browser preview.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `AppRuntime`
 * - `DEV_IPC_URL_ENV`
 * - `resolveDevIpcBridgeUrl`
 * - `resolveAppRuntime`
 * - `hasDesktopCommandTransport`
 * - `hasTauriGuestApi`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { isTauri } from '@tauri-apps/api/core'

/**
 * Defines the type-level contract for app runtime.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type AppRuntime = 'tauri' | 'browser-desktop-bridge' | 'browser-preview'

export const DEV_IPC_URL_ENV = 'VITE_PATHKEEP_DEV_IPC_URL'

/**
 * Resolves dev ipc bridge url from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resolveDevIpcBridgeUrl() {
  const raw = import.meta.env.VITE_PATHKEEP_DEV_IPC_URL
  if (typeof raw !== 'string') {
    return null
  }

  const trimmed = raw.trim().replace(/\/+$/, '')
  return trimmed.length > 0 ? trimmed : null
}

/**
 * Resolves app runtime from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function resolveAppRuntime(): AppRuntime {
  if (isTauri()) {
    return 'tauri'
  }

  return resolveDevIpcBridgeUrl() ? 'browser-desktop-bridge' : 'browser-preview'
}

/**
 * Returns whether desktop command transport.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function hasDesktopCommandTransport() {
  return resolveAppRuntime() !== 'browser-preview'
}

/**
 * Returns whether tauri guest api.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function hasTauriGuestApi() {
  return isTauri()
}
