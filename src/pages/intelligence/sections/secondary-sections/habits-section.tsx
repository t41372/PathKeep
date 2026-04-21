/**
 * @file habits-section.tsx
 * @description `/intelligence` secondary grid里的 Habits 卡片实现，合并稳定习惯与中断习惯。
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - 组合 habit patterns 与 interrupted habits 两条 deterministic 读路径。
 * - 通过 shared domain links 和 explainability panel 呈现可行动的习惯线索。
 * - 在数据缺失时使用统一 skeleton / empty state，而不是让 route shell 处理细节。
 *
 * ## Non-Responsibilities
 * - 不负责定义 habits 在页面中的排序或布局。
 * - 不负责翻译 key、domain route grammar 或 explainability contract。
 * - 不负责跨 section 的 filtering heuristic 抽象。
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` 提供 habit pattern 与 interrupted habit 数据。
 * - `section-meta` / `section-body` 保持 secondary card chrome 一致。
 * - `shared.ts` 提供 metadata 合并与 ISO 日期格式化 helper。
 *
 * ## Performance Notes
 * - 习惯与中断列表都做了硬上限 slice，避免大档案下 secondary grid 渲染过多行。
 * - 两次 async read 都走既有 cached overview contract，不在前端重算习惯统计。
 */

import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type HabitPattern,
  type InterruptedHabit,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import { firstSectionMeta, formatIsoDate, type T } from '../shared'

type HabitsSectionProps = {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Brings repeated routines and broken routines into one bounded card so the
 * user can see both "what sticks" and "what slipped" without scanning two surfaces.
 *
 * @param dateRange The active time scope for habit-pattern reads.
 * @param domainHref Shared domain deep-link builder so this section does not invent navigation behavior.
 * @param profileId Optional profile scope used by deterministic reads and explainability IDs.
 * @param scopeLabel Human-readable scope text shown beside freshness metadata.
 * @param t Route-local translator for the existing habits copy contract.
 * @returns The habits section, a loading/empty state, or a bounded combined card with interrupted and stable habits.
 *
 * Edge cases:
 * - Interrupted habits still render even when the current-range pattern list is empty.
 * - Metadata is taken from the first ready data source so partial results can still show freshness honestly.
 */
export function HabitsSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: HabitsSectionProps) {
  const patterns = useAsyncData(
    () => api.getHabitPatterns(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekHabitPatterns(dateRange, profileId),
    },
  )
  const interrupted = useAsyncData(
    () => api.getInterruptedHabits(profileId),
    [profileId],
    {
      getCached: () => api.peekInterruptedHabits(profileId),
    },
  )
  const patternsData = patterns.data?.data ?? []
  const interruptedData = interrupted.data?.data ?? []
  const empty =
    !patterns.loading &&
    !interrupted.loading &&
    patternsData.length === 0 &&
    interruptedData.length === 0
  const meta = firstSectionMeta(patterns.data, interrupted.data)

  return (
    <section className="intelligence-section habits-section">
      <h2 className="intelligence-section__title">{t('habitsTitle')}</h2>
      {meta ? (
        <IntelligenceSectionMeta meta={meta} scopeLabel={scopeLabel} />
      ) : null}
      {patterns.loading || interrupted.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : empty ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('habitsEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="habits-body">
          {interruptedData.length > 0 ? (
            <div className="habits-interrupted">
              <h3 className="habits-body__subtitle">
                {t('habitsInterruptedTitle')}
              </h3>
              <ul className="habits-interrupted__list">
                {interruptedData.slice(0, 5).map((habit, index) => (
                  <InterruptedHabitRow
                    key={index}
                    domainHref={domainHref}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
          {patternsData.length > 0 ? (
            <div className="habits-patterns">
              <h3 className="habits-body__subtitle">
                {t('habitsPatternsTitle')}
              </h3>
              <ul className="habits-patterns__list">
                {patternsData.slice(0, 12).map((habit, index) => (
                  <HabitPatternRow
                    key={index}
                    domainHref={domainHref}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function HabitPatternRow({
  domainHref,
  habit,
  profileId,
  t,
}: {
  domainHref: (domain: string) => string
  habit: HabitPattern
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row">
      <div className="habit-row__main">
        <div className="habit-row__header">
          <Link
            className="habit-row__domain intelligence-link"
            to={domainHref(habit.registrableDomain)}
          >
            {habit.displayName ?? habit.registrableDomain}
          </Link>
          <span
            className={`habit-row__type habit-row__type--${habit.habitType}`}
          >
            {t(`habitType_${habit.habitType}`)}
          </span>
        </div>
        <p className="habit-row__summary">
          {t('habitPatternSummary', {
            interval: habit.meanIntervalDays.toFixed(1),
            days: habit.visitCount,
          })}
        </p>
        <p className="habit-row__meta">
          {t('habitLastSeen', {
            date: formatIsoDate(habit.lastVisitedAt),
          })}
        </p>
      </div>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

function InterruptedHabitRow({
  domainHref,
  habit,
  profileId,
  t,
}: {
  domainHref: (domain: string) => string
  habit: InterruptedHabit
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row habit-row--interrupted">
      <div className="habit-row__main">
        <div className="habit-row__header">
          <Link
            className="habit-row__domain intelligence-link"
            to={domainHref(habit.registrableDomain)}
          >
            {habit.displayName ?? habit.registrableDomain}
          </Link>
          <span className="habit-row__type habit-row__type--interrupted">
            {t('habitInterruptedBadge')}
          </span>
        </div>
        <p className="habit-row__summary">
          {t('habitInterruptedSummary', {
            days: habit.daysSinceLastVisit,
            expected: habit.meanIntervalDays.toFixed(1),
          })}
        </p>
        <p className="habit-row__meta">
          {t('habitLastSeen', {
            date: formatIsoDate(habit.lastVisitedAt),
          })}
        </p>
      </div>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}
