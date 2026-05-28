import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  applyPaperPreferences,
  persistPaperPreferences,
  readPaperPreferences,
  type PaperPreferences,
} from './paper-preferences'

const DEFAULTS: PaperPreferences = {
  theme: 'light',
  fonts: 'bundled',
  density: 'comfortable',
  paperTexture: true,
}

afterEach(() => {
  vi.unstubAllGlobals()
  window.localStorage.clear()
  document.documentElement.removeAttribute('data-theme')
  document.documentElement.removeAttribute('data-fonts')
  document.documentElement.removeAttribute('data-density')
  document.documentElement.style.removeProperty('--noise-opacity')
  document.documentElement.style.removeProperty('--vignette-opacity')
})

describe('readPaperPreferences', () => {
  test('returns defaults when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(readPaperPreferences()).toEqual(DEFAULTS)
    })
  })

  test('returns defaults when localStorage is empty', () => {
    expect(readPaperPreferences()).toEqual(DEFAULTS)
  })

  test('reads each persisted preference key', () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')
    window.localStorage.setItem('pathkeep.fonts', 'system')
    window.localStorage.setItem('pathkeep.density', 'compact')
    window.localStorage.setItem('pathkeep.paperTexture', 'off')
    expect(readPaperPreferences()).toEqual({
      theme: 'dark',
      fonts: 'system',
      density: 'compact',
      paperTexture: false,
    })
  })

  test('defaults unrecognized stored values back to the shipped appearance', () => {
    window.localStorage.setItem('pathkeep.theme', 'sepia')
    window.localStorage.setItem('pathkeep.fonts', 'remote')
    window.localStorage.setItem('pathkeep.density', 'dense')
    window.localStorage.setItem('pathkeep.paperTexture', 'maybe')

    expect(readPaperPreferences()).toEqual(DEFAULTS)
  })

  test('returns defaults when localStorage.getItem throws', () => {
    const original = window.localStorage.getItem.bind(window.localStorage)
    window.localStorage.getItem = vi.fn(() => {
      throw new Error('storage disabled')
    })
    try {
      expect(readPaperPreferences()).toEqual(DEFAULTS)
    } finally {
      window.localStorage.getItem = original
    }
  })
})

describe('persistPaperPreferences', () => {
  test('does nothing when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(() => persistPaperPreferences(DEFAULTS)).not.toThrow()
    })
  })

  test('writes each key into localStorage', () => {
    persistPaperPreferences({
      theme: 'dark',
      fonts: 'system',
      density: 'compact',
      paperTexture: false,
    })
    expect(window.localStorage.getItem('pathkeep.theme')).toBe('dark')
    expect(window.localStorage.getItem('pathkeep.fonts')).toBe('system')
    expect(window.localStorage.getItem('pathkeep.density')).toBe('compact')
    expect(window.localStorage.getItem('pathkeep.paperTexture')).toBe('off')
  })

  test('writes "on" when paperTexture is true', () => {
    persistPaperPreferences(DEFAULTS)
    expect(window.localStorage.getItem('pathkeep.paperTexture')).toBe('on')
  })

  test('swallows localStorage.setItem errors', () => {
    const original = window.localStorage.setItem.bind(window.localStorage)
    window.localStorage.setItem = vi.fn(() => {
      throw new Error('storage full')
    })
    try {
      expect(() => persistPaperPreferences(DEFAULTS)).not.toThrow()
    } finally {
      window.localStorage.setItem = original
    }
  })
})

