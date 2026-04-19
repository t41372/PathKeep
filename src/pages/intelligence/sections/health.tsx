import '../../../components/intelligence/storage-analytics.css'

import { Link } from 'react-router-dom'
import { useShellData } from '../../../app/shell-data-context'
import { formatBytes } from '../../../lib/format'
import { useI18n } from '../../../lib/i18n/hooks'
import { storageGrowthEvidence } from '../../../lib/storage-analytics'
import { IntelligenceSectionBody } from './section-body'

function StorageBreakdownList({
  items,
}: {
  items: Array<{ id: string; bytes: number }>
}) {
  const { language, ns } = useI18n()
  const commonT = ns('common')

  return (
    <div className="storage-breakdown-list">
      {items.map((item) => (
        <div key={item.id} className="storage-breakdown-list__row">
          <span>{commonT(item.id)}</span>
          <span className="mono">{formatBytes(item.bytes, language)}</span>
        </div>
      ))}
    </div>
  )
}

export function StorageAnalyticsSection() {
  const { dashboard } = useShellData()
  const { language, ns } = useI18n()
  const t = ns('intelligence')
  const commonT = ns('common')
  const growth = storageGrowthEvidence(dashboard)

  if (!dashboard) {
    return (
      <section className="intelligence-section">
        <h2 className="intelligence-section__title">{t('storageAnalytics')}</h2>
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {t('noGrowthEvidenceDescription')}
          </p>
        </div>
      </section>
    )
  }

  return (
    <section className="intelligence-section">
      <h2 className="intelligence-section__title">{t('storageAnalytics')}</h2>
      <IntelligenceSectionBody className="storage-overview">
        <p className="intelligence-empty__text">
          {t('storageAnalyticsDescription')}
        </p>
        <div className="storage-overview-grid">
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {t('trackedStorage')}
            </span>
            <strong className="storage-overview-stat__value">
              {formatBytes(growth.trackedStorageBytes, language)}
            </strong>
          </div>
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {t('reclaimableSpace')}
            </span>
            <strong className="storage-overview-stat__value">
              {formatBytes(growth.reclaimableBytes, language)}
            </strong>
          </div>
          <div className="storage-overview-stat">
            <span className="storage-overview-stat__label">
              {t('dominantStorage')}
            </span>
            <strong className="storage-overview-stat__value">
              {commonT(growth.dominantGroup.id)}
            </strong>
          </div>
        </div>
        <div className="storage-overview-groups">
          <div className="storage-overview-group">
            <div className="storage-overview-group__header">
              <span>{commonT('coreHistory')}</span>
              <span className="mono">
                {formatBytes(growth.coreHistoryBytes, language)}
              </span>
            </div>
            <StorageBreakdownList items={growth.summary.coreBreakdown} />
          </div>
          <div className="storage-overview-group">
            <div className="storage-overview-group__header">
              <span>{commonT('otherData')}</span>
              <span className="mono">
                {formatBytes(growth.otherDataBytes, language)}
              </span>
            </div>
            <StorageBreakdownList items={growth.summary.otherBreakdown} />
          </div>
        </div>
      </IntelligenceSectionBody>
    </section>
  )
}

export function GrowthSignalSection() {
  const { dashboard } = useShellData()
  const { ns } = useI18n()
  const t = ns('intelligence')
  const growth = storageGrowthEvidence(dashboard)

  return (
    <section className="intelligence-section">
      <h2 className="intelligence-section__title">{t('growthSignal')}</h2>
      <IntelligenceSectionBody className="storage-growth-card">
        {!growth.latestRunId ? (
          <div className="intelligence-empty">
            <p className="intelligence-empty__text">
              {t('noGrowthEvidenceDescription')}
            </p>
          </div>
        ) : (
          <>
            <p className="storage-growth-card__body">
              {t('latestRunGrowthBody', {
                visits: growth.latestVisitGrowth,
                urls: growth.latestUrlGrowth,
                downloads: growth.latestDownloadGrowth,
              })}
            </p>
            <Link
              className="btn-secondary"
              to={`/audit?run=${growth.latestRunId}`}
            >
              {t('openGrowthAuditRun')}
            </Link>
          </>
        )}
      </IntelligenceSectionBody>
    </section>
  )
}
