/**
 * The two token cards (`agent-history.md` §2.4 items 3 and 4). "Token
 * usage" puts each reply's tokens in the bar of its own time, split by
 * kind, by tool or by model, cache reads left out unless switched on.
 * "Token mix" shows how the four kinds and the tools split the range,
 * and how many conversations have a usage record at all: those without
 * one are left out, never counted as 0.
 */
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { MouseHandlerDataParam } from 'recharts'
import { SectionCard } from '@/components/app/section-card'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type {
  InsightsBucket,
  InsightsHarnessRow,
  InsightsOverview,
  InsightsSeries,
  TokenKinds,
} from '@/lib/backend-client/agent-insights'
import { AGENT_TOOLS } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { dateKeyIn } from '@/lib/time-zone'
import { useInsightsFormat } from '@/features/insights/zone'
import { BucketTooltip, type BucketWords } from './bucket-tooltip'
import { MODEL_COLORS, OTHER_COLOR, TOKEN_KINDS, TOOL_COLOR } from './colors'
import { Legend } from './legend'
import { historyOnDay, type Scope } from './scope'
import { ShareList } from './share-list'
import {
  perKey,
  share,
  spanKey,
  tokenTotal,
  tokenUnit,
  tokensOf,
  bucketTick,
  bucketTitle,
} from './words'

type Split = 'kind' | 'tool' | 'model'

interface Series {
  key: string
  label: string
  color: string
  /** Raw tokens in a bucket; `null` when nothing there was recorded. */
  of: (bucket: InsightsBucket) => number | null
}

type Row = Record<string, number | string | null> & {
  key: string
  start: number
  end: number
}

function seriesFor(
  split: Split,
  series: InsightsSeries,
  withCacheRead: boolean,
  words: {
    kind: (key: keyof TokenKinds) => string
    tool: (tool: (typeof AGENT_TOOLS)[number]) => string
    unknownModel: string
    others: (count: number) => string
  },
): Series[] {
  const sum = (tokens: TokenKinds | null | undefined) =>
    tokens ? tokensOf(tokens, withCacheRead) : null
  if (split === 'kind')
    return TOKEN_KINDS.filter(
      (kind) => withCacheRead || kind.key !== 'cacheRead',
    ).map((kind) => ({
      key: kind.key,
      label: words.kind(kind.key),
      color: kind.color,
      of: (bucket) => bucket.tokens?.[kind.key] ?? null,
    }))
  if (split === 'tool') {
    const present = new Set(
      series.buckets.flatMap((bucket) => Object.keys(bucket.tokensByHarness)),
    )
    return AGENT_TOOLS.filter((tool) => present.has(tool)).map((tool) => ({
      key: tool,
      label: words.tool(tool),
      color: TOOL_COLOR[tool],
      of: (bucket) => sum(bucket.tokensByHarness[tool]),
    }))
  }
  const models: Series[] = series.modelKeys.map((model, index) => ({
    key: `model${index}`,
    label: model || words.unknownModel,
    color: MODEL_COLORS[index] ?? OTHER_COLOR,
    of: (bucket) => sum(bucket.tokensByModel[index]),
  }))
  if (series.otherModelCount > 0)
    models.push({
      key: 'other',
      label: words.others(series.otherModelCount),
      color: OTHER_COLOR,
      of: (bucket) => sum(bucket.otherModelTokens),
    })
  return models
}

