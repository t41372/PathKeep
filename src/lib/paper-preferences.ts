/**
 * Persisted "paper" UI preferences shared between the Settings appearance card
 * and the global shell.
 *
 * Why this file exists:
 * - The redesign exposes four ambient toggles (theme, fonts, density, paper
 *   texture) that affect every route. They are persisted in localStorage and
 *   applied to <html> via attributes / CSS variables so future renders pick
 *   them up immediately.
 *
 * Persistence keys:
 * - pathkeep.theme         → 'light' | 'dark'
 * - pathkeep.fonts         → 'bundled' | 'system'
 * - pathkeep.density       → 'comfortable' | 'compact'
 * - pathkeep.paperTexture  → 'on' | 'off'
 *
 * The shell already owns the theme attribute; this module is the single
 * place that knows how to read / write the full bundle.
 */

export type PaperTheme = 'light' | 'dark'
export type PaperFontPreference = 'bundled' | 'system'
export type PaperDensity = 'comfortable' | 'compact'

export interface PaperPreferences {
  theme: PaperTheme
  fonts: PaperFontPreference
  density: PaperDensity
  paperTexture: boolean
}

const DEFAULTS: PaperPreferences = {
  theme: 'light',
  fonts: 'bundled',
  density: 'comfortable',
  paperTexture: true,
}

const STORAGE_KEYS = {
  theme: 'pathkeep.theme',
  fonts: 'pathkeep.fonts',
  density: 'pathkeep.density',
  paperTexture: 'pathkeep.paperTexture',
} as const

export function readPaperPreferences(): PaperPreferences {
  if (typeof window === 'undefined') return DEFAULTS
  try {
    return {
      theme:
        window.localStorage.getItem(STORAGE_KEYS.theme) === 'dark'
          ? 'dark'
          : 'light',
      fonts:
        window.localStorage.getItem(STORAGE_KEYS.fonts) === 'system'
          ? 'system'
          : 'bundled',
      density:
        window.localStorage.getItem(STORAGE_KEYS.density) === 'compact'
          ? 'compact'
          : 'comfortable',
      paperTexture:
        window.localStorage.getItem(STORAGE_KEYS.paperTexture) !== 'off',
    }
  } catch {
    return DEFAULTS
  }
}

export function persistPaperPreferences(prefs: PaperPreferences): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(STORAGE_KEYS.theme, prefs.theme)
    window.localStorage.setItem(STORAGE_KEYS.fonts, prefs.fonts)
    window.localStorage.setItem(STORAGE_KEYS.density, prefs.density)
    window.localStorage.setItem(
      STORAGE_KEYS.paperTexture,
      prefs.paperTexture ? 'on' : 'off',
    )
  } catch {
    // localStorage may be unavailable; the shell will still respect the
    // currently-applied document attributes for the lifetime of this session.
  }
}

/**
 * Custom event dispatched on `window` whenever the paper preferences change.
 * The shell + Settings appearance card both listen so neither cache drifts
 * away from the persisted state — the previous design left them each owning
 * a private `useState` copy and a Settings change never propagated back to
 * the shell's theme button.
 */
export const PAPER_PREFERENCES_EVENT = 'pathkeep.paperPreferencesChanged'

export interface PaperPreferencesEventDetail {
  preferences: PaperPreferences
}

/**
 * Idempotent helper: read prefs (or accept a candidate), apply them to <html>,
 * persist, and return the resolved bundle. Components and the shell both call
 * this so the document state and the persisted state never diverge.
 *
 * After applying, dispatches `PAPER_PREFERENCES_EVENT` on the window so other
 * subscribers (shell, Settings) can rehydrate their local mirrors from the
 * single source of truth.
 */
export function applyPaperPreferences(
  candidate: PaperPreferences | null,
): PaperPreferences {
  const resolved = candidate ?? readPaperPreferences()
  if (typeof document !== 'undefined') {
    const html = document.documentElement
    html.setAttribute('data-theme', resolved.theme)
    html.setAttribute('data-fonts', resolved.fonts)
    html.setAttribute('data-density', resolved.density)
    if (resolved.paperTexture) {
      html.style.removeProperty('--noise-opacity')
      html.style.removeProperty('--vignette-opacity')
    } else {
      html.style.setProperty('--noise-opacity', '0')
      html.style.setProperty('--vignette-opacity', '0')
    }
  }
  persistPaperPreferences(resolved)
  if (typeof window !== 'undefined') {
    window.dispatchEvent(
      new CustomEvent<PaperPreferencesEventDetail>(PAPER_PREFERENCES_EVENT, {
        detail: { preferences: resolved },
      }),
    )
  }
  return resolved
}
