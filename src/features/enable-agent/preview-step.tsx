/**
 * The first step, "See what it reads": the table of tool folders as the
 * walk counts them, the note that nothing has been opened, what PathKeep
 * reads and never does, and three numbers (how much the archive grows, an
 * estimate; free space on that disk; encryption, the archive's own).
 *
 * Every state of the walk has its own face: counting (running count and a
 * way to stop), stopped, failed (with the reason and another try), no tool
 * folder on this computer (where it looked, and another check), and a
 * finished preview gone stale (run it again).
 */
import {
  CircleAlert,
  FolderSearch,
  Lock,
  RotateCcw,
  ShieldCheck,
  TriangleAlert,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import {
  dataKindsErrorCode,
  isStoreDamage,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { PREVIEW_CONNECTION_LOST } from '@/lib/ipc/agent-preview-stream'
import { lookedIn, shownRows, wire } from './preview-model'
import { PreviewTable } from './preview-table'
import type { PreviewState } from './use-agent-preview'

function failureReason(code: string | null): MessageKey {
  if (code === PREVIEW_CONNECTION_LOST) {
    return 'enableAgent.preview.reasons.connection'
  }
  if (isStoreDamage(code)) return 'enableAgent.preview.reasons.unavailable'
  switch (dataKindsErrorCode(code)) {
    case 'archive-busy':
      return 'enableAgent.preview.reasons.busy'
    case 'destination-unavailable':
      return 'enableAgent.preview.reasons.destination'
    case 'archive-replaced':
      return 'enableAgent.preview.reasons.replaced'
    case 'policy-changed':
      return 'enableAgent.preview.reasons.policyChanged'
    case 'root-changed':
      return 'enableAgent.preview.reasons.rootChanged'
    case 'archive-uninitialized':
      return 'enableAgent.preview.reasons.uninitialized'
    default:
      return 'enableAgent.preview.reasons.other'
  }
}

/** A message in place of the table, with the one thing to do next. */
function Notice({
  tone,
  title,
  children,
  action,
}: {
  tone: 'info' | 'error'
  title: ReactNode
  children?: ReactNode
  action?: ReactNode
}) {
  const Icon = tone === 'error' ? CircleAlert : TriangleAlert
  return (
    <div
      role={tone === 'error' ? 'alert' : 'status'}
      className="flex animate-rise flex-col gap-2 rounded-lg border bg-card p-4 text-[13px]"
    >
      <span className="flex items-start gap-2 font-medium">
        <Icon
          aria-hidden
          className={
            tone === 'error'
              ? 'mt-px size-4 shrink-0 text-destructive'
              : 'mt-px size-4 shrink-0 text-brand'
          }
        />
        {title}
      </span>
      {children && (
        <div className="flex flex-col gap-1.5 pl-6 text-muted-foreground">
          {children}
        </div>
      )}
      {action && <div className="pl-6">{action}</div>}
    </div>
  )
}

function Walk({
  state,
  onCancel,
  onRerun,
}: {
  state: PreviewState
  onCancel: () => void
  onRerun: () => void
}) {
  const { t } = useI18n()

  if (state.phase === 'cancelled') {
    return (
      <Notice
        tone="info"
        title={t('enableAgent.preview.cancelled')}
        action={
          <Button variant="outline" size="sm" onClick={onRerun}>
            <RotateCcw aria-hidden />
            {t('enableAgent.preview.runAgain')}
          </Button>
        }
      />
    )
  }

  if (state.phase === 'failed') {
    const reason = failureReason(state.code)
    // The details only where the plain words could not say what happened.
    const detail =
      reason === 'enableAgent.preview.reasons.other'
        ? state.error
          ? describeError(state.error)
          : state.code
        : null
    return (
      <Notice
        tone="error"
        title={t('enableAgent.preview.failed')}
        action={
          <Button variant="outline" size="sm" onClick={onRerun}>
            <RotateCcw aria-hidden />
            {t('common.retry')}
          </Button>
        }
      >
        <p>{t(reason)}</p>
        {detail && (
          <p className="font-mono text-xs [overflow-wrap:anywhere]">{detail}</p>
        )}
      </Notice>
    )
  }

  if (state.phase === 'walking') {
    return (
      <div className="flex flex-col gap-2.5">
        <PreviewTable rows={shownRows(state.roots)} counting />
        <div
          role="status"
          className="flex items-center gap-2 text-xs text-muted-foreground"
        >
          <Spinner aria-hidden className="size-3.5" />
          <span className="tabular">
            {state.checked === null
              ? t('enableAgent.preview.counting')
              : t('enableAgent.preview.checked', { count: state.checked })}
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="ml-auto h-7"
            onClick={onCancel}
          >
            {t('common.cancel')}
          </Button>
        </div>
      </div>
    )
  }

  const rows = shownRows(state.report.rows)
  if (rows.length === 0) {
    const places = lookedIn(state.report.rows)
    return (
      <Notice
        tone="info"
        title={t('enableAgent.preview.nothingFound.title')}
        action={
          <Button variant="outline" size="sm" onClick={onRerun}>
            <RotateCcw aria-hidden />
            {t('enableAgent.preview.nothingFound.checkAgain')}
          </Button>
        }
      >
        <p>{t('enableAgent.preview.nothingFound.body')}</p>
        {places.length > 0 && (
          <>
            <p>{t('enableAgent.preview.nothingFound.lookedIn')}</p>
            <ul className="flex max-h-32 flex-col gap-0.5 overflow-y-auto font-mono text-xs [overflow-wrap:anywhere]">
              {places.map((place) => (
                <li key={place}>{place}</li>
              ))}
            </ul>
          </>
        )}
      </Notice>
    )
  }

  return (
    <div className="flex flex-col gap-2.5">
      <PreviewTable rows={rows} counting={false} />
      {state.expired && (
        <Notice
          tone="info"
          title={t('enableAgent.preview.expired')}
          action={
            <Button variant="outline" size="sm" onClick={onRerun}>
              <RotateCcw aria-hidden />
              {t('enableAgent.preview.runAgain')}
            </Button>
          }
        />
      )}
    </div>
  )
}

function Numbers({ state }: { state: PreviewState }) {
  const { t } = useI18n()
  const format = useFormat()
  const report = state.phase === 'finished' ? state.report : null
  if (state.phase !== 'walking' && !report) return null
  if (report && shownRows(report.rows).length === 0) return null
  const value = (text: string | null) =>
    text === null ? (
      <Skeleton className="mt-1 h-4 w-20" />
    ) : (
      <dd className="font-medium tabular">{text}</dd>
    )
  return (
    <dl className="grid grid-cols-3 gap-3 rounded-lg bg-muted/60 px-3.5 py-3 text-[13px] max-[640px]:grid-cols-1">
      <div>
        <dt className="flex items-center gap-1.5 text-xs text-muted-foreground">
          {t('enableAgent.preview.grows')}
          <span className="rounded border px-1 text-[10px] leading-4">
            {t('enableAgent.preview.estimate')}
          </span>
        </dt>
        {value(
          report
            ? t('enableAgent.preview.growsValue', {
                size: format.bytes(wire(report.estimatedArchiveBytes)),
              })
            : null,
        )}
      </div>
      <div>
        <dt className="text-xs text-muted-foreground">
          {t('enableAgent.preview.free')}
        </dt>
        {value(report ? format.bytes(wire(report.availableBytes)) : null)}
      </div>
      <div>
        <dt className="flex items-center gap-1 text-xs text-muted-foreground">
          <Lock aria-hidden className="size-3" />
          {t('enableAgent.preview.encryption')}
        </dt>
        {value(
          report
            ? t(
                report.archiveMode === 'Encrypted'
                  ? 'enableAgent.preview.encrypted'
                  : 'enableAgent.preview.plaintext',
              )
            : null,
        )}
      </div>
    </dl>
  )
}

export function PreviewStep({
  state,
  onCancel,
  onRerun,
}: {
  state: PreviewState
  onCancel: () => void
  onRerun: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-4">
      <Walk state={state} onCancel={onCancel} onRerun={onRerun} />
      <p className="text-xs text-muted-foreground">
        {t('enableAgent.preview.nothingOpened')}
      </p>
      <div className="grid grid-cols-2 gap-3 text-[13px] max-[640px]:grid-cols-1">
        <div className="flex flex-col gap-2 rounded-lg border bg-card p-3.5">
          <h3 className="flex items-center gap-1.5 font-medium">
            <FolderSearch
              aria-hidden
              className="size-4 text-muted-foreground"
            />
            {t('enableAgent.preview.reads.title')}
          </h3>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-muted-foreground">
            <li>{t('enableAgent.preview.reads.transcripts')}</li>
            <li>{t('enableAgent.preview.reads.outputs')}</li>
            <li>{t('enableAgent.preview.reads.git')}</li>
          </ul>
        </div>
        <div className="flex flex-col gap-2 rounded-lg border bg-card p-3.5">
          <h3 className="flex items-center gap-1.5 font-medium">
            <ShieldCheck aria-hidden className="size-4 text-muted-foreground" />
            {t('enableAgent.preview.never.title')}
          </h3>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-muted-foreground">
            <li>{t('enableAgent.preview.never.write')}</li>
            <li>{t('enableAgent.preview.never.run')}</li>
            <li>{t('enableAgent.preview.never.send')}</li>
          </ul>
        </div>
      </div>
      <Numbers state={state} />
    </div>
  )
}