export function TokensCard({
  series,
  scope,
  zone,
}: {
  series: InsightsSeries | null
  scope: Scope
  zone: string
}) {
  const { t, language } = useI18n()
  const f = useInsightsFormat()
  const navigate = useNavigate()
  const [split, setSplit] = useState<Split>('kind')
  const [withCacheRead, setWithCacheRead] = useState(false)
  const unit = series?.unit ?? 'day'

  const chart = useMemo(() => {
    if (!series) return null
    const lines = seriesFor(split, series, withCacheRead, {
      kind: (key) => t(TOKEN_KINDS.find((kind) => kind.key === key)!.label),
      tool: (tool) => t(toolName(tool)),
      unknownModel: t('agentInsightsCards.tokens.unknownModel'),
      others: (count) => t('agentInsightsCards.tokens.otherModels', { count }),
    })
    const max = Math.max(
      0,
      ...series.buckets.map((bucket) =>
        lines.reduce((total, line) => total + (line.of(bucket) ?? 0), 0),
      ),
    )
    const scale = tokenUnit(max, language)
    const rows: Row[] = series.buckets.map((bucket) => {
      const row: Row = {
        key: String(bucket.bucketStart),
        start: bucket.bucketStart,
        end: bucket.bucketEnd,
      }
      for (const line of lines) {
        const raw = line.of(bucket)
        row[line.key] =
          raw === null ? null : Math.round((raw / scale.divisor) * 100) / 100
        row[`raw:${line.key}`] = raw
      }
      return row
    })
    const config = Object.fromEntries(
      lines.map((line) => [
        line.key,
        {
          label: line.label,
          colors: { light: [line.color], dark: [line.color] },
        },
      ]),
    )
    return { lines, rows, config, scale }
  }, [series, split, withCacheRead, t, language])

  const byBucket = useMemo(
    () => new Map(series?.buckets.map((b) => [String(b.bucketStart), b])),
    [series],
  )
  const describe = (row: Row): BucketWords => {
    const bucket = byBucket.get(row.key)
    return {
      title: bucketTitle(t, f, unit, row.start, row.end, zone),
      lines: (chart?.lines ?? []).map((line) => {
        const raw = row[`raw:${line.key}`]
        return {
          key: line.key,
          label: line.label,
          color: line.color,
          value:
            typeof raw === 'number'
              ? f.compact(raw)
              : t('agentInsightsCards.noRecord'),
        }
      }),
      note:
        bucket && bucket.conversationsWithUsage < bucket.conversationCount
          ? t('agentInsights.coverage', {
              count: bucket.conversationCount,
              known: f.number(bucket.conversationsWithUsage),
            })
          : null,
    }
  }
  const openDay = (state: MouseHandlerDataParam) => {
    if (unit !== 'day' || state.activeLabel === undefined) return
    void navigate(
      historyOnDay(scope, dateKeyIn(Number(state.activeLabel), zone)),
    )
  }
  const splitLabel = {
    kind: t('agentInsightsCards.tokens.byKind'),
    tool: t('agentInsightsCards.tokens.byTool'),
    model: t('agentInsightsCards.tokens.byModel'),
  }

  return (
    <SectionCard
      title={t('agentInsightsCards.tokens.title')}
      subtitle={t('agentInsightsCards.tokens.subtitle', {
        span: t(spanKey(scope.range)),
        per: t(perKey(unit)),
        unit: chart?.scale.key ? t(chart.scale.key) : '',
      })}
      action={
        <ToggleGroup
          type="single"
          size="sm"
          variant="outline"
          value={split}
          aria-label={t('agentInsightsCards.tokens.split')}
          onValueChange={(next) => next && setSplit(next as Split)}
        >
          {(['kind', 'tool', 'model'] as const).map((id) => (
            <ToggleGroupItem key={id} value={id} className="px-2 text-xs">
              {splitLabel[id]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      }
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Legend
          items={(chart?.lines ?? []).map((line) => ({
            label: line.label,
            color: line.color,
          }))}
        />
        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          {t('agentInsightsCards.tokens.cacheRead')}
          <Switch checked={withCacheRead} onCheckedChange={setWithCacheRead} />
        </label>
      </div>
      <div
        role="img"
        aria-label={t('agentInsightsCards.tokens.chart', {
          per: t(perKey(unit)),
          split: splitLabel[split],
        })}
        className="h-[220px]"
      >
        <EvilBarChart
          key={`${split}-${withCacheRead}`}
          data={chart?.rows ?? []}
          config={chart?.config ?? {}}
          xDataKey="key"
          stackType="stacked"
          isLoading={!chart}
          className="aspect-auto h-full"
          barRadius={2}
          chartProps={{
            onClick: openDay,
            style: unit === 'day' ? { cursor: 'pointer' } : undefined,
          }}
        >
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis
            dataKey="key"
            minTickGap={28}
            tickFormatter={(value: string) =>
              bucketTick(f, unit, Number(value), zone)
            }
            className="font-mono"
          />
          <EvilBarChart.YAxis
            tickFormatter={(value: number) => f.number(value)}
            className="font-mono"
            width={44}
          />
          <BucketTooltip<Row> describe={describe} />
          {(chart?.lines ?? []).map((line) => (
            <EvilBarChart.Bar key={line.key} dataKey={line.key} />
          ))}
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}

/** One bar split into the four kinds, with each kind's count and share. */
function KindBar({ tokens }: { tokens: TokenKinds }) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const total = tokenTotal(tokens)
  return (
    <div className="flex flex-col gap-2">
      <div
        aria-hidden
        className="flex h-2 overflow-hidden rounded-full bg-muted"
      >
        {TOKEN_KINDS.map((kind) => (
          <span
            key={kind.key}
            className="transition-[flex-grow] duration-500"
            style={{ flexGrow: tokens[kind.key], background: kind.color }}
          />
        ))}
      </div>
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
        {TOKEN_KINDS.map((kind) => (
          <li key={kind.key} className="flex items-center gap-1.5">
            <span
              aria-hidden
              className="size-2 rounded-[2px]"
              style={{ background: kind.color }}
            />
            {t(kind.label)}
            <span className="text-foreground tabular">
              {f.compact(tokens[kind.key])}
            </span>
            <span className="tabular">
              {share(f, tokens[kind.key], total) ?? '—'}
            </span>
          </li>
        ))}
      </ul>
    </div>
  )
}

export function TokenMixCard({
  overview,
  byHarness,
}: {
  overview: InsightsOverview
  byHarness: InsightsHarnessRow[] | null
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const tokens = overview.tokens
  const total = tokens ? tokenTotal(tokens) : 0
  const note = overview.usageCoverageNote
  const partial = overview.conversationsWithUsage < overview.conversationCount
  return (
    <SectionCard
      title={t('agentInsightsCards.mix.title')}
      subtitle={t('agentInsightsCards.mix.subtitle')}
    >
      {tokens ? (
        <>
          <KindBar tokens={tokens} />
          <p className="text-[13px] leading-relaxed text-muted-foreground">
            {t('agentInsightsCards.mix.cacheShare', {
              share: share(f, tokens.cacheRead, total) ?? '—',
            })}
          </p>
        </>
      ) : (
        <p className="text-[13px] text-muted-foreground">
          {t('agentInsightsCards.tokens.empty')}
        </p>
      )}
      {byHarness && byHarness.some((row) => row.tokens) && (
        <ShareList
          rows={byHarness.map((row) => {
            const sum = row.tokens ? tokenTotal(row.tokens) : null
            return {
              key: row.harness,
              label: <ToolBadge id={row.harness} />,
              value: sum,
              display: sum === null ? '—' : f.compact(sum),
              sub: t('agentInsightsCards.conversations', {
                count: row.conversations,
              }),
              color: TOOL_COLOR[row.harness],
            }
          })}
        />
      )}
      {partial && (
        <p className="text-xs leading-relaxed text-muted-foreground">
          {t('agentInsightsCards.mix.coverage', {
            count: overview.conversationCount,
            known: f.number(overview.conversationsWithUsage),
          })}{' '}
          {note &&
            t('agentInsightsCards.mix.older', {
              tool: t(toolName(note.harness)),
              date: f.date(note.firstUsageAt, { dateStyle: 'long' }),
            })}{' '}
          {t('agentInsightsCards.mix.notZero')}
        </p>
      )}
    </SectionCard>
  )
}
