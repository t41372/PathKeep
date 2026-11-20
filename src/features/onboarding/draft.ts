/**
 * The choices the wizard collects before anything is created, and the rules
 * for when each step may continue.
 *
 * Responsible for: the step order, the draft shape and its starting values
 * (which kinds of data start ticked, from what this computer has, or what
 * an unfinished setup saved), what is saved for a setup to resume with,
 * password strength, and per-step validation.
 * Not responsible for: saving anything or calling the backend (see
 * `use-finish-setup.ts`), walking the Agent tool folders
 * (`use-agent-sources.ts`), or rendering.
 */
import type { ProviderDraft } from '@/features/ai-setup/providers'
import { emptyDraft, draftIsComplete } from '@/features/ai-setup/providers'
import {
  frequencyForHours,
  type Frequency,
} from '@/features/backup/schedule-actions'
import {
  AGENT_TOOLS,
  type AgentToolId,
  type DataKindId,
  type DetectedSources,
  type OnboardingChoices,
} from '@/lib/backend-client/data-kinds'
import {
  browserDiscoveryState,
  isBrowserProfileReadable,
} from '@/lib/platform-guidance'
import type { AppSnapshot, BrowserProfile } from '@/lib/types'

export const steps = [
  'welcome',
  'kinds',
  'sources',
  'storage',
  'encryption',
  'schedule',
  'ai',
  'done',
] as const

export type StepId = (typeof steps)[number]

export type AiChoice = 'off' | 'local' | 'provider'

/** The kinds of data onboarding offers, in the order they are shown. */
export const KINDS: readonly DataKindId[] = ['browsing', 'agent-history']

export interface Draft {
  /**
   * The kinds to keep; `null` until the person ticks or unticks one, while
   * the ones found on this computer are the choice (`chosenKinds`).
   */
  kinds: DataKindId[] | null
  selectedProfileIds: string[]
  /**
   * The Agent tools to back up; `null` until the person ticks or unticks
   * one, while every tool whose folder can be read is the choice.
   */
  agentTools: AgentToolId[] | null
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
 * The kinds and Agent tools an unfinished setup saved, in the order they
 * are shown. Each is `null` when nothing was saved for it: no tools are
 * saved before the sources step, and that step can't be left with none.
 */
function savedChoices(snapshot: AppSnapshot) {
  const saved = snapshot.config.onboardingChoices
  const kinds = KINDS.filter((kind) => saved?.kinds.includes(kind))
  const tools = AGENT_TOOLS.filter((tool) => saved?.agentTools.includes(tool))
  return {
    kinds: kinds.length > 0 ? kinds : null,
    agentTools: tools.length > 0 ? tools : null,
  }
}

/**
 * What to keep for a setup to resume with (`save_onboarding_choices`):
 * creating the archive applies the last one saved, so it is saved again
 * with the final choices just before. A backup reads each project's `.git`
 * from the start (the sources step says so; the backup page turns it off).
 */
export function choicesToSave(
  kinds: DataKindId[],
  agentTools: AgentToolId[],
): OnboardingChoices {
  return { kinds, agentTools, readGit: true }
}

/**
 * Starts from whatever an earlier, unfinished setup saved; otherwise every
 * readable browser is picked and the prototype's defaults apply.
 */
export function initialDraft(snapshot: AppSnapshot): Draft {
  const { config } = snapshot
  const known = new Set(snapshot.browserProfiles.map((p) => p.profileId))
  const saved = config.selectedProfileIds.filter((id) => known.has(id))
  const choices = savedChoices(snapshot)
  return {
    kinds: choices.kinds,
    selectedProfileIds:
      saved.length > 0
        ? saved
        : snapshot.browserProfiles
            .filter(isBrowserProfileReadable)
            .map((profile) => profile.profileId),
    agentTools: choices.agentTools,
    encrypt: true,
    password: '',
    confirm: '',
    keychain: snapshot.keyringStatus.available,
    frequency: frequencyForHours(config.dueAfterHours) ?? 'hourly',
    ai: 'off',
    provider: emptyDraft(),
  }
}

/**
 * Whether browsing history has a source here: a history file, or Safari
 * that macOS hides (it counts as found; the sources step explains it).
 */
export function browsingFound(snapshot: AppSnapshot) {
  const listed = listedProfiles(snapshot.browserProfiles).length
  return (
    listed > 0 ||
    browserDiscoveryState(snapshot.browserDiscoveryIssue, listed) ===
      'full-disk-access'
  )
}

/** Whether any Agent tool has a folder here. Nothing inside was opened. */
export function agentFound(detected: DetectedSources | undefined) {
  return (
    detected?.sources.some(
      (source) => source.kind === 'agent-history' && source.found,
    ) ?? false
  )
}

/**
 * The kinds that start ticked: those with a source on this computer, or
 * browsing when neither has one (its sources step then says why).
 */
export function defaultKinds(snapshot: AppSnapshot, agent: boolean) {
  const found = KINDS.filter((kind) =>
    kind === 'browsing' ? browsingFound(snapshot) : agent,
  )
  return found.length > 0 ? found : (['browsing'] as DataKindId[])
}

/**
 * The kinds chosen now: the person's ticks, else the defaults once the
 * check for Agent tools has answered; `null` while it has not.
 */
export function chosenKinds(
  draft: Draft,
  snapshot: AppSnapshot,
  detection: { settled: boolean; detected: DetectedSources | undefined },
): DataKindId[] | null {
  if (draft.kinds) return draft.kinds
  if (!detection.settled) return null
  return defaultKinds(snapshot, agentFound(detection.detected))
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

/** What the steps that depend on more than the draft need to know. */
export interface StepContext {
  snapshot: AppSnapshot
  /** The chosen kinds; `null` while the check for Agent tools runs. */
  kinds: DataKindId[] | null
  /** The chosen Agent tools PathKeep can read; `null` until the walk ends. */
  agentReadable: AgentToolId[] | null
}

/** The chosen kinds without a ticked source PathKeep can read. */
export function kindsLackingSource(
  draft: Draft,
  context: StepContext,
): DataKindId[] {
  return (context.kinds ?? []).filter((kind) =>
    kind === 'browsing'
      ? readableSelection(draft, context.snapshot).length === 0
      : context.agentReadable !== null && context.agentReadable.length === 0,
  )
}

export function canContinue(step: StepId, draft: Draft, context: StepContext) {
  switch (step) {
    case 'kinds':
      return context.kinds !== null && context.kinds.length > 0
    case 'sources':
      return (
        context.kinds !== null &&
        (!context.kinds.includes('agent-history') ||
          context.agentReadable !== null) &&
        kindsLackingSource(draft, context).length === 0
      )
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
