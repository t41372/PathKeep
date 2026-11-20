/**
 * Step 3, "Sources": the sources of every chosen kind of data, in one card
 * whose height follows the window, so the step itself never scrolls. With
 * two or more kinds, the left pane lists them (how many sources were found
 * and ticked, what needs the person, what else is read) and the right pane
 * shows the selected kind's sources; ↑ and ↓ move between kinds. With one
 * kind the card is that kind's list alone.
 *
 * Under the card, each chosen kind without a ticked source PathKeep can
 * read says so: Continue needs one for every kind.
 * Not responsible for: the panes' rows (`browser-sources.tsx`,
 * `agent-sources.tsx`), saving the choice (the page) or the rule itself
 * (`draft.ts`).
 */
import { useId, useState, type KeyboardEvent } from 'react'
import type { AgentToolId, DataKindId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { spaceOfKind } from '@/lib/spaces'
import { AgentSourcesPane } from './agent-sources'
import { BrowserSources } from './browser-sources'
import {
  agentCounts,
  attentionText,
  browsingCounts,
  summaryText,
  type KindCounts,
} from './sources-model'
import type { AgentSources } from './use-agent-sources'

export function SourcesStep({
  kinds,
  selectedProfileIds,
  onProfiles,
  agent,
  agentTools,
  onAgentTools,
  lacking,
}: {
  kinds: DataKindId[]
  selectedProfileIds: string[]
  onProfiles: (selected: string[]) => void
  agent: AgentSources
  agentTools: AgentToolId[]
  onAgentTools: (tools: AgentToolId[]) => void
  /** Chosen kinds without a ticked source PathKeep can read. */
  lacking: DataKindId[]
}) {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const id = useId()
  const [activeKind, setActiveKind] = useState<DataKindId>(kinds[0])
  const [filter, setFilter] = useState('')
  const active = kinds.includes(activeKind) ? activeKind : kinds[0]
  const several = kinds.length > 1
  const counts: Record<DataKindId, KindCounts> = {
    browsing: browsingCounts(snapshot.browserProfiles, selectedProfileIds),
    'agent-history': agentCounts(agent, agentTools),
  }
  const tabId = (kind: DataKindId) => `${id}-tab-${kind}`
  const headingId = `${id}-heading`
  const title = t(spaceOfKind(active)!.name)

  function select(kind: DataKindId, focus = false) {
    setActiveKind(kind)
    setFilter('')
    if (focus) document.getElementById(tabId(kind))?.focus()
  }

  function onKeyDown(event: KeyboardEvent) {
    const step =
      event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0
    if (step === 0) return
    event.preventDefault()
    const index = kinds.indexOf(active)
    select(kinds[(index + step + kinds.length) % kinds.length], true)
  }

  return (
    <>
      <div
        className={cn(
          'flex overflow-hidden rounded-xl border bg-card shadow-card',
          several
            ? 'h-[clamp(340px,calc(100vh-352px),600px)]'
            : 'max-h-[clamp(340px,calc(100vh-352px),600px)] min-h-[220px]',
        )}
      >
        {several && (
          <div
            role="tablist"
            aria-orientation="vertical"
            aria-label={t('onboarding.sources.kindsLabel')}
            onKeyDown={onKeyDown}
            className="flex w-[252px] shrink-0 flex-col gap-0.5 overflow-y-auto border-r bg-muted/20 p-2"
          >
            {kinds.map((kind) => (
              <KindTab
                key={kind}
                id={tabId(kind)}
                kind={kind}
                counts={counts[kind]}
                current={kind === active}
                panelId={`${id}-panel`}
                onSelect={() => select(kind)}
              />
            ))}
          </div>
        )}
        <section
          id={`${id}-panel`}
          role={several ? 'tabpanel' : 'group'}
          aria-labelledby={several ? tabId(active) : headingId}
          className="flex min-w-0 flex-1 flex-col"
        >
          {/* Mounts fresh per kind, so switching fades the new list in. */}
          <div
            key={active}
            className="flex min-h-0 flex-1 animate-rise flex-col motion-reduce:animate-none"
          >
            {active === 'browsing' ? (
              <BrowserSources
                headingId={headingId}
                title={title}
                counts={counts.browsing}
                selected={selectedProfileIds}
                onChange={onProfiles}
                filter={filter}
                onFilter={setFilter}
              />
            ) : (
              <AgentSourcesPane
                headingId={headingId}
                title={title}
                counts={counts['agent-history']}
                sources={agent}
                chosen={agentTools}
                onChange={onAgentTools}
                filter={filter}
                onFilter={setFilter}
              />
            )}
          </div>
        </section>
      </div>
      {lacking.map((kind) => (
        <p key={kind} className="animate-rise text-[13px] text-brand">
          {t('onboarding.sources.noReadable', {
            kind: t(spaceOfKind(kind)!.name),
          })}
        </p>
      ))}
    </>
  )
}

function KindTab({
  id,
  kind,
  counts,
  current,
  panelId,
  onSelect,
}: {
  id: string
  kind: DataKindId
  counts: KindCounts
  current: boolean
  panelId: string
  onSelect: () => void
}) {
  const { t } = useI18n()
  const space = spaceOfKind(kind)!
  const Icon = space.icon
  const attention = attentionText(t, counts)
  return (
    <button
      type="button"
      role="tab"
      id={id}
      aria-selected={current}
      aria-controls={panelId}
      tabIndex={current ? 0 : -1}
      onClick={onSelect}
      className={cn(
        'flex w-full items-start gap-3 rounded-lg px-3 py-2.5 text-left transition-colors duration-150 outline-none focus-visible:ring-[3px] focus-visible:ring-ring/40',
        current ? 'bg-muted' : 'hover:bg-muted/50',
      )}
    >
      <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-background shadow-card">
        <Icon className="size-4" strokeWidth={1.75} aria-hidden />
      </span>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate text-sm font-medium">{t(space.name)}</span>
        <span
          className={cn(
            'text-xs',
            !counts.counting && counts.readableChosen === 0
              ? 'text-brand'
              : 'text-muted-foreground',
          )}
        >
          {summaryText(t, counts)}
        </span>
        {attention && <span className="text-xs text-brand">{attention}</span>}
        {kind === 'agent-history' && (
          <span className="text-xs text-muted-foreground">
            {t('onboarding.sources.agent.alsoGit')}
          </span>
        )}
      </span>
    </button>
  )
}
