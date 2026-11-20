/**
 * The cost card and the note on how the numbers are worked out
 * (`agent-history.md` §2.4 items 12 and 13). Cost is shown at API prices:
 * what the tools recorded, plus an estimate at list prices for the rest
 * while that switch is on. The switch is the same saved setting as in
 * Settings › Agent conversations. A model with no price, or a
 * conversation with no usage record, stays unknown and is counted apart.
 */
import { SectionCard } from '@/components/app/section-card'
import { Switch } from '@/components/ui/switch'
import type { InsightsCost } from '@/lib/backend-client/agent-insights'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { usd } from './words'

export function CostCard({
  cost,
  onEstimate,
}: {
  cost: InsightsCost
  onEstimate: (on: boolean) => void
}) {
  const { t, locale } = useI18n()
  const money = (value: number | null) =>
    value === null ? t('agentInsightsCards.cost.none') : usd(value, locale)
  const conversations = (count: number) =>
    t('agentInsightsCards.conversations', { count })
  const estimate = cost.includeEstimates
  const switchId = 'agent-insights-estimate'
  return (
    <SectionCard
      title={t('agentInsightsCards.cost.title')}
      subtitle={t('agentInsightsCards.cost.subtitle')}
    >
      <div className="flex flex-wrap items-baseline gap-x-2">
        <span className="text-2xl font-semibold tracking-[-0.02em] tabular">
          {cost.totalUsd === null ? '—' : usd(cost.totalUsd, locale)}
        </span>
        <span className="text-xs text-muted-foreground">
          {estimate
            ? t('agentInsightsCards.cost.withEstimates')
            : t('agentInsightsCards.cost.recordedOnly')}
        </span>
      </div>
      <dl className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5 text-[13px]">
        <dt>{t('agentInsightsCards.cost.recorded')}</dt>
        <dd className="text-right tabular">
          {t('agentInsightsCards.cost.amount', {
            amount: money(cost.recordedUsd),
            conversations: conversations(cost.recordedConversations),
          })}
        </dd>
        <dt
          className={cn(
            'transition-colors',
            !estimate && 'text-muted-foreground line-through',
          )}
        >
          {t('agentInsightsCards.cost.estimated')}
        </dt>
        <dd
          className={cn(
            'text-right tabular transition-colors',
            !estimate && 'text-muted-foreground line-through',
          )}
        >
          {t('agentInsightsCards.cost.amount', {
            amount: money(cost.availableEstimatedUsd),
            conversations: conversations(cost.estimatedConversations),
          })}
        </dd>
        {cost.unpricedConversations > 0 && (
          <>
            <dt className="text-muted-foreground">
              {t('agentInsightsCards.cost.unpriced')}
            </dt>
            <dd className="text-right text-muted-foreground tabular">
              {conversations(cost.unpricedConversations)}
            </dd>
          </>
        )}
      </dl>
      <div className="flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-[13px]">
        <label htmlFor={switchId} className="flex flex-col">
          {t('agentInsightsCards.cost.switch')}
          <span className="text-xs text-muted-foreground">
            {t('agentInsightsCards.cost.switchNote', {
              version: cost.priceTableVersion,
            })}
          </span>
        </label>
        <Switch id={switchId} checked={estimate} onCheckedChange={onEstimate} />
      </div>
    </SectionCard>
  )
}

export function MethodNote() {
  const { t } = useI18n()
  const items = [
    ['conversationTerm', 'conversation'],
    ['timeTerm', 'time'],
    ['tokensTerm', 'tokens'],
    ['missingTerm', 'missing'],
  ] as const
  return (
    <SectionCard title={t('agentInsightsCards.method.title')}>
      <ul className="flex list-disc flex-col gap-1.5 pl-5 text-[13px] leading-relaxed text-muted-foreground">
        {items.map(([term, text]) => (
          <li key={term}>
            <span className="text-foreground">
              {t(`agentInsightsCards.method.${term}`)}
            </span>
            {t('agentInsightsCards.method.colon')}
            {t(`agentInsightsCards.method.${text}`)}
          </li>
        ))}
        <li>{t('agentInsightsCards.method.local')}</li>
      </ul>
    </SectionCard>
  )
}
