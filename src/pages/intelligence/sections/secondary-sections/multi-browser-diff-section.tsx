/**
 * @file multi-browser-diff-section.tsx
 * @description 承接 `/intelligence` 的多瀏覽器差異卡片，讓 route-level secondary grid 不必再內嵌 archive-wide 對比細節。
 * @module intelligence/secondary-sections
 *
 * ## 職責
 * - 讀取 archive-wide multi-browser diff 資料與快取。
 * - 渲染 profile 概覽、shared domains、exclusive domains 與 category distribution。
 * - 保持既有的 section meta、空態、loading 與 shared domain deep-link 行為。
 *
 * ## 不負責
 * - 不決定 secondary grid 的排序、顯示條件或 layout composition。
 * - 不抽共享 heuristics；只保留這張卡片專用的局部 helper。
 *
 * ## 依賴關係
 * - 依賴 `lib/core-intelligence/api` 取得 multi-browser diff 與快取。
 * - 依賴 `IntelligenceSectionMeta` 與 `IntelligenceSectionBody` 維持 `/intelligence` 共用 chrome。
 * - 依賴 `intelligenceText` 保持 archive-wide badge 文案與 i18n 契約一致。
 *
 * ## 性能備注
 * - 卡片只渲染已經過 backend 彙整後的 top-N diff 內容，不在前端做額外的大規模重算。
 */

import { Link } from 'react-router-dom'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type BrowserDiff,
  type BrowserProfileSummary,
  type CategoryMixEntry,
  type DateRange,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../../lib/i18n'
import { intelligenceText } from '../../copy'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'

type MultiBrowserDiffSectionProps = {
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  scopeLabel: string
  t: T
}

/**
 * 為 `/intelligence` 提供 archive-wide 的跨瀏覽器差異卡，讓使用者能快速看到哪些 domain 與分類只出現在特定 profile。
 *
 * `domainHref` 必須保持既有 deep-link grammar，因為 shared/exclusive domain CTA 會直接沿用它。
 * 當 diff 尚未就緒或 profile 少於兩個時，這裡故意維持原本的空態，而不是嘗試推測部分結果。
 */
export function MultiBrowserDiffSection({
  dateRange,
  domainHref,
  language,
  scopeLabel,
  t,
}: MultiBrowserDiffSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getMultiBrowserDiff(dateRange),
    [dateRange],
    {
      getCached: () => api.peekMultiBrowserDiff(dateRange),
    },
  )
  const diff = data?.data ?? null

  return (
    <section className="intelligence-section multi-browser-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('multiBrowserTitle')}
        </h2>
        <span className="status-badge status-info">
          {intelligenceText(language, t, 'archiveWideBadge')}
        </span>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !diff || diff.profiles.length < 2 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('multiBrowserEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <MultiBrowserDiffBody data={diff} domainHref={domainHref} t={t} />
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function MultiBrowserDiffBody({
  data,
  domainHref,
  t,
}: {
  data: BrowserDiff
  domainHref: (domain: string) => string
  t: T
}) {
  const profileById = new Map<string, BrowserProfileSummary>(
    data.profiles.map((profile) => [profile.profileId, profile]),
  )
  const exclusiveByProfile = new Map<string, typeof data.exclusiveDomains>()
  for (const entry of data.exclusiveDomains) {
    const list = exclusiveByProfile.get(entry.profileId) ?? []
    list.push(entry)
    exclusiveByProfile.set(entry.profileId, list)
  }

  return (
    <div className="multi-browser">
      <div className="multi-browser__profiles">
        {data.profiles.map((profile) => (
          <div key={profile.profileId} className="multi-browser__profile">
            <span className="multi-browser__profile-name">
              {profile.profileName}
            </span>
            <span className="multi-browser__profile-family">
              {profile.browserFamily}
            </span>
            <span className="multi-browser__profile-stats">
              {t('multiBrowserVisits', { count: profile.visitCount })} ·{' '}
              {t('multiBrowserDomains', { count: profile.domainCount })}
            </span>
          </div>
        ))}
      </div>
      <div className="multi-browser__shared">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserShared', { count: data.sharedDomains.length })}
        </h3>
        <div className="multi-browser__shared-chips">
          {data.sharedDomains.slice(0, 10).map((domain) => (
            <Link
              key={domain}
              className="multi-browser__chip intelligence-link"
              to={domainHref(domain)}
            >
              {domain}
            </Link>
          ))}
        </div>
      </div>
      <div className="multi-browser__exclusive">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserExclusive')}
        </h3>
        <div className="multi-browser__exclusive-grid">
          {Array.from(exclusiveByProfile.entries()).map(
            ([profileId, entries]) => {
              const profile = profileById.get(profileId)
              return (
                <div
                  key={profileId}
                  className="multi-browser__exclusive-column"
                >
                  <span className="multi-browser__exclusive-header">
                    {profile?.profileName ?? profileId}
                  </span>
                  <ul className="multi-browser__exclusive-list">
                    {entries.slice(0, 5).map((entry) => (
                      <li
                        key={entry.registrableDomain}
                        className="multi-browser__exclusive-row"
                      >
                        <Link
                          className="intelligence-link"
                          to={domainHref(entry.registrableDomain)}
                        >
                          {entry.registrableDomain}
                        </Link>
                        <span className="multi-browser__exclusive-count">
                          {entry.visitCount}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )
            },
          )}
        </div>
      </div>
      <div className="multi-browser__categories">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserCategories')}
        </h3>
        <MultiBrowserCategoryBars
          distributions={data.categoryDistributions}
          t={t}
        />
      </div>
    </div>
  )
}

function MultiBrowserCategoryBars({
  distributions,
  t,
}: {
  distributions: BrowserDiff['categoryDistributions']
  t: T
}) {
  const allCategories = new Set<string>()
  for (const distribution of distributions) {
    for (const category of distribution.categories) {
      allCategories.add(category.domainCategory)
    }
  }
  const categoryList = Array.from(allCategories)

  return (
    <div className="multi-browser__category-bars">
      {categoryList.map((category) => (
        <div key={category} className="multi-browser__category-row">
          <span className="multi-browser__category-label">
            {t(`category_${category}`) || category}
          </span>
          <div className="multi-browser__category-profiles">
            {distributions.map((distribution) => {
              const entry: CategoryMixEntry | undefined =
                distribution.categories.find(
                  (item) => item.domainCategory === category,
                )
              const share = entry ? Math.round(entry.share * 100) : 0
              return (
                <div
                  key={distribution.profileId}
                  className="multi-browser__category-bar"
                  title={`${distribution.profileName}: ${share}%`}
                >
                  <span
                    className="multi-browser__category-bar-fill"
                    style={{ width: `${share}%` }}
                  />
                  <span className="multi-browser__category-bar-meta">
                    {distribution.profileName} {share}%
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}
