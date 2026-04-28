/**
 * This module defines the React context and small helpers behind shared profile scope.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `ProfileScopeValue`
 * - `ProfileScopeContext`
 * - `useProfileScope`
 * - `profileIdLabel`
 * - `profileIdBrowserKind`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { createContext, useContext } from 'react'

/**
 * Defines the typed shape for profile scope value.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export interface ProfileScopeValue {
  activeProfileId: string | null
  setActiveProfileId: (nextProfileId: string | null) => void
}

/**
 * Holds the React context used to share profile scope across the shell.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export const ProfileScopeContext = createContext<ProfileScopeValue | null>(null)

/**
 * Provides the `useProfileScope` hook.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function useProfileScope() {
  const value = useContext(ProfileScopeContext)

  if (!value) {
    throw new Error('useProfileScope must be used inside ProfileScopeProvider')
  }

  return value
}

/**
 * Explains how profile id label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function profileIdLabel(profileId: string) {
  return profileId.split(':').slice(1).join(':') || profileId
}

/**
 * Explains how profile id browser kind works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function profileIdBrowserKind(profileId: string) {
  return profileId.split(':')[0]
}
