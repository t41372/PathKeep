/**
 * Light / dark / system theme. Stored in localStorage and applied as the
 * `.dark` class on <html>, which is what shadcn and EvilCharts key off.
 */
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'

export type ThemePreference = 'light' | 'dark' | 'system'
export type ResolvedTheme = 'light' | 'dark'

const STORAGE_KEY = 'pathkeep-theme'
const darkQuery = '(prefers-color-scheme: dark)'

export function readStoredTheme(): ThemePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    if (value === 'light' || value === 'dark' || value === 'system') return value
  } catch {
    // Fall through to the default.
  }
  return 'system'
}

function systemTheme(): ResolvedTheme {
  return window.matchMedia?.(darkQuery).matches ? 'dark' : 'light'
}

/** Applies the stored theme before React renders, so there is no flash. */
export function applyStoredTheme() {
  const preference = readStoredTheme()
  const resolved = preference === 'system' ? systemTheme() : preference
  document.documentElement.classList.toggle('dark', resolved === 'dark')
  document.documentElement.style.colorScheme = resolved
}

interface ThemeValue {
  preference: ThemePreference
  resolved: ResolvedTheme
  setPreference: (next: ThemePreference) => void
}

const ThemeContext = createContext<ThemeValue | null>(null)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState(readStoredTheme)
  const [system, setSystem] = useState(systemTheme)

  useEffect(() => {
    const media = window.matchMedia?.(darkQuery)
    if (!media) return
    const onChange = () => setSystem(media.matches ? 'dark' : 'light')
    media.addEventListener('change', onChange)
    return () => media.removeEventListener('change', onChange)
  }, [])

  const resolved = preference === 'system' ? system : preference

  useEffect(() => {
    document.documentElement.classList.toggle('dark', resolved === 'dark')
    document.documentElement.style.colorScheme = resolved
  }, [resolved])

  const value = useMemo<ThemeValue>(
    () => ({
      preference,
      resolved,
      setPreference: (next) => {
        try {
          localStorage.setItem(STORAGE_KEY, next)
        } catch {
          // Keep the in-memory choice.
        }
        setPreference(next)
      },
    }),
    [preference, resolved],
  )

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}

export function useTheme() {
  const value = useContext(ThemeContext)
  if (!value) throw new Error('useTheme must be used inside ThemeProvider')
  return value
}
