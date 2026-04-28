/**
 * @file profile-scope.test.tsx
 * @description Focused coverage for the shared profile-scope provider storage contract.
 * @module lib
 */

import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, test } from 'vitest'
import { ProfileScopeProvider } from './profile-scope'
import { useProfileScope } from './profile-scope-context'

const storageKey = 'pathkeep.profile-scope'

function wrapper({ children }: { children: ReactNode }) {
  return <ProfileScopeProvider>{children}</ProfileScopeProvider>
}

describe('ProfileScopeProvider', () => {
  afterEach(() => {
    window.localStorage.removeItem(storageKey)
  })

  test('hydrates, stores, and clears the shared profile scope', () => {
    window.localStorage.setItem(storageKey, 'chrome:Stored')

    const { result } = renderHook(() => useProfileScope(), { wrapper })

    expect(result.current.activeProfileId).toBe('chrome:Stored')

    act(() => {
      result.current.setActiveProfileId('safari:Personal')
    })
    expect(window.localStorage.getItem(storageKey)).toBe('safari:Personal')

    act(() => {
      result.current.setActiveProfileId(null)
    })
    expect(window.localStorage.getItem(storageKey)).toBeNull()
  })

  test('ignores blank stored profile scope values', () => {
    window.localStorage.setItem(storageKey, '   ')

    const { result } = renderHook(() => useProfileScope(), { wrapper })

    expect(result.current.activeProfileId).toBeNull()
  })
})
