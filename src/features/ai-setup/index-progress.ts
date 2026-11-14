/**
 * Responsibilities: give both index screens the same phase, unit and validated ratio.
 * Not responsible for inventing totals, repairing backend data or estimating across phases.
 */
import type { useFormat, useI18n } from '@/lib/i18n'
import type { IndexBuildProgress } from '@/lib/types'

export function presentIndexProgress(
  progress: IndexBuildProgress | null | undefined,
  t: ReturnType<typeof useI18n>['t'],
  format: ReturnType<typeof useFormat>,
) {
  const phase = progress?.phase ?? 'planning'
  const label = t(`settingsAi.semantic.phases.${phase}`)
  const valid =
    progress &&
    phase === 'visits' &&
    progress.total !== null &&
    Number.isSafeInteger(progress.done) &&
    Number.isSafeInteger(progress.total) &&
    progress.done >= 0 &&
    progress.total >= progress.done
  return {
    label,
    detail: valid
      ? t('settingsAi.semantic.indexProgress', {
          done: format.number(progress.done),
          total: format.number(progress.total!),
        })
      : undefined,
    percent:
      valid && progress.total! > 0
        ? Math.floor((progress.done / progress.total!) * 100)
        : null,
  }
}
