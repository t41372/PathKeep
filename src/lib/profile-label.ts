/**
 * How a browser profile is named on screen: "Google Chrome · Work". A Google
 * Takeout import is a profile too, which the backend names in English
 * ("Imported browser history"); it reads as Google Takeout instead.
 */
import type { Translator } from '@/lib/i18n'

export interface NamedProfile {
  profileId: string
  browserName: string
  profileName: string
}

export function profileLabel(profile: NamedProfile, t: Translator) {
  if (profile.profileId.split(':')[0] === 'takeout')
    return t('common.takeoutProfile')
  return `${profile.browserName} · ${profile.profileName}`
}
