/**
 * The choices the wizard collects before anything is created, and the rules
 * for when each step may continue.
 *
 * Responsible for: the step order, the draft shape and its starting values,
 * password strength, and per-step validation.
 * Not responsible for: saving anything or calling the backend (see
 * `use-finish-setup.ts`), or rendering.
 */
import type { ProviderDraft } from '@/features/ai-setup/providers'
import { emptyDraft, draftIsComplete } from '@/features/ai-setup/providers'
import {
  frequencyForHours,
  type Frequency,
} from '@/features/backup/schedule-actions'
import { isBrowserProfileReadable } from '@/lib/platform-guidance'
import type { AppSnapshot, BrowserProfile } from '@/lib/types'

export const steps = [
  'welcome',
  'browsers',
  'storage',
  'encryption',
  'schedule',
  'ai',
  'done',
] as const

export type StepId = (typeof steps)[number]

export type AiChoice = 'off' | 'local' | 'provider'

export interface Draft {
  selectedProfileIds: string[]
  encrypt: boolean
  password: string
  confirm: string
  keychain: boolean
  frequency: Frequency
  ai: AiChoice
  provider: ProviderDraft
}

/**
 * Profiles worth showing: ones with a history file, plus Safari when macOS
 * hides its history (no Full Disk Access). A browser that is simply not
 * installed is left out.
 */
export function listedProfiles(profiles: BrowserProfile[]) {
  return profiles.filter(
    (profile) =>
      profile.historyExists ||
      (profile.browserFamily === 'safari' && Boolean(profile.historyPath)),
  )
}

/**
 * Starts from whatever an earlier, unfinished setup saved; otherwise every
 * readable browser is picked and the prototype's defaults apply.
 */
export function initialDraft(snapshot: AppSnapshot): Draft {
  const { config } = snapshot
  const known = new Set(snapshot.browserProfiles.map((p) => p.profileId))
  const saved = config.selectedProfileIds.filter((id) => known.has(id))
  return {
    selectedProfileIds:
      saved.length > 0
        ? saved
        : snapshot.browserProfiles
            .filter(isBrowserProfileReadable)
            .map((profile) => profile.profileId),
    encrypt: true,
    password: '',
    confirm: '',
    keychain: snapshot.keyringStatus.available,
    frequency: frequencyForHours(config.dueAfterHours) ?? 'hourly',
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

export function readableSelection(draft: Draft, snapshot: AppSnapshot) {
  const selected = new Set(draft.selectedProfileIds)
  return snapshot.browserProfiles.filter(
    (profile) =>
      selected.has(profile.profileId) && isBrowserProfileReadable(profile),
  )
}

export function canContinue(step: StepId, draft: Draft, snapshot: AppSnapshot) {
  switch (step) {
    case 'browsers':
      return readableSelection(draft, snapshot).length > 0
    case 'encryption':
      return (
        !draft.encrypt ||
        (passwordScore(draft.password) >= 2 && draft.password === draft.confirm)
      )
    case 'ai':
      return draft.ai !== 'provider' || draftIsComplete(draft.provider)
    default:
      return true
  }
}

/** Background backups can only open an encrypted archive through the keychain. */
export function backgroundCanUnlock(draft: Draft, snapshot: AppSnapshot) {
  return !draft.encrypt || (draft.keychain && snapshot.keyringStatus.available)
}
