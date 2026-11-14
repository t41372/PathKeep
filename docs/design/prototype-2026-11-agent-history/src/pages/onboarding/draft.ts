/**
 * The choices the prototype's onboarding collects, and when each step may
 * continue. Mirrors src/features/onboarding/draft.ts; the browser choice is
 * replaced by the types and sources that onboarding.tsx holds.
 */
import type { ProviderDraft } from '../settings/ai/providers'
import { draftIsComplete, emptyDraft } from '../settings/ai/providers'

export type Frequency = 'hourly' | 'sixHours' | 'daily' | 'off'
export type AiChoice = 'off' | 'local' | 'provider'

export interface Draft {
  encrypt: boolean
  password: string
  confirm: string
  keychain: boolean
  frequency: Frequency
  ai: AiChoice
  provider: ProviderDraft
}

export function initialDraft(keychainAvailable: boolean): Draft {
  return {
    encrypt: true,
    password: '',
    confirm: '',
    keychain: keychainAvailable,
    frequency: 'hourly',
    ai: 'off',
    provider: emptyDraft(),
  }
}

/** 0 = empty, 1 = too short, 2 = okay, 3 = good, 4 = strong. */
export function passwordScore(password: string) {
  if (password.length === 0) return 0
  if (password.length < 10) return 1
  const mixed = /[0-9]/.test(password) && /[A-Z]/.test(password)
  if (!mixed) return 2
  return password.length >= 14 ? 4 : 3
}

export function draftStepReady(step: string, draft: Draft) {
  if (step === 'encryption') return !draft.encrypt || (passwordScore(draft.password) >= 2 && draft.password === draft.confirm)
  if (step === 'ai') return draft.ai !== 'provider' || draftIsComplete(draft.provider)
  return true
}

/** Background backups can only open an encrypted archive through the keychain. */
export function backgroundCanUnlock(draft: Draft, keychainAvailable: boolean) {
  return !draft.encrypt || (draft.keychain && keychainAvailable)
}
