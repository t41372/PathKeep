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

type TauriInternalsShape = {
  invoke?: unknown
}

/**
 * Returns whether the renderer itself is already running inside a Tauri webview.
 *
 * The Tauri app serves the front-end from `tauri://localhost`, so the protocol
 * is another reliable desktop-only signal even when the injected globals lag
 * behind or a dev host omits `globalThis.isTauri`.
 */
function hasTauriLocation() {
  return (
    typeof globalThis === 'object' &&
    globalThis !== null &&
    typeof globalThis.location?.protocol === 'string' &&
    globalThis.location.protocol === 'tauri:'
  )
}

/**
 * Returns the low-level Tauri IPC internals injected into the webview.
 *
 * Tauri v2 can provide a working `window.__TAURI_INTERNALS__` bridge even when
 * `globalThis.isTauri` is falsy, so real desktop sessions must treat that as a
 * first-class signal instead of falling back to preview fixtures.
 */
function resolveTauriInternals(): TauriInternalsShape | null {
  const candidate = (
    globalThis as typeof globalThis & {
      __TAURI_INTERNALS__?: TauriInternalsShape
    }
  ).__TAURI_INTERNALS__

  if (typeof candidate !== 'object' || candidate === null) {
    return null
  }

  return candidate
}

/**
 * Returns whether the current renderer has a working Tauri IPC bridge.
 *
 * This stays separate from the upstream `isTauri()` helper because PathKeep's
 * dev desktop webview may expose `__TAURI_INTERNALS__` without also defining
 * the legacy `globalThis.isTauri` flag.
 */
function hasTauriIpcBridge() {
  return (
    isTauri() ||
    hasTauriLocation() ||
    typeof resolveTauriInternals()?.invoke === 'function'
  )
}

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
  if (hasTauriIpcBridge()) {
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
  return hasTauriIpcBridge()
}
