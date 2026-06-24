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
 * - `hasMacOverlayTitlebar`
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

/**
 * Returns whether the current renderer is hosted on macOS.
 *
 * `navigator.platform` is the most reliable signal still exposed in the Tauri
 * webview (the WKWebView user agent does report "Macintosh", but platform is
 * cheaper and unambiguous). Kept defensive so the helper is safe to call before
 * first paint and inside non-DOM test contexts.
 */
function isMacOsHost() {
  if (typeof navigator === 'undefined') {
    return false
  }
  const platform = navigator.platform || ''
  const userAgent = navigator.userAgent || ''
  return /Mac|iPhone|iPod|iPad/.test(platform) || /Mac OS X/.test(userAgent)
}

/**
 * Returns whether the macOS overlay title bar is in effect for this window.
 *
 * Why this helper exists:
 * - `tauri.conf.json` sets `titleBarStyle: "Overlay"`, which is a macOS-only
 *   flag: the native traffic lights float over the webview and the content
 *   extends under the ~28px title strip so the app background shows through
 *   instead of an opaque black bar. The shell must then reserve clearance for
 *   the traffic lights and the title strip — but ONLY when that overlay is
 *   actually present (real desktop window on macOS). In the browser preview,
 *   on Windows, and on Linux there is no overlay, so no offset is applied.
 *
 * Windows/Linux note: `titleBarStyle` is ignored off macOS, so those platforms
 * keep their native decorations and this returns false. A custom client-side
 * titlebar for Windows/Linux is a deliberate follow-up, not handled here.
 */
export function hasMacOverlayTitlebar() {
  return hasTauriIpcBridge() && isMacOsHost()
}
