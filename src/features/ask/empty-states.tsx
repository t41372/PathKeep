/**
 * Ask screens that have no conversation yet: the starter and the setup
 * prompt. The starter says what the assistant reads, as it is now: the
 * browsing history unless that kind is off, and the Agent conversations
 * only while "Assistant can read Agent conversations" is on and the kind
 * can be read; and whether what it needs leaves this computer.
 */
import { CornerDownRight } from 'lucide-react'
import { Link } from 'react-router-dom'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useViewerPreferences } from '@/lib/queries/agent-viewer'
import { useKindState } from '@/lib/queries/data-kinds'

/** What the assistant can read now; `null` while that is being read. */
function useReads(): { reads: MessageKey; agentNote: boolean } | null {
  const browsing = useKindState('browsing')
  const agent = useAgentKind()
  const prefs = useViewerPreferences()
  if (browsing === undefined || agent.row === undefined) return null
  const browsingOn = browsing !== 'off'
  // Off, or a store that won't open: the assistant can't reach it either way.
  if (!agent.readable)
    return {
      reads: browsingOn ? 'ask.empty.readsBrowsing' : 'ask.empty.readsNothing',
      agentNote: false,
    }
  // The switch can't be read: say only what is certain.
  if (!prefs.data)
    return prefs.isError
      ? { reads: 'ask.empty.readsArchive', agentNote: false }
      : null
  const allowed = prefs.data.assistantMayRead
  if (allowed)
    return {
      reads: browsingOn ? 'ask.empty.readsBoth' : 'ask.empty.readsAgent',
      agentNote: false,
    }
  return {
    reads: browsingOn ? 'ask.empty.readsBrowsing' : 'ask.empty.readsNothing',
    agentNote: browsingOn,
  }
}

export function StarterEmpty({
  remoteProvider,
  onPick,
}: {
  /** Name of the online service in use, or null when the model is local. */
  remoteProvider: string | null
  onPick: (question: string) => void
}) {
  const { t } = useI18n()
  const reads = useReads()
  const examples = [
    t('ask.empty.example1'),
    t('ask.empty.example2'),
    t('ask.empty.example3'),
    t('ask.empty.example4'),
  ]
  return (
    <div className="mx-auto flex max-w-[600px] flex-col items-center gap-[18px] px-8 pt-[16vh] pb-8 text-center animate-rise">
      <BrandMark className="size-10" />
      <h1 className="text-[22px] font-semibold tracking-[-0.01em]">
        {t('ask.empty.title')}
      </h1>
      {reads ? (
        <p className="max-w-[440px] text-[13px] leading-[1.55] text-muted-foreground animate-in fade-in-0">
          {t(reads.reads)}{' '}
          {reads.agentNote && `${t('ask.empty.agentNotAllowed')} `}
          {remoteProvider
            ? t('ask.empty.privacyRemote', { provider: remoteProvider })
            : t('ask.empty.privacyLocal')}
        </p>
      ) : (
        <div
          aria-hidden
          className="flex w-[360px] max-w-full flex-col items-center gap-1.5"
        >
          <Skeleton className="h-3.5 w-full" />
          <Skeleton className="h-3.5 w-2/3" />
        </div>
      )}
      <div
        className="flex w-full flex-col gap-1.5"
        role="group"
        aria-label={t('ask.empty.examplesLabel')}
      >
        {examples.map((question) => (
          <button
            key={question}
            type="button"
            onClick={() => onPick(question)}
            className="flex items-center gap-2.5 rounded-[10px] border bg-card px-3.5 py-3 text-left transition-colors hover:bg-popover"
          >
            <CornerDownRight className="size-3.5 shrink-0 text-muted-foreground" />
            {question}
          </button>
        ))}
      </div>
    </div>
  )
}

/** Shown instead of the chat when no usable assistant provider is set up. */
export function AssistantSetup() {
  const { t } = useI18n()
  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="flex max-w-[440px] flex-col items-center gap-4 text-center animate-rise">
        <BrandMark className="size-10" />
        <h1 className="text-[22px] font-semibold tracking-[-0.01em]">
          {t('ask.setup.title')}
        </h1>
        <p className="text-[13px] leading-[1.6] text-muted-foreground">
          {t('ask.setup.body')}
        </p>
        <Button asChild>
          <Link to="/settings/ai">{t('ask.setup.action')}</Link>
        </Button>
        <p className="text-xs text-muted-foreground">
          {t('ask.setup.optional')}
        </p>
      </div>
    </div>
  )
}
