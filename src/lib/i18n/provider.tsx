/**
 * Language and clock state for the whole app. The preferences live in
 * localStorage so the first paint is already right; the shell mirrors the
 * language into the backend config (`preferredLanguage`) for the scheduler
 * and worker. The OS's clock setting is read once from the backend
 * (`system_clock`), which the webview's `Intl` cannot see, and remembered for
 * the next first paint.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { supportClient } from '@/lib/backend-client/support'
import type { LanguagePreference } from '@/lib/types'
import { I18nContext, type ClockPreference } from './context'
import type { HourCycle } from './format'
import { createTranslator, localeTag, resolveLanguage } from './runtime'

const STORAGE_KEY = 'pathkeep-language-preference'
const CLOCK_KEY = 'pathkeep-clock-preference'
const SYSTEM_CLOCK_KEY = 'pathkeep-system-clock'
const preferences: LanguagePreference[] = ['system', 'en', 'zh-CN', 'zh-TW']
const clocks: ClockPreference[] = ['system', 'h12', 'h23']

function read(key: string) {
  try {
    return localStorage.getItem(key)
  } catch {
    // Storage can be unavailable in hardened WebViews.
    return null
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value)
  } catch {
    // Keep the in-memory choice even if it cannot be persisted.
  }
}

function asHourCycle(value: string | null): HourCycle {
  return value === 'h12' || value === 'h23' ? value : undefined
}

export function readStoredLanguagePreference(): LanguagePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    if (preferences.includes(value as LanguagePreference)) {
      return value as LanguagePreference
    }
  } catch {
    // Storage can be unavailable in hardened WebViews.
  }
  return 'system'
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState(readStoredLanguagePreference)
  const language = resolveLanguage(preference)
  const [clock, setClock] = useState<ClockPreference>(() => {
    const stored = read(CLOCK_KEY)
    return clocks.includes(stored as ClockPreference)
      ? (stored as ClockPreference)
      : 'system'
  })
  const [systemClock, setSystemClock] = useState(() =>
    asHourCycle(read(SYSTEM_CLOCK_KEY)),
  )

  useEffect(() => {
    document.documentElement.lang = localeTag(language)
  }, [language])

  useEffect(() => {
    let live = true
    supportClient
      .systemClock()
      .then((value) => {
        if (!live) return
        setSystemClock(asHourCycle(value))
        write(SYSTEM_CLOCK_KEY, value ?? '')
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [])

  const value = useMemo(
    () => ({
      language,
      locale: localeTag(language),
      preference,
      t: createTranslator(language),
      setPreference: (next: LanguagePreference) => {
        write(STORAGE_KEY, next)
        setPreference(next)
      },
      clock,
      hourCycle: clock === 'system' ? systemClock : clock,
      setClock: (next: ClockPreference) => {
        write(CLOCK_KEY, next)
        setClock(next)
      },
    }),
    [language, preference, clock, systemClock],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
