/**
 * What the "Build a semantic search index" switch has made, under it in
 * Settings › Agent conversations (`agent-history.md` §7): whether the index
 * is ready, behind, being built (with Pause, Resume and Cancel) or not
 * built, why it can't be (the backend's typed refusals, with the settings
 * page that fixes each), and what turning the switch off does: the build
 * stops and search stops using the index, which stays on this computer
 * until freed (`free-semantic-index.tsx`).
 *
 * Responsibilities: putting the backend's status into words and offering
 * the one action each state allows; a skeleton while it is read. The state
 * line is a polite live region; the passage count, which moves several
 * times a second, sits outside it.
 * Not responsible for: the switch (`agent-section.tsx`), reading or
 * following the status (`useAgentSemanticIndex`), or the tasks page's row
 * for the same build.
 */
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { useEffect, useRef, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  SemanticAction,
  SemanticGeneration,
  SemanticIndexStatus,
} from '@/lib/backend-client/agent-semantic'
import { aiRefusalCode } from '@/lib/backend-client/ai-refusals'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import {
  isActiveJob,
  refetchSemanticIndex,
  useAgentSemanticIndex,
  useSemanticIndexControl,
} from '@/lib/queries/agent-semantic'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '@/lib/types'
import {
  refusalPlacePath,
  refusalWords,
  type RefusalPlace,
} from '../../ai-setup/ai-refusals'
import { KeptSemanticIndex } from './free-semantic-index'

/** Where a refusal is fixed, by the button that goes there; this page itself needs none. */
const PLACE_LINK: Partial<Record<RefusalPlace, MessageKey>> = {
  ai: 'settingsAgent.ai.index.openAi',
  kinds: 'settingsAgent.ai.index.openKinds',
}

/** The built-in model by its name; a service by its name and the model it ran. */
function useModelName() {
  const { t } = useI18n()
  const providers = useSnapshot()?.config.ai.embeddingProviders ?? []
  return (serving: SemanticGeneration) => {
    if (serving.provider === BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID)
      return t('settingsAi.semantic.switch.inAppName')
    const service = providers.find((item) => item.id === serving.provider)
    return service ? `${service.name} · ${serving.model}` : serving.model
  }
}

interface View {
  state: string
  detail: string | null
  /** A second line: the last build's outcome while an older index serves. */
  note?: string | null
  actions: SemanticAction[]
  /** Other settings pages that decide it; offer the way there. */
  places?: RefusalPlace[]
}

/** A typed refusal from the backend, in the words every screen uses for it. */
function refusalView(
  t: ReturnType<typeof useI18n>['t'],
  code: NonNullable<ReturnType<typeof aiRefusalCode>>,
  actions: SemanticAction[] = [],
): View {
  const words = refusalWords(t, code, 'semantic')
  return {
    state: words.title,
    detail: words.body,
    actions,
    places: words.places,
  }
}

