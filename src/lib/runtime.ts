/**
 * This module detects whether the front-end is running inside Tauri, in a browser attached to the dev desktop bridge, or in a plain browser with no backend at all (where every command fails and the app shows its "could not start" screen).
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
 * - `windowFrame`
 * - `hasMacOverlayTitlebar`
 * - `drawsWindowControls`
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
export type AppRuntime = 'tauri' | 'browser-desktop-bridge' | 'browser-only'

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
 * first-class signal, not fall through to the no-backend state.
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

  return resolveDevIpcBridgeUrl() ? 'browser-desktop-bridge' : 'browser-only'
}

/**
 * Returns whether desktop command transport.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function hasDesktopCommandTransport() {
  return resolveAppRuntime() !== 'browser-only'
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
export function isMacOsHost() {
  if (typeof navigator === 'undefined') {
    return false
  }
  const platform = navigator.platform || ''
  const userAgent = navigator.userAgent || ''
  return /Mac|iPhone|iPod|iPad/.test(platform) || /Mac OS X/.test(userAgent)
}

/**
 * What stands where a system title bar would be. A desktop window never has
 * one (`back-forward-window-frame-tradeoff.md` D):
 * - `mac-overlay`: macOS. `titleBarStyle: "Overlay"` floats the traffic
 *   lights over the content, which runs up under the ~28px title strip.
 * - `drawn`: Windows and Linux. The system title bar is turned off
 *   (`desktop_integration/window.rs`) and the app draws the window buttons.
 * - `browser`: a page in a browser, which has its own frame.
 *
 * Every screen the window can show takes its title-bar duties from this one
 * answer: where the buttons go, which room they need, what drags the window.
 */
export type WindowFrame = 'mac-overlay' | 'drawn' | 'browser'

/**
 * `pathkeep-dev-desktop-window` set to `1` in the dev bridge's page makes it
 * stand in for the desktop window of the platform the browser reports, so an
 * E2E spec sees the frame people get (`tests/e2e/support/window-frame.ts`).
 * Only the dev bridge reads it; the desktop app and a plain browser do not.
 * The page still has no window to minimize or move.
 */
function standsInForDesktopWindow() {
  if (resolveAppRuntime() !== 'browser-desktop-bridge') return false
  try {
    return localStorage.getItem('pathkeep-dev-desktop-window') === '1'
  } catch {
    return false
  }
}

let frame: WindowFrame | undefined

/** This window's frame; it cannot change while the page lives. */
export function windowFrame(): WindowFrame {
  frame ??=
    hasTauriIpcBridge() || standsInForDesktopWindow()
      ? isMacOsHost()
        ? 'mac-overlay'
        : 'drawn'
      : 'browser'
  return frame
}

/**
 * Whether the traffic lights float over the top left, so the shell keeps
 * clear of them and of the title strip they sit in.
 */
export function hasMacOverlayTitlebar() {
  return windowFrame() === 'mac-overlay'
}

/** Whether the app draws the window's minimize, maximize and close buttons. */
export function drawsWindowControls() {
  return windowFrame() === 'drawn'
}
