/**
 * @file reopened-investigations-section.tsx
 * @description `/intelligence` secondary grid里的 Reopened Investigations 卡片实现。
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - 读取当前 scope 下的 reopened investigation 摘要并渲染有意义的卡片。
 * - 复用既有 query-family / page deep-link grammar，而不是自行决定跳转目标。
 * - 在数据不足时退化为空或隐藏，而不是让低信号卡片占据版面。
 *
 * ## Non-Responsibilities
 * - 不负责 route-level section 排序或布局编排。
 * - 不负责定义 reopened investigation 的过滤规则来源；这里只消费已有 heuristics。
 * - 不负责共享 explainability 或 metadata 组件的样式与交互。
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` 提供 deterministic reopened investigation 数据。
 * - `./heuristics` 提供 low-signal 过滤，保证二级卡片只展示真正像“重新调查”的项目。
 * - `lib/intelligence` 提供 canonical reopened investigation deep-link grammar。
 *
 * ## Performance Notes
 * - 只渲染过滤后的前 8 条结果，避免 secondary grid 在大档案下膨胀。
 * - 过滤逻辑基于已界定长度的 overview payload，不触碰原始历史流。
 */

import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import { useAsyncData, type DateRange } from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { reopenedInvestigationHref } from '../../../../lib/intelligence'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'
import { isSearchBackedReopenedInvestigation } from './heuristics'

type ReopenedInvestigationsSectionProps = {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Keeps the reopened-investigation card focused on recurring search questions
 * instead of login pages, callback URLs, or one-off navigation noise.
 *
 * @param dateRange The active Intelligence time scope used for reopened-investigation reads and deep links.
 * @param profileId Optional profile scope used to keep reopen signals and resulting links inside one profile.
 * @param scopeLabel Human-readable scope text shown next to freshness metadata.
 * @param t Route-local translator for the existing reopened-investigation copy contract.
 * @returns The reopened-investigation card, an empty/loading state, or `null` when the ready payload contains no meaningful search-backed signals.
 *
 * Edge cases:
 * - Hides the whole section once the ready payload collapses to low-signal items after filtering.
 * - Preserves the existing anchor-type badge and explainability contract for every visible card.
 */
export function ReopenedInvestigationsSection({
  dateRange,
  profileId,
  scopeLabel,
  t,
}: ReopenedInvestigationsSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getReopenedInvestigations(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekReopenedInvestigations(dateRange, profileId),
    },
  )
  const reopened = (data?.data ?? []).filter(
    isSearchBackedReopenedInvestigation,
  )

  if (!loading && reopened.length === 0 && data?.meta.state === 'ready') {
    return null
  }

  return (
    <section className="intelligence-section reopened-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('reopenedTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : reopened.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('reopenedEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <div className="reopened-list">
            {reopened.slice(0, 8).map((item) => (
              <div key={item.investigationId} className="reopened-card">
                <div className="reopened-card__header">
                  <span
                    className={`reopened-card__anchor-badge reopened-card__anchor-badge--${item.anchorType}`}
                  >
                    {item.anchorType === 'query_family'
                      ? t('reopenedAnchorQuery')
                      : t('reopenedAnchorPage')}
                  </span>
                  <Link
                    className="reopened-card__label intelligence-link"
                    to={reopenedInvestigationHref({
                      anchorId: item.anchorId,
                      anchorType: item.anchorType,
                      dateRange,
                      profileId,
                    })}
                  >
                    {item.anchorLabel}
                  </Link>
                </div>
                <div className="reopened-card__meta">
                  <span>
                    {t('reopenedOccurrences', {
                      count: item.occurrenceCount,
                    })}
                  </span>
                  <span>
                    {t('reopenedDistinctDays', { days: item.distinctDays })}
                  </span>
                </div>
                <span className="reopened-card__dates">
                  {item.firstSeenAt} - {item.lastSeenAt}
                </span>
                <ExplainabilityPanel
                  entityType="reopened_investigation"
                  entityId={item.investigationId}
                  t={t}
                />
              </div>
            ))}
          </div>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}
