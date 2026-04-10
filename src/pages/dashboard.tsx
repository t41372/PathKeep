import { useEffect, useState } from 'react'
import { useApp } from '../lib/app-context'
import { formatDateTime } from '../lib/format'
import { Glyph, StatusTag, Surface } from '../components/ui'
import { backend } from '../lib/backend'
import type {
  BackupReport,
  HealthReport,
  IntelligenceRuntimeSnapshot,
} from '../lib/types'

export function DashboardPage() {
  const {
    t,
    resolvedLanguage,
    snapshot,
    initialized,
    unlocked,
    archiveStatus,
    insightStatus,
    runTask,
    setNotice,
    setActivePage,
  } = useApp()

  const [lastBackupReport, setLastBackupReport] = useState<BackupReport | null>(
    null,
  )
  const [doctorReport, setDoctorReport] = useState<HealthReport | null>(null)
  const [runtimeSnapshot, setRuntimeSnapshot] =
    useState<IntelligenceRuntimeSnapshot | null>(null)

  // ------ derived health status ------
  const healthTone = !initialized
    ? 'neutral'
    : !unlocked
      ? 'danger'
      : archiveStatus.warning
        ? 'danger'
        : 'success'
  const healthLabel = !initialized
    ? t('needsSetup')
    : !unlocked
      ? t('archiveLocked')
      : archiveStatus.warning
        ? archiveStatus.warning
        : t('archiveHealthy')

  const lastBackupAt = formatDateTime(
    archiveStatus.lastSuccessfulBackupAt,
    resolvedLanguage,
  )

  const recentRuns = snapshot?.recentRuns.slice(0, 5) ?? []
  const profileCount = snapshot?.config.selectedProfileIds.length ?? 0

  useEffect(() => {
    if (!initialized || !unlocked) return

    let cancelled = false
    void (async () => {
      const next = await backend.loadIntelligenceRuntime()
      if (!cancelled) setRuntimeSnapshot(next)
    })()

    return () => {
      cancelled = true
    }
  }, [initialized, unlocked])

  // ------ handlers ------
  async function handleBackupNow() {
    await runTask(t('runBackupNow'), async () => {
      const report = await backend.runBackupNow()
      setLastBackupReport(report)
      if (initialized && unlocked) {
        setRuntimeSnapshot(await backend.loadIntelligenceRuntime())
      }
      if (report.dueSkipped) {
        setNotice(report.reason ?? t('backupSkipped'))
      } else {
        setNotice(t('backupCompleted'))
      }
    })
  }

  async function handleDoctor() {
    await runTask(t('runDoctor'), async () => {
      const report = await backend.doctor()
      setDoctorReport(report)
    })
  }

  return (
    <div className="pageContent">
      <section className="pageIntro">
        <p className="sectionEyebrow">{t('dashboardNav')}</p>
        <h2>{t('dashboardTitle')}</h2>
      </section>

      {/* Health badge */}
      <div className="dashboardGrid">
        <Surface
          eyebrow={t('systemHealth')}
          title={healthLabel}
          icon="monitor_heart"
        >
          <div className="healthBadge">
            <div className={`healthDot ${healthTone}`} />
            <span className="healthText">{healthLabel}</span>
          </div>
          {!initialized && (
            <button
              className="primaryButton"
              type="button"
              onClick={() => setActivePage('onboarding')}
            >
              <Glyph icon="rocket_launch" />
              {t('startSetup')}
            </button>
          )}
        </Surface>

        {/* Quick stats */}
        <Surface
          eyebrow={t('quickStats')}
          title={t('archiveOverview')}
          icon="analytics"
        >
          <div className="statGrid">
            <div className="statCard">
              <span className="statValue">{profileCount}</span>
              <span className="statLabel">{t('profiles')}</span>
            </div>
            <div className="statCard">
              <span className="statValue">
                {insightStatus.ready
                  ? `${Math.round(insightStatus.contentCoverage * 100)}%`
                  : '—'}
              </span>
              <span className="statLabel">{t('insightCoverage')}</span>
            </div>
            <div className="statCard">
              <span className="statValue">
                {runtimeSnapshot?.queue.queued ?? '—'}
              </span>
              <span className="statLabel">{t('queuedJobs')}</span>
            </div>
            <div className="statCard">
              <span className="statValue">
                {runtimeSnapshot?.queue.failed ?? '—'}
              </span>
              <span className="statLabel">{t('failedJobs')}</span>
            </div>
          </div>
        </Surface>

        {/* Last backup */}
        <Surface
          eyebrow={t('lastBackup')}
          title={lastBackupAt ?? t('noBackupYet')}
          icon="backup"
          actions={
            <button
              className="primaryButton"
              type="button"
              disabled={!initialized || !unlocked}
              onClick={handleBackupNow}
            >
              <Glyph icon="play_arrow" filled />
              {t('runBackupNow')}
            </button>
          }
        >
          {lastBackupReport && (
            <div className="backupReportSummary">
              {lastBackupReport.dueSkipped ? (
                <StatusTag tone="neutral">
                  {lastBackupReport.reason ?? t('backupSkipped')}
                </StatusTag>
              ) : (
                <StatusTag tone="success">{t('backupCompleted')}</StatusTag>
              )}
              {lastBackupReport.profiles.map((profile) => (
                <p key={profile.profileId}>
                  {profile.profileId}: +{profile.newVisits} {t('visits')}
                </p>
              ))}
            </div>
          )}
        </Surface>

        {/* Quick actions */}
        <Surface eyebrow={t('quickActions')} title={t('tools')} icon="build">
          <div className="actionButtonGroup">
            <button
              className="secondaryButton"
              type="button"
              onClick={handleDoctor}
            >
              <Glyph icon="health_and_safety" />
              {t('runDoctor')}
            </button>
            <button
              className="secondaryButton"
              type="button"
              disabled={!initialized}
              onClick={() => setActivePage('explorer')}
            >
              <Glyph icon="search" />
              {t('explorerNav')}
            </button>
            <button
              className="secondaryButton"
              type="button"
              onClick={() => setActivePage('settings')}
            >
              <Glyph icon="settings" />
              {t('settingsNav')}
            </button>
          </div>
        </Surface>
      </div>

      {/* Doctor report */}
      {doctorReport && (
        <Surface
          eyebrow={t('healthReport')}
          title={t('doctorResults')}
          icon="health_and_safety"
        >
          {doctorReport.checks.length === 0 ? (
            <p className="muted">{t('noHealthChecks')}</p>
          ) : (
            <div className="checkList">
              {doctorReport.checks.map((check) => (
                <div className="checkRow" key={check.name}>
                  <StatusTag
                    tone={
                      check.status === 'pass'
                        ? 'success'
                        : check.status === 'warn'
                          ? 'info'
                          : 'danger'
                    }
                  >
                    {check.status}
                  </StatusTag>
                  <strong>{check.name}</strong>
                  <span>{check.message}</span>
                </div>
              ))}
            </div>
          )}
        </Surface>
      )}

      {/* Recent runs */}
      {recentRuns.length > 0 && (
        <Surface
          eyebrow={t('recentActivity')}
          title={t('recentRuns')}
          icon="history"
        >
          <div className="runList">
            {recentRuns.map((run) => (
              <div className="runCard" key={run.id}>
                <div className="runHeader">
                  <StatusTag
                    tone={run.status === 'finished' ? 'success' : 'info'}
                  >
                    {run.status}
                  </StatusTag>
                  <span className="runDate">
                    {formatDateTime(run.startedAt, resolvedLanguage)}
                  </span>
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
              </div>
            ))}
          </div>
        </Surface>
      )}
    </div>
  )
}