function useView(status: SemanticIndexStatus): View {
  const { t } = useI18n()
  const format = useFormat()
  const modelName = useModelName()
  const { serving, job } = status
  const when = serving ? format.relative(serving.builtAt) : ''
  // With the switch on, "not enabled" means AI or semantic search is off in
  // Settings › AI; the status doesn't say which.
  if (!status.enabled)
    return {
      state: t('settingsAgent.ai.index.aiOff'),
      detail: t('settingsAgent.ai.index.aiOffDetail'),
      actions: [],
      places: ['ai'],
    }
  const reason = aiRefusalCode(status.reason ?? null)
  if (reason) return refusalView(t, reason)
  if (job && isActiveJob(job)) {
    if (job.state === 'cancelling')
      return {
        state: t('settingsAgent.ai.index.stopping'),
        detail: t('settingsAgent.ai.index.stoppingDetail'),
        actions: [],
      }
    const paused = job.state === 'paused'
    return {
      state: t(
        paused
          ? 'settingsAgent.ai.index.paused'
          : 'settingsAgent.ai.index.building',
      ),
      detail: paused
        ? t('settingsAgent.ai.index.pausedDetail')
        : serving
          ? t('settingsAgent.ai.index.buildingUpdate', { when })
          : t('settingsAgent.ai.index.buildingFirst'),
      actions: paused ? ['resume', 'cancel'] : ['pause', 'cancel'],
    }
  }
  // A build that failed for a typed reason says which, and where to fix it.
  const failure =
    job?.state === 'failed' ? aiRefusalCode(job.error ?? null) : null
  const failureWords = failure ? refusalWords(t, failure, 'semantic') : null
  const outcome =
    job?.state === 'failed'
      ? failureWords
        ? `${failureWords.title}. ${failureWords.body}`
        : t('settingsAgent.ai.index.failedDetail')
      : job?.state === 'cancelled'
        ? t('settingsAgent.ai.index.cancelled')
        : null
  if (!serving)
    return job?.state === 'failed'
      ? {
          state: t('settingsAgent.ai.index.failed'),
          detail: outcome,
          actions: ['build'],
          places: failureWords?.places,
        }
      : {
          state: t('settingsAgent.ai.index.none'),
          detail: t('settingsAgent.ai.index.noneDetail'),
          note: outcome,
          actions: ['build'],
        }
  const built = t('settingsAgent.ai.index.readyDetail', {
    count: serving.passages,
    when,
    model: modelName(serving),
  })
  if (status.stale)
    return {
      state: t('settingsAgent.ai.index.stale'),
      detail: t('settingsAgent.ai.index.staleDetail', { when }),
      note:
        job?.state === 'failed'
          ? failureWords
            ? `${t('settingsAgent.ai.index.failed')}: ${failureWords.title}`
            : t('settingsAgent.ai.index.failed')
          : built,
      actions: ['build'],
      places: failureWords?.places,
    }
  return {
    state: t('settingsAgent.ai.index.ready'),
    detail: built,
    actions: [],
  }
}

/** The tasks page's words for the same build's buttons. */
const ACTION_WORDS: Record<
  SemanticAction,
  { label: MessageKey; aria?: MessageKey }
> = {
  build: { label: 'settingsAgent.ai.index.build' },
  pause: { label: 'tasks.actions.pause', aria: 'tasks.actions.pauseLabel' },
  resume: { label: 'tasks.actions.resume', aria: 'tasks.actions.resumeLabel' },
  cancel: { label: 'tasks.actions.cancel', aria: 'tasks.actions.cancelLabel' },
}

/** Buttons to the other settings pages that decide it. */
function PlaceLinks({ places }: { places: RefusalPlace[] | undefined }) {
  const { t } = useI18n()
  return (places ?? []).map((place) => {
    const label = PLACE_LINK[place]
    return label ? (
      <Button key={place} size="sm" variant="outline" asChild>
        <Link to={refusalPlacePath[place]}>{t(label)}</Link>
      </Button>
    ) : null
  })
}

function StateLines({ view }: { view: View }) {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('settingsAgent.ai.index.title')}
      className="flex flex-col gap-0.5"
    >
      <span className="font-medium">{view.state}</span>
      {view.detail && (
        <span className="text-muted-foreground">{view.detail}</span>
      )}
      {view.note && (
        <span className="text-xs text-muted-foreground">{view.note}</span>
      )}
    </div>
  )
}

