/**
 * What the window is made of. Solid, the default, paints the soft gradient the
 * app has always had. Glass is the system's own translucent, blurred window
 * material, as in the design prototype: vibrancy on macOS, Mica on Windows 11.
 * Linux has no such material, so the choice is offered on macOS and Windows
 * only. A display preference, stored and applied before the first render like
 * the theme.
 *
 * Responsible for: the preference, the `data-window-material` attribute the
 * stylesheet keys off, and asking the window for its effect. Not responsible
 * for: the colors (index.css).
 */
import { useState } from 'react'
import { Effect, EffectState, getCurrentWindow } from '@tauri-apps/api/window'
import { hasTauriGuestApi, isMacOsHost } from './runtime'

export type WindowMaterial = 'solid' | 'glass'

const STORAGE_KEY = 'pathkeep-window-material'

function isWindowsHost() {
  return (
    typeof navigator !== 'undefined' && /Win/.test(navigator.platform || '')
  )
}

/** Whether this window can be glass: a desktop window on macOS or Windows. */
export function windowMaterialAvailable() {
  return hasTauriGuestApi() && (isMacOsHost() || isWindowsHost())
}

function readStoredMaterial(): WindowMaterial {
  try {
    if (localStorage.getItem(STORAGE_KEY) === 'glass') return 'glass'
  } catch {
    // Fall through to the default.
  }
  return 'solid'
}

function applyMaterial(material: WindowMaterial) {
  const effective = windowMaterialAvailable() ? material : 'solid'
  document.documentElement.dataset.windowMaterial = effective
  if (!windowMaterialAvailable()) return
  const window = getCurrentWindow()
  // A failure leaves the solid background showing, which is still readable.
  void (
    effective === 'glass'
      ? window.setEffects({
          effects: [Effect.UnderWindowBackground, Effect.Mica],
          state: EffectState.FollowsWindowActiveState,
        })
      : window.clearEffects()
  ).catch(() => undefined)
}

/** Applies the stored material before React renders, so there is no flash. */
export function applyStoredWindowMaterial() {
  applyMaterial(readStoredMaterial())
}

export function useWindowMaterial() {
  const [material, setMaterial] = useState(readStoredMaterial)
  const choose = (next: WindowMaterial) => {
    try {
      localStorage.setItem(STORAGE_KEY, next)
    } catch {
      // Keep the in-memory choice.
    }
    applyMaterial(next)
    setMaterial(next)
  }
  return [material, choose] as const
}
