import '../../../components/intelligence/storage-analytics.css'

import { Link } from 'react-router-dom'
import { formatBytes } from '../../../lib/format'
import { useI18n } from '../../../lib/i18n/hooks'
import { storageGrowthEvidence } from '../../../lib/storage-analytics'
import type { DashboardSnapshot } from '../../../lib/types'
import {
  commonHealthText,
  intelligenceText,
  type CommonHealthTextKey,
} from '../copy'
import { IntelligenceSectionBody } from './section-body'

function StorageBreakdownList({
  items,
  language,
}: {
  items: Array<{ id: CommonHealthTextKey; bytes: number }>
  language: 'en' | 'zh-CN' | 'zh-TW'
}) {
  const { ns } = useI18n()
  const commonT = ns('common')

  return (
    <div className="storage-breakdown-list">
      {items.map((item) => (
        <div key={item.id} className="storage-breakdown-list__row">
          <span>{commonHealthText(language, commonT, item.id)}</span>
          <span className="mono">{formatBytes(item.bytes, language)}</span>
        </div>
      ))}
    </div>
  )
}

export function StorageAnalyticsSection({
  dashboard,
}: {
  dashboard: DashboardSnapshot | null
}) {
  const { language, ns } = useI18n()
  const t = ns('intelligence')
  const commonT = ns('common')
  const growth = storageGrowthEvidence(dashboard)

  if (!dashboard) {
    return (
      <section className="intelligence-section">
        <h2 className="intelligence-section__title">
          {intelligenceText(language, t, 'storageAnalytics')}
        </h2>
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {intelligenceText(language, t, 'noGrowthEvidenceDescription')}
          </p>
        </div>
      </section>
    )
  }

  return (
    <section className="intelligence-section">
      <h2 className="intelligence-section__title">
        {intelligenceText(language, t, 'storageAnalytics')}
      </h2>
      <IntelligenceSectionBody className="storage-overview">
        <p className="intelligence-empty__text">
          {intelligenceText(language, t, 'storageAnalyticsDescription')}
        </p>
        <div className="storage-overview-grid">
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {intelligenceText(language, t, 'trackedStorage')}
            </span>
            <strong className="storage-overview-stat__value">
              {formatBytes(growth.trackedStorageBytes, language)}
            </strong>
          </div>
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {intelligenceText(language, t, 'reclaimableSpace')}
            </span>
            <strong className="storage-overview-stat__value">
              {formatBytes(growth.reclaimableBytes, language)}
            </strong>
          </div>
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {intelligenceText(language, t, 'dominantStorage')}
            </span>
            <strong className="storage-overview-stat__value">
              {commonHealthText(language, commonT, growth.dominantGroup.id)}
            </strong>
          </div>
        </div>
        <div className="storage-overview-groups">
          <div className="storage-overview-group">
            <div className="storage-overview-group__header">
              <span>{commonHealthText(language, commonT, 'coreHistory')}</span>
              <span className="mono">
                {formatBytes(growth.coreHistoryBytes, language)}
              </span>
            </div>
            <StorageBreakdownList
              items={growth.summary.coreBreakdown}
              language={language}
            />
          </div>
          <div className="storage-overview-group">
            <div className="storage-overview-group__header">
              <span>{commonHealthText(language, commonT, 'otherData')}</span>
              <span className="mono">
                {formatBytes(growth.otherDataBytes, language)}
              </span>
            </div>
            <StorageBreakdownList
              items={growth.summary.otherBreakdown}
              language={language}
            />
          </div>
        </div>
      </IntelligenceSectionBody>
    </section>
  )
}

export function GrowthSignalSection({
  dashboard,
}: {
  dashboard: DashboardSnapshot | null
}) {
  const { language, ns } = useI18n()
  const t = ns('intelligence')
  const growth = storageGrowthEvidence(dashboard)

  return (
    <section className="intelligence-section">
      <h2 className="intelligence-section__title">
        {intelligenceText(language, t, 'growthSignal')}
      </h2>
      <IntelligenceSectionBody className="storage-growth-card">
        {!growth.latestRunId ? (
          <div className="intelligence-empty">
            <p className="intelligence-empty__text">
              {intelligenceText(language, t, 'noGrowthEvidenceDescription')}
            </p>
          </div>
        ) : (
          <>
            <p className="storage-growth-card__body">
              {intelligenceText(language, t, 'latestRunGrowthBody', {
                visits: growth.latestVisitGrowth,
                urls: growth.latestUrlGrowth,
                downloads: growth.latestDownloadGrowth,
              })}
            </p>
            <Link
              className="btn-secondary"
              to={`/audit?run=${growth.latestRunId}`}
            >
              {intelligenceText(language, t, 'openGrowthAuditRun')}
            </Link>
          </>
        )}
      </IntelligenceSectionBody>
    </section>
  )
}
