/**
 * This module boots the front-end runtime and hands the first render over to the app shell.
 *
 * Why this file exists:
 * - It is the narrow desktop-contract entry point that should stay boring, predictable, and easy to audit.
 * - If startup behavior looks wrong, this file should answer when global CSS, runtime diagnostics, and the React root come online.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - The surrounding shell contract lives in `docs/design/screens-and-nav.md` and `docs/design/ux-principles.md`.
 * - This file should avoid product logic; route and workflow orchestration belong in `src/app/` and `src/pages/`.
 */

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './app'
import { localeTag, resolveLanguage } from './lib/i18n'
import { readStoredPreference } from './lib/i18n/context'
import { installRuntimeDiagnostics } from './lib/runtime-diagnostics'
import { resolveAppRuntime } from './lib/runtime'
import { applyPaperPreferences } from './lib/paper-preferences'

// Apply persisted paper preferences (theme, fonts, density, paper texture)
// before first paint so the user never sees a flash of the wrong palette.
applyPaperPreferences(null)

document.documentElement.setAttribute(
  'data-pathkeep-runtime',
  resolveAppRuntime(),
)

// Keep the DOM locale honest before first paint when a persisted locale exists.
try {
  document.documentElement.lang = localeTag(
    resolveLanguage(readStoredPreference()),
  )
} catch {
  // localStorage or navigator access may be unavailable
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

void installRuntimeDiagnostics()
