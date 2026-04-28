/**
 * This module provides the shared profile-scope state that the shell and scoped routes read from one place.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `ProfileScopeProvider`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { useEffect, useMemo, useState, type ReactNode } from 'react'
import {
  ProfileScopeContext,
  type ProfileScopeValue,
} from './profile-scope-context'
const profileScopeStorageKey = 'pathkeep.profile-scope'

/**
 * Loads stored profile scope.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function loadStoredProfileScope() {
  const value = window.localStorage.getItem(profileScopeStorageKey)
  return value?.trim() ? value : null
}

/**
 * Provides profile scope to descendant components.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function ProfileScopeProvider({ children }: { children: ReactNode }) {
  const [activeProfileId, setActiveProfileId] = useState<string | null>(
    loadStoredProfileScope,
  )

  useEffect(() => {
    if (activeProfileId) {
      window.localStorage.setItem(profileScopeStorageKey, activeProfileId)
      return
    }

    window.localStorage.removeItem(profileScopeStorageKey)
  }, [activeProfileId])

  const value = useMemo<ProfileScopeValue>(
    () => ({
      activeProfileId,
      setActiveProfileId,
    }),
    [activeProfileId],
  )

  return (
    <ProfileScopeContext.Provider value={value}>
      {children}
    </ProfileScopeContext.Provider>
  )
}
