import { useState } from 'react'
import { useApp } from '../lib/app-context'
import { formatDateTime } from '../lib/format'
import { EmptyState, Glyph, StatusTag, Surface } from '../components/ui'
import { backend } from '../lib/backend'
import type { BackupReport, RemoteBackupPreview } from '../lib/types'

export function ActivityLogPage() {
  const {
    t,
    resolvedLanguage,
    snapshot,
    initialized,
    unlocked,
    runTask,
    setNotice,
  } = useApp()

  const [lastBackupReport, setLastBackupReport] = useState<BackupReport | null>(
    null,
  )
  const [remotePreview, setRemotePreview] =
    useState<RemoteBackupPreview | null>(null)
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null)

  const recentRuns = snapshot?.recentRuns ?? []
  const activeSelectedRunId = recentRuns.some((run) => run.id === selectedRunId)
    ? selectedRunId
    : (recentRuns[0]?.id ?? null)

  const selectedRun =
    recentRuns.find((run) => run.id === activeSelectedRunId) ?? null

  async function handleBackupRun() {
    await runTask(t('runBackupNow'), async () => {
      const report = await backend.runBackupNow(false)
      setLastBackupReport(report)
      await backend.getAppSnapshot()
      // Refresh snapshot through context
      if (report.dueSkipped) {
        setNotice(report.reason ?? t('backupComplete'))
        return
      }
      const baseNotice = report.manifestPath
        ? t('backupCompleteWithManifest', { path: report.manifestPath })
        : t('backupComplete')
      const nextNotice = report.remoteBackup
        ? `${baseNotice} ${report.remoteBackup.message}`
        : baseNotice
      setNotice(nextNotice)
    })
  }

  async function handlePreviewRemoteBackup() {
    await runTask(t('previewUpload'), async () => {
      const preview = await backend.previewRemoteBackup()
      setRemotePreview(preview)
      setNotice(t('s3PreviewReady'))
    })
  }

  async function handleRunRemoteBackup() {
    await runTask(t('uploadNow'), async () => {
      const result = await backend.runRemoteBackup()
      setNotice(result.message)
    })
  }

  return (
    <div className="pageContent">
      <section className="pageIntro">
        <p className="sectionEyebrow">{t('activityNav')}</p>
        <h2>{t('backupsDescription')}</h2>
        <div className="pathActions">
          <button
            className="primaryButton"
            type="button"
            disabled={!initialized || !unlocked}
            onClick={handleBackupRun}
          >
            <Glyph icon="play_arrow" filled />
            {t('runBackupNow')}
          </button>
        </div>
      </section>

      {/* Last backup result */}
      {lastBackupReport && (
        <Surface
          eyebrow={t('lastBackup')}
          title={
            lastBackupReport.dueSkipped
              ? t('backupSkipped')
              : t('backupCompleted')
          }
          icon="backup"
        >
          {lastBackupReport.profiles.map((profile) => (
            <div key={profile.profileId} className="profileRunSummary">
              <strong>{profile.profileId}</strong>
              <span>
                +{profile.newVisits} {t('visits')}, +{profile.newUrls}{' '}
                {t('urls')}
              </span>
            </div>
          ))}
          {lastBackupReport.warnings.map((w, i) => (
            <StatusTag key={i} tone="info">
              {w}
            </StatusTag>
          ))}
        </Surface>
      )}

      {/* Recent runs */}
      {recentRuns.length > 0 ? (
        <Surface
          eyebrow={t('recentRuns')}
          title={t('recentRuns')}
          icon="history"
        >
          <div className="runList">
            {recentRuns.map((run) => (
              <button
                key={run.id}
                className={`runCard ${activeSelectedRunId === run.id ? 'selected' : ''}`}
                type="button"
                onClick={() => setSelectedRunId(run.id)}
              >
                <div className="runHeader">
                  <StatusTag
                    tone={run.status === 'finished' ? 'success' : 'info'}
                  >
                    {run.status}
                  </StatusTag>
                  <span>{formatDateTime(run.startedAt, resolvedLanguage)}</span>
                </div>
                <div className="runStats">
                  <span>
                    +{run.newVisits} {t('visits')}
                  </span>
                  <span>
                    +{run.newUrls} {t('urls')}
                  </span>
                  <span>
                    {run.profilesProcessed} {t('profiles')}
                  </span>
                </div>
              </button>
            ))}
          </div>

          {/* Selected run detail */}
          {selectedRun && (
            <div className="runDetail">
              <div className="runDetailRow">
                <span className="fieldLabel">{t('statusLabel')}</span>
                <span>{selectedRun.status}</span>
              </div>
              <div className="runDetailRow">
                <span className="fieldLabel">{t('startedAt')}</span>
                <span>
                  {formatDateTime(selectedRun.startedAt, resolvedLanguage)}
                </span>
              </div>
              {selectedRun.finishedAt && (
                <div className="runDetailRow">
                  <span className="fieldLabel">{t('finishedAt')}</span>
                  <span>
                    {formatDateTime(selectedRun.finishedAt, resolvedLanguage)}
                  </span>
                </div>
              )}
              {selectedRun.manifestHash && (
                <div className="runDetailRow">
                  <span className="fieldLabel">{t('manifestHash')}</span>
                  <code>{selectedRun.manifestHash}</code>
                </div>
              )}
            </div>
          )}
        </Surface>
      ) : (
        <EmptyState icon="history" message={t('noBackupYet')} />
      )}

      {/* Remote backup */}
      {snapshot?.config.remoteBackup.enabled && (
        <Surface
          eyebrow={t('remoteBackupTitle')}
          title={t('remoteBackupTitle')}
          icon="cloud_upload"
        >
          <div className="pathActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handlePreviewRemoteBackup}
            >
              <Glyph icon="preview" />
              {t('previewUpload')}
            </button>
            <button
              className="primaryButton"
              type="button"
              onClick={handleRunRemoteBackup}
            >
              <Glyph icon="cloud_upload" />
              {t('uploadNow')}
            </button>
          </div>
          {remotePreview && (
            <div className="remotePreviewDetail">
              <div className="detailField">
                <span className="fieldLabel">{t('objectKeyLabel')}</span>
                <code>{remotePreview.objectKey}</code>
              </div>
              <div className="detailField">
                <span className="fieldLabel">{t('uploadUrlLabel')}</span>
                <code className="codeWrap">{remotePreview.uploadUrl}</code>
              </div>
            </div>
          )}
        </Surface>
      )}
    </div>
  )
}
