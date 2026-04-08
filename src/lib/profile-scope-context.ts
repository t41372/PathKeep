import { createContext, useContext } from 'react'

export interface ProfileScopeValue {
  activeProfileId: string | null
  setActiveProfileId: (nextProfileId: string | null) => void
}

export const ProfileScopeContext = createContext<ProfileScopeValue | null>(null)

export function useProfileScope() {
  const value = useContext(ProfileScopeContext)

  if (!value) {
    throw new Error('useProfileScope must be used inside ProfileScopeProvider')
  }

  return value
}

export function useScopedProfileId(explicitProfileId: string | null) {
  const { activeProfileId } = useProfileScope()

  return explicitProfileId ?? activeProfileId
}

export function profileIdLabel(profileId: string) {
  return profileId.split(':').slice(1).join(':') || profileId
}

export function profileIdBrowserKind(profileId: string) {
  return profileId.split(':')[0] ?? profileId
}
