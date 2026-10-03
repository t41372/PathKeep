/**
 * Front-end entry point. Kept deliberately small: apply theme and language
 * before the first paint, mount the app, then install runtime diagnostics.
 * Product logic belongs in `src/app/` and `src/features/`.
 */
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './app'
import { localeTag, readStoredLanguagePreference, resolveLanguage } from './lib/i18n'
import { installRuntimeDiagnostics } from './lib/runtime-diagnostics'
import { resolveAppRuntime } from './lib/runtime'
import { applyStoredTheme } from './lib/theme'

applyStoredTheme()
document.documentElement.setAttribute('data-pathkeep-runtime', resolveAppRuntime())
document.documentElement.lang = localeTag(resolveLanguage(readStoredLanguagePreference()))

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

void installRuntimeDiagnostics()
