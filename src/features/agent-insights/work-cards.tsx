/**
 * Where the time went and what was done (`agent-history.md` §2.4 items 2
 * and 8–11): the projects with the most summed time, the longest
 * conversations, the code changed, the tool calls and the models. Each
 * says how many conversations have its figure when not all do.
 */
import { ArrowRight, MessagesSquare } from 'lucide-react'
import { Link } from 'react-router-dom'
import { SectionCard } from '@/components/app/section-card'
import { ToolBadge, ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import { chatTitle } from '@/features/agent-chats/chat-words'
import type {
  InsightsBreakdowns,
  InsightsOverview,
} from '@/lib/backend-client/agent-insights'
import { useI18n } from '@/lib/i18n'
import { useInsightsFormat } from '@/features/insights/zone'
import { MODEL_COLORS, OTHER_COLOR, TOOL_COLOR } from './colors'
import { ProjectIcon } from './project-bits'
import { historyWithChat, projectPath, projectsPath, type Scope } from './scope'
import { ShareList } from './share-list'
import { hours, projectName, share, spanKey, tokenTotal } from './words'

const quiet = 'text-[13px] text-muted-foreground'

export function TopProjectsCard({
  rows,
  overview,
  scope,
}: {
  rows: InsightsBreakdowns['topProjects']
  overview: InsightsOverview
  scope: Scope
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  return (
    <SectionCard
      title={t('agentInsightsCards.projects.title')}
      subtitle={t('agentInsightsCards.projects.subtitle', {
        span: t(spanKey(scope.range)),
      })}
      action={
        <Link
          to={projectsPath()}
          className="flex shrink-0 items-center gap-1 text-xs whitespace-nowrap text-muted-foreground hover:text-foreground"
        >
          {t('agentInsightsCards.projects.all', {
            count: overview.projectCount,
          })}
          <ArrowRight className="size-3" aria-hidden />
        </Link>
      }
    >
      {rows.length === 0 ? (
        <p className={quiet}>{t('agentInsightsCards.projects.empty')}</p>
      ) : (
        <ShareList
          rows={rows.map((row, index) => {
            const conversations = t('agentInsightsCards.conversations', {
              count: row.conversations,
            })
            const tokens = row.tokens ? tokenTotal(row.tokens) : null
            const name = row.project ? projectName(t, row.project) : ''
            return {
              key: row.project?.id ?? `none-${index}`,
              label: row.project ? (
                <Link
                  to={projectPath(row.project.id)}
                  className="flex min-w-0 items-center gap-1.5 hover:underline"
                >
                  <ProjectIcon project={row.project} />
                  <span className="truncate">{name}</span>
                  <span className="flex shrink-0 items-center gap-0.5">
                    {row.harnesses.map((tool) => (
                      <span key={tool} title={t(toolName(tool))}>
                        <ToolIcon id={tool} />
                      </span>
                    ))}
                  </span>
                </Link>
              ) : (
                <span className="truncate text-muted-foreground">
                  {t('agentInsightsCards.longest.noProject')}
                </span>
              ),
              value: row.summedMs,
              display:
                row.summedMs === null
                  ? t('agentInsightsCards.projects.noTime')
                  : hours(t, f, row.summedMs),
              sub: [
                tokens === null
                  ? t('agentInsightsCards.projects.rowNoUsage', {
                      conversations,
                    })
                  : t('agentInsightsCards.projects.row', {
                      conversations,
                      tokens: f.compact(tokens),
                    }),
                row.conversationsWithUsage < row.conversations &&
                tokens !== null
                  ? t('agentInsightsCards.projects.partly', {
                      known: f.number(row.conversationsWithUsage),
                      count: row.conversations,
                    })
                  : null,
              ]
                .filter(Boolean)
                .join(' · '),
            }
          })}
        />
      )}
    </SectionCard>
  )
}

export function LongestCard({ rows }: { rows: InsightsBreakdowns['longest'] }) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  return (
    <SectionCard
      title={t('agentInsightsCards.longest.title')}
      subtitle={t('agentInsightsCards.longest.subtitle')}
    >
      {rows.length === 0 ? (
        <p className={quiet}>{t('agentInsightsCards.longest.empty')}</p>
      ) : (
        <ul className="-mx-2 flex flex-col">
          {rows.map((row) => (
            <li key={row.conversationId}>
              <Link
                to={historyWithChat(row.conversationId)}
                className="flex min-w-0 items-center gap-2.5 rounded-lg px-2 py-2 transition-colors hover:bg-foreground/[0.035] hover:no-underline"
              >
                <MessagesSquare
                  aria-hidden
                  className="size-3.5 shrink-0 text-muted-foreground"
                />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-[13px] font-medium">
                    {chatTitle(t, row)}
                  </span>
                  <span className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                    <ToolBadge id={row.harness} className="text-xs" />
                    <span className="truncate">
                      {row.project
                        ? projectName(t, row.project)
                        : t('agentInsightsCards.longest.noProject')}{' '}
                      ·{' '}
                      {t('agentInsightsCards.longest.turns', {
                        count: row.turnsInRange,
                      })}
                    </span>
                  </span>
                </span>
                <span className="shrink-0 text-[13px] font-medium tabular">
                  {hours(t, f, row.agentMsInRange)}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </SectionCard>
  )
}

export function ChangesCard({
  breakdowns,
  overview,
}: {
  breakdowns: InsightsBreakdowns
  overview: InsightsOverview
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const { linesAdded: added, linesRemoved: removed } = breakdowns
  return (
    <SectionCard
      title={t('agentInsightsCards.changes.title')}
      subtitle={t('agentInsightsCards.changes.subtitle')}
    >
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        {added === null || removed === null ? (
          <span className="text-2xl font-semibold">
            {t('agentInsightsCards.noRecord')}
          </span>
        ) : (
          <>
            <span
              className="text-2xl font-semibold tracking-[-0.02em] text-green tabular"
              aria-label={t('agentInsightsCards.changes.added', {
                count: f.number(added),
              })}
            >
              +{f.compact(added)}
            </span>
            <span
              className="text-lg font-medium text-red tabular"
              aria-label={t('agentInsightsCards.changes.removed', {
                count: f.number(removed),
              })}
            >
              −{f.compact(removed)}
            </span>
          </>
        )}
        <span className="text-xs text-muted-foreground">
          {t('agentInsightsCards.changes.touched', {
            count: overview.conversationCount,
            known: f.number(breakdowns.conversationsWithEdits),
          })}
        </span>
      </div>
      {breakdowns.editsWithUnknownLineCounts > 0 && (
        <p className="text-xs text-muted-foreground">
          {t('agentInsightsCards.changes.unknown', {
            count: breakdowns.editsWithUnknownLineCounts,
          })}
        </p>
      )}
    </SectionCard>
  )
}

export function ToolCallsCard({
  calls,
}: {
  calls: InsightsBreakdowns['toolCalls']
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  return (
    <SectionCard
      title={t('agentInsightsCards.calls.title')}
      subtitle={t('agentInsightsCards.calls.subtitle', { count: calls.total })}
    >
      {calls.top.length === 0 ? (
        <p className={quiet}>{t('agentInsightsCards.calls.empty')}</p>
      ) : (
        <ShareList
          rows={calls.top.map((call) => ({
            key: `${call.harness}:${call.name}`,
            label: (
              <>
                <span title={t(toolName(call.harness))}>
                  <ToolIcon id={call.harness} />
                </span>
                <span className="truncate font-mono text-[12px]">
                  {call.name}
                </span>
              </>
            ),
            value: call.count,
            display: f.compact(call.count),
            color: TOOL_COLOR[call.harness] ?? OTHER_COLOR,
          }))}
        />
      )}
    </SectionCard>
  )
}

export function ModelsCard({
  models,
}: {
  models: InsightsBreakdowns['models']
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const total = models.reduce(
    (sum, row) => sum + (row.tokens ? tokenTotal(row.tokens) : 0),
    0,
  )
  return (
    <SectionCard
      title={t('agentInsightsCards.models.title')}
      subtitle={t('agentInsightsCards.models.subtitle')}
    >
      {models.length === 0 ? (
        <p className={quiet}>{t('agentInsightsCards.models.empty')}</p>
      ) : (
        <ShareList
          rows={models.map((row, index) => {
            const tokens = row.tokens ? tokenTotal(row.tokens) : null
            return {
              key: row.model ?? `unknown-${index}`,
              label: (
                <span className="truncate font-mono text-[12px]">
                  {row.model ?? t('agentInsightsCards.tokens.unknownModel')}
                </span>
              ),
              value: tokens,
              display: tokens === null ? '—' : (share(f, tokens, total) ?? '—'),
              sub: t('agentInsightsCards.models.main', {
                count: row.primaryConversations,
              }),
              color: MODEL_COLORS[index] ?? OTHER_COLOR,
            }
          })}
        />
      )}
    </SectionCard>
  )
}
