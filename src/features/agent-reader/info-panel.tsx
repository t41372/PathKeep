/**
 * Everything known about one conversation, grouped the way people ask
 * (`agent-history.md` §4.7): how long, how much, where, what changed, with
 * what, and how it is kept. A row the tool did not record is left out;
 * only Agent time and tokens say outright that nothing was recorded,
 * which is never shown as 0. Costs follow the two saved cost switches
 * (`agent-history.md` §1.8), the ones Insights' cost card changes: the
 * backend leaves out what they turn off, and the line about replies
 * without a price shows only while estimates are on.
 */
import { ChevronDown, ExternalLink } from 'lucide-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Skeleton } from '@/components/ui/skeleton'
import { supportClient } from '@/lib/backend-client/support'
import type {
  ChatHeader,
  ChatInfo,
  TokenKinds,
} from '@/lib/backend-client/agent-chats'
import { describeError } from '@/lib/errors'
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'
import { useChatInfo } from '@/lib/queries/agent-chats'
import { useInsightsPreferences } from '@/lib/queries/agent-insights'
import { durationText, tokenTotal, toolLabel } from './reader-words'

function Section({
  title,
  open,
  children,
}: {
  title: string
  open?: boolean
  children: ReactNode
}) {
  return (
    <Collapsible
      defaultOpen={open}
      className="group/section border-b px-4 py-2.5 last:border-b-0"
    >
      <CollapsibleTrigger className="flex w-full items-center justify-between py-0.5 text-[12px] font-semibold text-muted-foreground hover:text-foreground">
        {title}
        <ChevronDown
          className="size-3.5 transition-transform duration-200 group-data-[state=open]/section:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <dl className="flex flex-col gap-2 pt-1.5">{children}</dl>
      </CollapsibleContent>
    </Collapsible>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-[11.5px] text-muted-foreground">{label}</dt>
      <dd className="text-[13px] [overflow-wrap:anywhere]">{children}</dd>
    </div>
  )
}

const kinds: { key: keyof TokenKinds; label: MessageKey; className: string }[] =
  [
    {
      key: 'output',
      label: 'agentInfo.tokens.output',
      className: 'bg-chart-1',
    },
    { key: 'input', label: 'agentInfo.tokens.input', className: 'bg-chart-2' },
    {
      key: 'cacheWrite',
      label: 'agentInfo.tokens.cacheWrite',
      className: 'bg-chart-3',
    },
    {
      key: 'cacheRead',
      label: 'agentInfo.tokens.cacheRead',
      className: 'bg-chart-5',
    },
  ]

function TokenBar({ tokens, f }: { tokens: TokenKinds; f: Formatters }) {
  const { t } = useI18n()
  const total = tokenTotal(tokens) || 1
  return (
    <div className="flex flex-col gap-2">
      <div
        className="flex h-2 overflow-hidden rounded-full bg-muted"
        aria-hidden
      >
        {kinds.map((kind) => (
          <span
            key={kind.key}
            className={kind.className}
            style={{ width: `${(tokens[kind.key] / total) * 100}%` }}
          />
        ))}
      </div>
      <ul className="grid grid-cols-2 gap-x-3 gap-y-1 text-[12px]">
        {kinds.map((kind) => (
          <li key={kind.key} className="flex items-center gap-1.5">
            <span
              className={`size-2 shrink-0 rounded-sm ${kind.className}`}
              aria-hidden
            />
            <span className="text-muted-foreground">{t(kind.label)}</span>
            <span className="ml-auto tabular">
              {f.compact(tokens[kind.key])}
            </span>
          </li>
        ))}
      </ul>
    </div>
  )
}

function usd(value: number, locale: string) {
  return new Intl.NumberFormat(locale, {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: value < 1 ? 4 : 2,
  }).format(value)
}