function Status({ status }: { status: SemanticIndexStatus }) {
  const { t } = useI18n()
  const view = useView(status)
  const control = useSemanticIndexControl()
  const job = status.job && isActiveJob(status.job) ? status.job : null
  const send = (action: SemanticAction) =>
    control.mutate(
      { action, jobId: job?.jobId },
      {
        onError: (error) =>
          toast.error(t('settingsAgent.ai.index.controlFailed'), {
            description: describeError(error),
          }),
      },
    )
  const pressing = control.isPending ? control.variables?.action : undefined
  const name = t('agentTasks.names.semanticIndex')
  const links = (view.places ?? []).some((place) => PLACE_LINK[place])
  return (
    <>
      <StateLines view={view} />
      {job && (
        <div className="flex flex-col gap-1.5">
          <Progress
            aria-label={t('settingsAgent.ai.index.title')}
            value={null}
            className={cn(
              'h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand',
              job.state !== 'running' &&
                '[&>[data-slot=progress-indicator]]:animate-none',
            )}
          />
          <span className="text-xs text-muted-foreground tabular">
            {t('agentTasks.progress.passages', { count: job.passages })}
          </span>
        </div>
      )}
      {(view.actions.length > 0 || links) && (
        <div className="flex flex-wrap gap-2">
          {view.actions.map((action) => {
            const words = ACTION_WORDS[action]
            return (
              <Button
                key={action}
                size="sm"
                variant="outline"
                aria-label={words.aria ? t(words.aria, { name }) : undefined}
                aria-busy={pressing === action || undefined}
                disabled={control.isPending}
                onClick={() => send(action)}
              >
                {action === 'build' && status.serving
                  ? t('settingsAgent.ai.index.update')
                  : t(words.label)}
              </Button>
            )
          })}
          <PlaceLinks places={view.places} />
        </div>
      )}
    </>
  )
}

function Body({
  index,
  saving,
}: {
  index: ReturnType<typeof useAgentSemanticIndex>
  saving: boolean
}) {
  const { t } = useI18n()
  const refused = index.isError ? aiRefusalCode(index.error) : null
  let body: ReactNode
  if (index.data && !saving) body = <Status status={index.data} />
  else if (refused && !saving) {
    // The read itself was refused (Agent conversations off, say): what
    // stands in the way and where it's changed.
    const view = refusalView(t, refused)
    body = (
      <>
        <StateLines view={view} />
        {(view.places ?? []).some((place) => PLACE_LINK[place]) && (
          <div className="flex flex-wrap gap-2">
            <PlaceLinks places={view.places} />
          </div>
        )}
      </>
    )
  } else if (index.isError && !saving)
    body = (
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-destructive">
          {t('settingsAgent.ai.index.loadFailed')}
        </span>
        <Button size="sm" variant="ghost" onClick={() => void index.refetch()}>
          {t('settingsAgent.ai.index.retry')}
        </Button>
      </div>
    )
  else
    body = (
      <div className="flex flex-col gap-1.5">
        <span className="sr-only">{t('settingsAgent.ai.index.loading')}</span>
        <Skeleton aria-hidden className="h-4 w-24" />
        <Skeleton aria-hidden className="h-3.5 w-64" />
      </div>
    )
  return body
}

/** Under the switch; `on` is the switch's value, `saving` while it changes. */
export function SemanticIndexStatus({
  on,
  saving,
}: {
  on: boolean
  saving: boolean
}) {
  const { t } = useI18n()
  const still = !!useReducedMotion()
  // Read with the switch off too: the index stays until freed, and says its size.
  const index = useAgentSemanticIndex()
  // A read that crossed the switch's save may say it is still off: read
  // again once the save is through, and show nothing of it meanwhile.
  const wasSaving = useRef(saving)
  useEffect(() => {
    if (wasSaving.current && !saving) refetchSemanticIndex()
    wasSaving.current = saving
  }, [saving])
  const kept =
    index.data && !index.data.enabled && !saving ? index.data.keptBytes : null
  return (
    <div className="flex flex-col border-t pt-3 text-[13px]">
      <AnimatePresence initial={false}>
        {on && (
          // It opens and closes by its height, so the line under it slides
          // instead of jumping.
          <motion.div
            key="status"
            initial={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: 'easeOut' }}
            className="overflow-hidden"
          >
            <div className="flex flex-col gap-2.5 pb-3">
              <Body index={index} saving={saving} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      <KeptSemanticIndex keptBytes={kept} />
      <p className="text-xs leading-relaxed text-muted-foreground">
        {t(
          on
            ? 'settingsAgent.ai.index.whenOn'
            : 'settingsAgent.ai.index.whenOff',
        )}
      </p>
    </div>
  )
}
