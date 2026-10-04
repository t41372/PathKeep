/** How long a run took, as localized text. */
import type { Translator } from '@/lib/i18n'
import type { BackupRunOverview } from '@/lib/types'

export function runDuration(run: BackupRunOverview, t: Translator) {
  if (!run.finishedAt) return null
  const seconds =
    (Date.parse(run.finishedAt) - Date.parse(run.startedAt)) / 1000
  if (!Number.isFinite(seconds) || seconds < 0) return null
  if (seconds < 60)
    return t('backup.runs.seconds', { value: seconds.toFixed(1) })
  return t('backup.runs.minutes', {
    minutes: Math.floor(seconds / 60),
    seconds: Math.round(seconds % 60),
  })
}
