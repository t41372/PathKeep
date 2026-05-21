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
  window.localStorage.clear()
  document.documentElement.removeAttribute('data-theme')
  document.documentElement.removeAttribute('data-fonts')
  document.documentElement.removeAttribute('data-density')
  document.documentElement.style.removeProperty('--noise-opacity')
  document.documentElement.style.removeProperty('--vignette-opacity')
})

describe('readPaperPreferences', () => {
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
})
