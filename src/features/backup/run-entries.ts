/**
 * What a run recorded besides its counts, as sentences in the user's language.
 *
 * Backups, imports and post-backup work record coded entries; this module
 * turns each into one sentence, sorted into warnings and notes by
 * `run-codes.ts`. A code it does
 * not know, or an entry from before codes existed, still shows: a translated
 * lead-in with the recorded English text under it, because that text is all
 * the evidence there is.
 *
 * Not responsible for loading the run (`run-sheet.tsx`) or drawing the
 * entries (`run-warnings.tsx`).
 */
import { useMemo } from 'react'
import { useSourceStats } from '@/features/home/queries'
import { useFormat, useI18n, type Translator } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AuditRunDetail, BackupWarning } from '@/lib/types'
import {
  entriesOf,
  isNote,
  isWarning,
  NOTES,
  PARSER_PREFIX,
  WARNINGS,
  type Rule,
  type RunAction,
} from './run-codes'

export interface RunEntry {
  text: string
  evidence?: string
  action?: RunAction
}

export interface RunEntries {
  warnings: RunEntry[]
  notes: RunEntry[]
}

function evidenceOf(warning: BackupWarning, rule: Rule) {
  if (rule.evidence === 'message') return warning.message
  if (rule.evidence === 'diagnostic') return warning.diagnostic ?? undefined
  return undefined
}

/** Sorts a run's entries into warnings and notes, each as one sentence. */
function describeRunEntries(
  entries: BackupWarning[],
  t: Translator,
  values: (warning: BackupWarning) => Record<string, string | number>,
): RunEntries {
  const described: RunEntries = { warnings: [], notes: [] }
  for (const warning of entries) {
    const params = values(warning)
    if (isNote(warning.code)) {
      const rule: Rule = NOTES[warning.code]
      described.notes.push({
        text: t(`backupRuns.notes.${warning.code}`, params),
        evidence: evidenceOf(warning, rule),
        action: rule.action,
      })
    } else if (isWarning(warning.code)) {
      const rule: Rule = WARNINGS[warning.code]
      described.warnings.push({
        text: t(`backupRuns.warnings.${warning.code}`, params),
        evidence: evidenceOf(warning, rule),
        action: rule.action,
      })
    } else if (warning.code.startsWith(PARSER_PREFIX)) {
      described.warnings.push({
        text: t('backupRuns.warnings.parser'),
        evidence: warning.diagnostic ?? warning.message,
      })
    } else {
      described.warnings.push({
        text: t('backupRuns.warnings.other'),
        evidence: warning.message,
      })
    }
  }
  return described
}

/** `{profile}` as the Sources card writes it, even for a profile now gone. */
function useProfileName() {
  const snapshot = useSnapshot()
  const stats = useSourceStats()
  return useMemo(() => {
    const names = new Map<string, string>()
    for (const stat of stats.data ?? [])
      names.set(stat.profileId, `${stat.browserName} ${stat.profileName}`)
    for (const profile of snapshot.browserProfiles)
      names.set(
        profile.profileId,
        `${profile.browserName} ${profile.profileName}`,
      )
    return (profileId: string | null | undefined) =>
      profileId ? (names.get(profileId) ?? profileId) : ''
  }, [stats.data, snapshot.browserProfiles])
}

/** The run's warnings and notes, ready to render. */
export function useRunEntries(detail: AuditRunDetail): RunEntries {
  const { t } = useI18n()
  const format = useFormat()
  const profileName = useProfileName()
  return useMemo(
    () =>
      describeRunEntries(entriesOf(detail), t, (warning) => ({
        profile: profileName(warning.profileId),
        count: warning.count ?? 0,
        succeeded: warning.succeeded ?? 0,
        bytes: format.bytes(warning.bytes ?? 0),
      })),
    [detail, t, format, profileName],
  )
}