function originKey(info: ChatInfo, header: ChatHeader): MessageKey | null {
  if (info.launchOrigin === 'cli') return 'agentInfo.madeWith.origins.cli'
  if (info.launchOrigin === 'cowork') return 'agentInfo.madeWith.origins.cowork'
  if (info.launchOrigin === 'desktop')
    return header.summary.harness === 'codex'
      ? 'agentInfo.madeWith.origins.codexApp'
      : 'agentInfo.madeWith.origins.desktop'
  return null
}

function Body({
  info,
  header,
  f,
  onOpenProject,
}: {
  info: ChatInfo
  header: ChatHeader
  f: Formatters
  onOpenProject: (project: string) => void
}) {
  const { t, locale } = useI18n()
  const saved = useInsightsPreferences().data
  // Unread switches count as on: the backend applies the saved ones anyway.
  const estimating = (saved?.showCost ?? true) && (saved?.estimateCost ?? true)
  const summary = header.summary
  const tool = toolLabel(t, summary.harness)
  const own = info.own
  const total = info.total
  const tokens = total.tokens
  const turnsKnown =
    own.recordedTurns + own.derivedTurns + own.unknownDurationTurns
  const timeSource =
    own.agentMs === null
      ? null
      : info.timeSource === 'recorded'
        ? t('agentInfo.time.recorded', { tool })
        : info.timeSource === 'derived'
          ? t('agentInfo.time.derived')
          : t('agentInfo.time.mixed', {
              count: turnsKnown,
              recorded: own.recordedTurns,
            })
  const origin = originKey(info, header)
  const models = [
    ...(info.primaryModel ? [info.primaryModel] : []),
    ...info.models
      .map((row) => row.model)
      .filter((model) => model !== info.primaryModel),
  ]
  const remotes = info.git?.remotes ?? []
  const pr = total.pullRequests.filter(
    (item) => item.url || item.number !== null,
  )
  return (
    <div className="flex flex-col">
      <Section title={t('agentInfo.time.title')} open>
        <Row label={t('agentInfo.time.agent')}>
          {own.agentMs === null
            ? t('agentInfo.time.none')
            : durationText(t, own.agentMs)}
          {timeSource && (
            <span className="block text-[12px] text-muted-foreground">
              {timeSource}
            </span>
          )}
        </Row>
        {summary.startedAt !== null && (
          <Row label={t('agentInfo.time.started')}>
            {f.date(summary.startedAt, {
              dateStyle: 'long',
              timeStyle: 'short',
            })}
          </Row>
        )}
        {summary.lastActivityAt !== null && (
          <Row label={t('agentInfo.time.lastActivity')}>
            {f.date(summary.lastActivityAt, {
              dateStyle: 'long',
              timeStyle: 'short',
            })}
          </Row>
        )}
        <Row label={t('agentInfo.time.turns')}>
          {t('agentInfo.time.turnsCalls', {
            turns: t('agentInfo.time.turnCount', { count: own.turns }),
            calls: t('agentInfo.time.callCount', { count: own.calls }),
          })}
        </Row>
      </Section>
      <Section title={t('agentInfo.tokens.title')} open>
        {tokens ? (
          <>
            <div className="text-[20px] font-semibold tracking-tight tabular">
              {f.number(tokenTotal(tokens))}
            </div>
            <TokenBar tokens={tokens} f={f} />
            {total.reasoningTokens !== null && total.reasoningTokens > 0 && (
              <p className="text-[12px] text-muted-foreground">
                {t('agentInfo.tokens.reasoning', {
                  count: f.compact(total.reasoningTokens),
                })}
              </p>
            )}
            {info.subagentTokens !== null && info.subagentTokens > 0 && (
              <p className="text-[12px] text-muted-foreground">
                {t('agentInfo.tokens.subagents', {
                  count: f.compact(info.subagentTokens),
                  share: f.percent(
                    info.subagentTokens / (tokenTotal(tokens) || 1),
                  ),
                })}
              </p>
            )}
            {total.recordedUsd != null && total.recordedUsd > 0 && (
              <Row label={t('agentInfo.tokens.cost')}>
                {usd(total.recordedUsd, locale)}
                <span className="block text-[12px] text-muted-foreground">
                  {t('agentInfo.tokens.recorded', { tool })}
                </span>
              </Row>
            )}
            {total.estimatedUsd != null && total.estimatedUsd > 0 && (
              <Row label={t('agentInfo.tokens.estimated')}>
                {usd(total.estimatedUsd, locale)}
                {info.priceTable && (
                  <span className="block text-[12px] text-muted-foreground">
                    {t('agentInfo.tokens.estimatedBy', {
                      version: info.priceTable,
                    })}
                  </span>
                )}
              </Row>
            )}
            {estimating && total.unpricedResponses > 0 && (
              <p className="text-[12px] text-muted-foreground">
                {t('agentInfo.tokens.unpriced', {
                  count: total.unpricedResponses,
                })}
              </p>
            )}
          </>
        ) : (
          <p className="text-[13px] text-muted-foreground">
            {t('agentInfo.tokens.none', { tool })}
          </p>
        )}
      </Section>
      <Section title={t('agentInfo.where.title')} open>
        {summary.project && (
          <Row label={t('agentInfo.where.project')}>
            <button
              type="button"
              className="text-left underline underline-offset-2"
              onClick={() => onOpenProject(summary.project!.id)}
            >
              {summary.project.label}
            </button>
          </Row>
        )}
        {info.folder && (
          <Row label={t('agentInfo.where.folder')}>
            <span className="font-mono text-[12px]">{info.folder}</span>
            {info.folderState === 'worktreeGone' && (
              <span className="block text-[12px] text-muted-foreground">
                {t('agentInfo.where.worktreeGone')}
              </span>
            )}
            {info.folderState === 'gone' && (
              <span className="block text-[12px] text-muted-foreground">
                {t('agentInfo.where.folderGone')}
              </span>
            )}
          </Row>
        )}
        {remotes.length > 0 && (
          <Row label={t('agentInfo.where.remote')}>
            {remotes.map((remote) => (
              <span key={remote} className="block font-mono text-[12px]">
                {remote}
              </span>
            ))}
          </Row>
        )}
        {info.git?.branch && (
          <Row label={t('agentInfo.where.branch')}>
            <span className="font-mono text-[12px]">{info.git.branch}</span>
          </Row>
        )}
        {info.startCommit && (
          <Row label={t('agentInfo.where.startCommit')}>
            <span className="font-mono text-[12px]">{info.startCommit}</span>
          </Row>
        )}
      </Section>
      <Section title={t('agentInfo.changes.title')}>
        {total.addedLines + total.removedLines > 0 ||
        total.editedFileCount > 0 ? (
          <Row label={t('agentInfo.changes.code')}>
            {t('agentInfo.changes.lines', {
              added: f.number(total.addedLines),
              removed: f.number(total.removedLines),
              files: t('agentInfo.changes.files', {
                count: total.editedFileCount,
              }),
            })}
            {total.unknownEditCounts > 0 && (
              <span className="block text-[12px] text-muted-foreground">
                {t('agentInfo.changes.unknownLines', {
                  count: total.unknownEditCounts,
                })}
              </span>
            )}
          </Row>
        ) : (
          <p className="text-[13px] text-muted-foreground">
            {t('agentInfo.changes.none')}
          </p>
        )}
        {pr.map((item, index) => {
          const label = `${item.repository ?? ''}#${item.number ?? ''}`
          return (
            <Row key={`${label}-${index}`} label={t('agentInfo.changes.pr')}>
              {item.url ? (
                <button
                  type="button"
                  className="inline-flex items-center gap-1 underline underline-offset-2"
                  onClick={() =>
                    void supportClient
                      .openExternalUrl(item.url!)
                      .catch((error: unknown) =>
                        toast.error(t('agentInfo.changes.prFailed'), {
                          description: describeError(error),
                        }),
                      )
                  }
                >
                  {label}
                  <ExternalLink className="size-3" aria-hidden />
                </button>
              ) : (
                label
              )}
            </Row>
          )
        })}
      </Section>
      <Section title={t('agentInfo.madeWith.title')}>
        <Row label={t('agentInfo.madeWith.tool')}>
          {[tool, info.toolVersion].filter(Boolean).join(' ')}
        </Row>
        {origin && (
          <Row label={t('agentInfo.madeWith.origin')}>{t(origin)}</Row>
        )}
        {models.length > 0 && (
          <Row label={t('agentInfo.madeWith.models')}>
            {models.join(t('agentReader.rows.listSeparator'))}
          </Row>
        )}
        {info.effort && (
          <Row label={t('agentInfo.madeWith.effort')}>{info.effort}</Row>
        )}
        {info.permissionMode && (
          <Row label={t('agentInfo.madeWith.permission')}>
            {info.permissionMode}
          </Row>
        )}
        {total.compactions > 0 && (
          <Row label={t('agentInfo.madeWith.compactions')}>
            {t('agentInfo.madeWith.compactionCount', {
              count: total.compactions,
            })}
          </Row>
        )}
        {total.apiErrors > 0 && (
          <Row label={t('agentInfo.madeWith.apiErrors')}>
            {t('agentInfo.madeWith.apiErrorCount', { count: total.apiErrors })}
            {info.allErrorsRetried && t('agentInfo.madeWith.allRetried')}
          </Row>
        )}
      </Section>
      <Section title={t('agentInfo.saved.title')}>
        <Row label={t('agentInfo.saved.state')}>
          {summary.onlyInPathKeep
            ? t('agentInfo.saved.onlyHere')
            : t('agentInfo.saved.alsoThere')}
        </Row>
        {info.sourcePath && (
          <Row label={t('agentInfo.saved.source')}>
            <span className="font-mono text-[11.5px]">{info.sourcePath}</span>
          </Row>
        )}
        <Row label={t('agentInfo.saved.size')}>
          {t('agentInfo.saved.sizeLine', {
            size: f.bytes(info.sourceBytes),
            records: t('agentInfo.saved.records', {
              count: info.physicalRecords,
            }),
          })}
          {info.subagentSourceCount > 0 &&
            t('agentInfo.saved.subagentFiles', {
              count: info.subagentSourceCount,
            })}
        </Row>
        {info.lastSavedAt !== null && (
          <Row label={t('agentInfo.saved.lastBackup')}>
            {f.date(info.lastSavedAt, {
              dateStyle: 'long',
              timeStyle: 'short',
            })}
          </Row>
        )}
      </Section>
    </div>
  )
}

