import { useEffect, useMemo, useState, type ReactNode } from 'react'
import {
  ProfileScopeContext,
  type ProfileScopeValue,
} from './profile-scope-context'
const profileScopeStorageKey = 'pathkeep.profile-scope'

function loadStoredProfileScope() {
  if (typeof window === 'undefined') return null
  const value = window.localStorage.getItem(profileScopeStorageKey)
  return value?.trim() ? value : null
}

export function ProfileScopeProvider({ children }: { children: ReactNode }) {
  const [activeProfileId, setActiveProfileId] = useState<string | null>(
    loadStoredProfileScope,
  )

  useEffect(() => {
    if (typeof window === 'undefined') return

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
