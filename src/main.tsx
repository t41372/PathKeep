import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './app'
import { localeTag, resolveLanguage } from './lib/i18n'
import { readStoredPreference } from './lib/i18n/context'
import { installRuntimeDiagnostics } from './lib/runtime-diagnostics'

// Restore persisted theme preference before first paint
try {
  const saved = window.localStorage.getItem('pathkeep.theme')
  if (saved === 'light' || saved === 'dark') {
    document.documentElement.setAttribute('data-theme', saved)
  }
} catch {
  // localStorage may be unavailable
}

// Keep glyph fallback honest before first paint when a persisted locale exists.
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
