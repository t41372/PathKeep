/**
 * @file observed-interactions-section.tsx
 * @description 承接 `/intelligence` 的 observed interactions 卡片，集中呈現 capability-gated 的前景停留、滾動與鍵擊觀察。
 * @module intelligence/secondary-sections
 *
 * ## 職責
 * - 讀取 observed interactions 資料與快取。
 * - 保持 capability badge、disclaimer、empty/loading state 與 top-10 observation row 呈現。
 * - 將 interaction metrics 的格式化限制在這張卡片內，避免 route shell 承擔細節組裝。
 *
 * ## 不負責
 * - 不決定 capability 觀察是否啟用；只誠實渲染 backend 已產出的結果。
 * - 不共享 interaction row helper；目前只服務這張卡片。
 *
 * ## 依賴關係
 * - 依賴 `lib/core-intelligence/api` 取得 observed interactions 與快取。
 * - 依賴 `IntelligenceSectionMeta` 與 `IntelligenceSectionBody` 維持 shared section chrome。
 * - 依賴 `formatDuration` 保持 duration 文案格式與其他 intelligence surfaces 一致。
 *
 * ## 性能備注
 * - 只渲染前十筆 observation，避免 capability 資料在前端無上限展開。
 */

import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type ObservedInteraction,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import { formatDuration, type T } from '../shared'

type ObservedInteractionsSectionProps = {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * 為 `/intelligence` 提供 capability-aware 的 observed interactions 卡片，讓使用者看到實際觀察到的前景停留、滾動與鍵擊訊號。
 *
 * `profileId` 會直接決定 scope，因此這裡只讀既有 query contract，不自行做跨 profile 合併。
 * 當沒有 observation 時，保留原本的誠實 empty state，而不是用 capability badge 假裝已有資料。
 */
export function ObservedInteractionsSection({
  dateRange,
  profileId,
  scopeLabel,
  t,
}: ObservedInteractionsSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getObservedInteractions(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekObservedInteractions(dateRange, profileId),
    },
  )
  const observations = data?.data ?? []

  return (
    <section className="intelligence-section observed-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('observedTitle')}</h2>
        <span className="observed-section__badge">
          {t('observedCapabilityBadge')}
        </span>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      <p className="observed-section__disclaimer">{t('observedDisclaimer')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : observations.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('observedEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <ul className="observed-list">
            {observations.slice(0, 10).map((item) => (
              <ObservedInteractionRow key={item.visitId} item={item} t={t} />
            ))}
          </ul>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function ObservedInteractionRow({
  item,
  t,
}: {
  item: ObservedInteraction
  t: T
}) {
  const foreground =
    item.foregroundDurationMs != null
      ? formatDuration(item.foregroundDurationMs)
      : null
  const scroll =
    item.scrollingTimeMs != null ? formatDuration(item.scrollingTimeMs) : null

  return (
    <li className="observed-row">
      <div className="observed-row__main">
        <span className="observed-row__title" title={item.url}>
          {item.title ?? item.url}
        </span>
        <span className="observed-row__family">{item.browserFamily}</span>
      </div>
      <div className="observed-row__metrics">
        {foreground ? (
          <span className="observed-row__metric">
            {t('observedForeground', { duration: foreground })}
          </span>
        ) : null}
        {scroll ? (
          <span className="observed-row__metric">
            {t('observedScroll', { duration: scroll })}
          </span>
        ) : null}
        {item.keyPresses != null && item.keyPresses > 0 ? (
          <span className="observed-row__metric">
            {t('observedKeyPresses', { count: item.keyPresses })}
          </span>
        ) : null}
        {item.loadSuccessful === false ? (
          <span className="observed-row__metric observed-row__metric--warn">
            {t('observedLoadFailed')}
          </span>
        ) : null}
      </div>
    </li>
  )
}
