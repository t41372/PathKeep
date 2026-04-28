/**
 * @file profile-scope-context.test.tsx
 * @description Focused coverage for the shared profile-scope context helpers.
 * @module lib/profile-scope-context
 */

import { renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, test, vi } from 'vitest'
import {
  ProfileScopeContext,
  profileIdBrowserKind,
  profileIdLabel,
  useProfileScope,
} from './profile-scope-context'

describe('profile-scope context helpers', () => {
  test('throws when the hook is used outside the provider', () => {
    expect(() => renderHook(() => useProfileScope())).toThrow(
      'useProfileScope must be used inside ProfileScopeProvider',
    )
  })

  test('returns provider state and normalizes profile-id display helpers', () => {
    const setActiveProfileId = vi.fn()
    const wrapper = ({ children }: { children: ReactNode }) => (
      <ProfileScopeContext.Provider
        value={{
          activeProfileId: 'chrome:Profile:Work',
          setActiveProfileId,
        }}
      >
        {children}
      </ProfileScopeContext.Provider>
    )

    const { result } = renderHook(() => useProfileScope(), { wrapper })

    expect(result.current.activeProfileId).toBe('chrome:Profile:Work')
    expect(result.current.setActiveProfileId).toBe(setActiveProfileId)
    expect(profileIdLabel('chrome:Profile:Work')).toBe('Profile:Work')
    expect(profileIdLabel('orphan')).toBe('orphan')
    expect(profileIdBrowserKind('chrome:Profile:Work')).toBe('chrome')
    expect(profileIdBrowserKind('')).toBe('')
  })
})