export function InfoPanel({
  header,
  f,
  onOpenProject,
}: {
  header: ChatHeader
  f: Formatters
  onOpenProject: (project: string) => void
}) {
  const { t } = useI18n()
  const info = useChatInfo(header.summary.id, true)
  return (
    <aside
      aria-label={t('agentInfo.panel')}
      className="animate-rise w-[300px] shrink-0 overflow-y-auto border-l bg-background/30"
    >
      {info.data ? (
        <Body
          info={info.data}
          header={header}
          f={f}
          onOpenProject={onOpenProject}
        />
      ) : info.isError ? (
        <div className="flex flex-col gap-2 p-4 text-[12.5px]">
          <p>{t('agentInfo.failed')}</p>
          <p className="break-words text-muted-foreground">
            {describeError(info.error)}
          </p>
          <button
            type="button"
            className="self-start underline underline-offset-2"
            onClick={() => void info.refetch()}
          >
            {t('common.retry')}
          </button>
        </div>
      ) : (
        <div
          role="status"
          aria-label={t('agentInfo.loading')}
          className="flex flex-col gap-3 p-4"
        >
          {Array.from({ length: 6 }, (_, index) => (
            <Skeleton
              key={index}
              className="h-3.5"
              style={{ width: `${80 - index * 8}%` }}
            />
          ))}
        </div>
      )}
    </aside>
  )
}