describe('applyPaperPreferences', () => {
  test('returns the candidate without touching globals when window and document are unavailable', () => {
    const candidate: PaperPreferences = {
      theme: 'dark',
      fonts: 'system',
      density: 'compact',
      paperTexture: false,
    }

    withWindowAndDocumentUnavailable(() => {
      expect(applyPaperPreferences(candidate)).toEqual(candidate)
    })
  })

  test('reads stored prefs when no candidate is supplied', () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')
    const applied = applyPaperPreferences(null)
    expect(applied.theme).toBe('dark')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  test('writes data-* attributes and clears noise/vignette when paperTexture is on', () => {
    applyPaperPreferences({
      theme: 'light',
      fonts: 'bundled',
      density: 'comfortable',
      paperTexture: true,
    })
    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    expect(document.documentElement.getAttribute('data-fonts')).toBe('bundled')
    expect(document.documentElement.getAttribute('data-density')).toBe(
      'comfortable',
    )
    expect(
      document.documentElement.style.getPropertyValue('--noise-opacity'),
    ).toBe('')
  })

  test('sets noise / vignette to 0 when paperTexture is off', () => {
    applyPaperPreferences({
      theme: 'light',
      fonts: 'bundled',
      density: 'comfortable',
      paperTexture: false,
    })
    expect(
      document.documentElement.style.getPropertyValue('--noise-opacity'),
    ).toBe('0')
    expect(
      document.documentElement.style.getPropertyValue('--vignette-opacity'),
    ).toBe('0')
  })

  test('dispatches PAPER_PREFERENCES_EVENT with the resolved prefs', () => {
    const events: PaperPreferences[] = []
    const listener = (e: Event) => {
      const detail = (e as CustomEvent<{ preferences: PaperPreferences }>)
        .detail
      events.push(detail.preferences)
    }
    window.addEventListener('pathkeep.paperPreferencesChanged', listener)
    try {
      const candidate: PaperPreferences = {
        theme: 'dark',
        fonts: 'system',
        density: 'compact',
        paperTexture: false,
      }
      applyPaperPreferences(candidate)
      expect(events).toHaveLength(1)
      expect(events[0]).toEqual(candidate)
    } finally {
      window.removeEventListener('pathkeep.paperPreferencesChanged', listener)
    }
  })

  test('still applies document attributes and dispatches when persistence fails', () => {
    const original = window.localStorage.setItem.bind(window.localStorage)
    window.localStorage.setItem = vi.fn(() => {
      throw new Error('storage full')
    })
    const events: PaperPreferences[] = []
    const listener = (e: Event) => {
      events.push(
        (e as CustomEvent<{ preferences: PaperPreferences }>).detail
          .preferences,
      )
    }
    window.addEventListener('pathkeep.paperPreferencesChanged', listener)
    try {
      const candidate: PaperPreferences = {
        theme: 'dark',
        fonts: 'system',
        density: 'compact',
        paperTexture: false,
      }
      expect(applyPaperPreferences(candidate)).toEqual(candidate)
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
      expect(document.documentElement.getAttribute('data-fonts')).toBe('system')
      expect(document.documentElement.getAttribute('data-density')).toBe(
        'compact',
      )
      expect(events).toEqual([candidate])
    } finally {
      window.removeEventListener('pathkeep.paperPreferencesChanged', listener)
      window.localStorage.setItem = original
    }
  })

  test('persists and returns the resolved bundle', () => {
    const candidate: PaperPreferences = {
      theme: 'dark',
      fonts: 'system',
      density: 'compact',
      paperTexture: true,
    }
    const result = applyPaperPreferences(candidate)
    expect(result).toEqual(candidate)
    expect(window.localStorage.getItem('pathkeep.theme')).toBe('dark')
    expect(window.localStorage.getItem('pathkeep.fonts')).toBe('system')
    expect(window.localStorage.getItem('pathkeep.density')).toBe('compact')
    expect(window.localStorage.getItem('pathkeep.paperTexture')).toBe('on')
  })
})

function withWindowUnavailable(assertion: () => void) {
  const originalWindow = globalThis.window
  vi.stubGlobal('window', undefined)
  try {
    assertion()
  } finally {
    vi.stubGlobal('window', originalWindow)
  }
}

function withWindowAndDocumentUnavailable(assertion: () => void) {
  const originalWindow = globalThis.window
  const originalDocument = globalThis.document
  vi.stubGlobal('window', undefined)
  vi.stubGlobal('document', undefined)
  try {
    assertion()
  } finally {
    vi.stubGlobal('document', originalDocument)
    vi.stubGlobal('window', originalWindow)
  }
}
