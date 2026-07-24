/**
 * @file schedule-attempt-history.tsx
 * @description Renders the bounded, archive-independent scheduled-backup attempt ledger.
 *
 * ## Responsibilities
 * - Make every native scheduler wake visible, including skips, deferrals, failures, and interruptions.
 * - Keep diagnostic reason codes and canonical run links readable without exposing secrets.
 *
 * ## Not responsible for
 * - Loading or mutating scheduler state.
 * - Deciding whether an outcome changes the route-level health state.
 */

import type { ResolvedLanguage } from '../../lib/i18n'
import type { ScheduledBackupAttempt } from '../../lib/types'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

interface ScheduleAttemptHistoryProps {
  attempts: readonly ScheduledBackupAttempt[]
  language: ResolvedLanguage
  t: Translator
}

const OUTCOME_TONE: Record<ScheduledBackupAttempt['outcome'], string> = {
  deferred: 'text-warning',
  failed: 'text-error',
  interrupted: 'text-error',
  running: 'text-accent-text',
  skipped: 'text-ink-muted',
  success: 'text-success',
}

/**
 * Shows the newest native worker attempts without querying the canonical archive.
 */
export function ScheduleAttemptHistory({
  attempts,
  language,
  t,
}: ScheduleAttemptHistoryProps) {
  return (
    <section
      aria-labelledby="schedule-attempt-history-title"
      className="border-border-light bg-card rounded-paper border"
    >
      <div className="border-border-light flex items-center justify-between border-b px-4 py-3">
        <div>
          <h3
            className="text-ink font-sans text-[12px] font-semibold"
            id="schedule-attempt-history-title"
          >
            {t('schedule.attemptHistoryTitle')}
          </h3>
          <p className="text-ink-muted m-0 mt-1 font-sans text-[12px]">
            {t('schedule.attemptHistoryBody')}
          </p>
        </div>
        <span className="text-ink-muted font-mono text-[11px]">
          {t('schedule.attemptHistoryCount', { count: attempts.length })}
        </span>
      </div>
      {attempts.length === 0 ? (
        <p className="text-ink-muted m-0 px-4 py-4 font-sans text-[12px]">
          {t('schedule.attemptHistoryEmpty')}
        </p>
      ) : (
        <ol className="m-0 list-none p-0">
          {attempts.map((attempt) => (
            <li
              className="border-border-light grid gap-2 border-b px-4 py-3 last:border-b-0 md:grid-cols-[minmax(0,1fr)_auto]"
              key={attempt.id}
            >
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <strong
                    className={`font-sans text-[12px] ${OUTCOME_TONE[attempt.outcome]}`}
                  >
                    {t(`schedule.attemptOutcome.${attempt.outcome}`)}
                  </strong>
                  <span className="text-ink-muted font-mono text-[10px]">
                    {attempt.phase}
                  </span>
                  {attempt.reasonCode ? (
                    <span className="text-ink-muted font-mono text-[10px]">
                      {attempt.reasonCode}
                    </span>
                  ) : null}
                </div>
                {attempt.detail ? (
                  <p className="text-ink-secondary m-0 mt-1 break-words font-sans text-[12px]">
                    {attempt.detail}
                  </p>
                ) : null}
                <div className="text-ink-muted mt-1 flex flex-wrap gap-3 font-mono text-[10px]">
                  <span>{attempt.id}</span>
                  {attempt.runId == null ? null : (
                    <span>
                      {t('schedule.attemptRunId', { id: attempt.runId })}
                    </span>
                  )}
                </div>
              </div>
              <time
                className="text-ink-muted font-mono text-[10px]"
                dateTime={attempt.finishedAt ?? attempt.startedAt}
              >
                {formatAttemptTime(
                  attempt.finishedAt ?? attempt.startedAt,
                  language,
                )}
              </time>
            </li>
          ))}
        </ol>
      )}
    </section>
  )
}

function formatAttemptTime(value: string, language: ResolvedLanguage): string {
  const timestamp = new Date(value)
  if (Number.isNaN(timestamp.getTime())) return value
  return new Intl.DateTimeFormat(language, {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(timestamp)
}
