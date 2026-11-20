/**
 * Step 2, "What to keep": one card per kind of data, each with why it is
 * worth keeping and the sources found for it on this computer. A kind with
 * no source here starts unticked and says so; the person can still tick it.
 *
 * Only looks at whether the folders and history files are there (the
 * snapshot's browser profiles, `detect_data_sources` for the Agent tools);
 * nothing inside them is read. Until that check answers the cards wait, so
 * the defaults never change under a click.
 * Not responsible for: the defaults themselves (`draft.ts`) or keeping the
 * choice (the page).
 */
import { Check } from 'lucide-react'
import { useId, useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { foundAgentTools } from '@/features/data-kinds/presence'
import { toolName } from '@/features/data-kinds/tools'
import type {
  DataKindId,
  DetectedSources,
} from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { spaceOfKind } from '@/lib/spaces'
import { browsingFound, KINDS, listedProfiles } from './draft'
import { InlineError } from './inline-error'

export interface Detection {
  pending: boolean
  detected: DetectedSources | undefined
  error: unknown
  retry: () => void
}

export function KindsStep({
  chosen,
  onToggle,
  detection,
}: {
  /** `null` while the check for Agent tools runs. */
  chosen: DataKindId[] | null
  onToggle: (kind: DataKindId) => void
  detection: Detection
}) {
  const { t } = useI18n()
  return (
    <>
      <div
        role="group"
        aria-label={t('onboarding.kinds.label')}
        aria-busy={chosen === null}
        className="grid grid-cols-2 gap-3"
      >
        {KINDS.map((kind) => (
          <KindCard
            key={kind}
            kind={kind}
            on={chosen?.includes(kind) ?? false}
            waiting={chosen === null}
            detection={detection}
            onToggle={() => onToggle(kind)}
          />
        ))}
      </div>
      {detection.error != null && (
        <InlineError
          title={t('onboarding.kinds.detectFailed')}
          detail={describeError(detection.error, 'detect_data_sources')}
          actions={
            <Button size="sm" variant="outline" onClick={detection.retry}>
              {t('onboarding.kinds.retry')}
            </Button>
          }
        />
      )}
      {chosen?.length === 0 && (
        <p className="text-[13px] text-destructive">
          {t('onboarding.kinds.needOne')}
        </p>
      )}
    </>
  )
}

/** The sources found here, by name; `null` while still looking. */
function useFoundNames(kind: DataKindId, detection: Detection) {
  const { t, locale } = useI18n()
  const snapshot = useSnapshot()
  return useMemo(() => {
    let names: string[]
    if (kind === 'browsing') {
      names = [
        ...new Set(
          listedProfiles(snapshot.browserProfiles).map((p) => p.browserName),
        ),
      ]
      // macOS hides every profile: Safari is there, and the next step says why.
      if (names.length === 0 && browsingFound(snapshot)) names = ['Safari']
    } else {
      if (detection.pending) return null
      names = detection.detected
        ? foundAgentTools(detection.detected).map((tool) => t(toolName(tool)))
        : []
    }
    return new Intl.ListFormat(locale, { type: 'conjunction' }).format(names)
  }, [detection.detected, detection.pending, kind, locale, snapshot, t])
}

function KindCard({
  kind,
  on,
  waiting,
  detection,
  onToggle,
}: {
  kind: DataKindId
  on: boolean
  waiting: boolean
  detection: Detection
  onToggle: () => void
}) {
  const { t } = useI18n()
  const id = useId()
  const space = spaceOfKind(kind)!
  const Icon = space.icon
  const found = useFoundNames(kind, detection)
  // The check failed: nobody knows, which is not the same as "not found".
  const unchecked =
    kind === 'agent-history' &&
    detection.error != null &&
    detection.detected === undefined
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={on}
      aria-labelledby={`${id}-name`}
      aria-describedby={`${id}-about`}
      disabled={waiting}
      onClick={onToggle}
      className={cn(
        'group flex flex-col gap-2 rounded-xl border-[1.5px] bg-card p-4 text-left shadow-card transition-[border-color,box-shadow] duration-150 outline-none hover:border-foreground/30 focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-default disabled:hover:border-border',
        on && 'border-foreground hover:border-foreground',
      )}
    >
      <span className="flex items-center gap-2.5">
        <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Icon className="size-4" strokeWidth={1.75} aria-hidden />
        </span>
        <span id={`${id}-name`} className="flex-1 font-medium">
          {t(space.name)}
        </span>
        <span
          aria-hidden
          className={cn(
            'flex size-[18px] shrink-0 items-center justify-center rounded-[5px] border-[1.5px] border-border transition-colors duration-150',
            on && 'border-foreground bg-foreground text-background',
          )}
        >
          <Check
            className={cn(
              'size-3 scale-0 transition-transform duration-150',
              on && 'scale-100',
            )}
            strokeWidth={3}
          />
        </span>
      </span>
      <span id={`${id}-about`} className="flex flex-col gap-1.5">
        <span className="text-xs leading-relaxed text-muted-foreground">
          {t(space.why)}
        </span>
        {found === null ? (
          <span className="flex h-4 items-center">
            <Skeleton className="h-3 w-40" />
            <span className="sr-only">{t('onboarding.kinds.looking')}</span>
          </span>
        ) : found ? (
          <span className="text-[11.5px] text-muted-foreground/90">
            {t('onboarding.kinds.sources', { list: found })}
          </span>
        ) : unchecked ? (
          <span className="text-[11.5px] leading-relaxed text-brand">
            {t('onboarding.kinds.unchecked')}
          </span>
        ) : (
          <span className="animate-rise text-[11.5px] leading-relaxed text-brand">
            {t('onboarding.kinds.notFound')}
          </span>
        )}
      </span>
    </button>
  )
}
