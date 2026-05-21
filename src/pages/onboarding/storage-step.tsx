/**
 * @file storage-step.tsx
 * @description Renders the onboarding storage overview and archive-size estimate step.
 * @module pages/onboarding
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'

export interface StorageEstimateSnapshot {
  archiveDbBytes: number
  manifestBytes: number
  profileCount: number
  snapshotsBytes: number
  sourceBytes: number
  totalBytes: number
}

export interface StorageStepProps {
  appRoot: string
  onBack: () => void
  onContinue: () => void
  storageEstimate: StorageEstimateSnapshot
}

export function StorageStep({
  appRoot,
  onBack,
  onContinue,
  storageEstimate,
}: StorageStepProps) {
  const { language, t } = useI18n('onboarding')

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('storageTitle')}</h2>
        <p className="ob-desc">{t('storageDesc')}</p>
      </div>

      <div className="mt-4">
        <PaperCard testId="onboarding-storage-archive-root">
          <PaperCardHeader
            title={t('archiveRoot')}
            right={<PaperCardBadge>{t('localFirst')}</PaperCardBadge>}
          />
          <PaperCardBody>
            <div
              className="storage-path-display"
              style={{ marginBottom: 'var(--space-4)' }}
            >
              <span className="storage-path-field">{appRoot}</span>
            </div>

            <div className="dir-tree">
              <div className="dir-item">
                <span className="dir-icon">📁</span>
                <span>{appRoot}</span>
              </div>
              <div className="dir-item indent">
                <span className="dir-icon">🗄</span>
                <span>archive/history-vault.sqlite</span>
              </div>
              <div className="dir-item indent">
                <span className="dir-icon">📋</span>
                <span>audit/manifests/</span>
              </div>
              <div className="dir-item indent">
                <span className="dir-icon">📸</span>
                <span>raw-snapshots/</span>
              </div>
              <div className="dir-item indent">
                <span className="dir-icon">📤</span>
                <span>exports/</span>
              </div>
              <div className="dir-item indent">
                <span className="dir-icon">⚙</span>
                <span>config.json</span>
              </div>
            </div>
          </PaperCardBody>
        </PaperCard>
      </div>

      <div className="mt-4">
        <PaperCard testId="onboarding-storage-size-estimates">
          <PaperCardHeader
            title={t('sizeEstimates')}
            right={<PaperCardBadge>{t('projected')}</PaperCardBadge>}
          />
          <PaperCardBody>
            <div className="estimate-grid">
              <div className="estimate-item">
                <span className="estimate-label">{t('estimateArchiveDb')}</span>
                <span className="estimate-value mono">
                  {formatBytes(storageEstimate.archiveDbBytes, language)}
                </span>
              </div>
              <div className="estimate-item">
                <span className="estimate-label">{t('estimateManifest')}</span>
                <span className="estimate-value mono">
                  {formatBytes(storageEstimate.manifestBytes, language)}
                </span>
              </div>
              <div className="estimate-item">
                <span className="estimate-label">{t('estimateSnapshots')}</span>
                <span className="estimate-value mono">
                  {formatBytes(storageEstimate.snapshotsBytes, language)}
                </span>
              </div>
              <div className="estimate-item highlight">
                <span className="estimate-label">{t('estimateTotal')}</span>
                <span className="estimate-value mono">
                  {formatBytes(storageEstimate.totalBytes, language)}
                </span>
              </div>
            </div>
            <p className="mono-support" style={{ marginTop: 'var(--space-3)' }}>
              {t('estimateExplanation', {
                count: storageEstimate.profileCount,
                source: formatBytes(storageEstimate.sourceBytes, language),
              })}
            </p>
          </PaperCardBody>
        </PaperCard>
      </div>

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button className="btn-primary" type="button" onClick={onContinue}>
          {t('continueButton')}
        </button>
      </div>
    </div>
  )
}
