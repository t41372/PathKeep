/**
 * @file diagnostics-section.tsx
 * @description Renders maintenance-owned support diagnostics and local path review rows.
 * @module pages/maintenance
 *
 * ## Responsibilities
 * - Show local data, archive, audit, log, crash, build, and MCP posture in one bounded diagnostics panel.
 * - Delegate copy/open-path behavior to the route-owned support handlers.
 * - Keep support diagnostics out of the preference-only Settings route.
 *
 * ## Not responsible for
 * - Running repair actions or mutating schedule/security configuration.
 * - Rendering updater, retention, remote backup, or derived-state workflows.
 * - Owning backend support snapshot loading.
 *
 * ## Dependencies
 * - Uses shared review path rows and the Settings support state that already owns copy feedback.
 * - Reads build/runtime diagnostics from the shell snapshot.
 *
 * ## Performance notes
 * - The panel renders already-loaded shell data only; it does not perform additional diagnostics IO.
 */

import {
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type { AppBuildInfo, AppSnapshot } from '../../lib/types'

/**
 * Props for the maintenance diagnostics section.
 */
export interface DiagnosticsSectionProps {
  buildInfo: AppBuildInfo | null
  copyFeedback: ReviewCopyFeedback | null
  onCopyPath: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => void
  /** Optional callback to reveal the logs directory in the platform file manager. */
  onRevealLogs?: () => void
  snapshot: AppSnapshot
}

/**
 * Renders support and troubleshooting diagnostics from shell-owned data.
 *
 * Keeping this as a Maintenance panel makes Settings shorter while preserving
 * the release/support paths that users still need when something is wrong.
 */
export function DiagnosticsSection({
  buildInfo,
  copyFeedback,
  onCopyPath,
  onOpenPath,
  onRevealLogs,
  snapshot,
}: DiagnosticsSectionProps) {
  const { language, t } = useI18n()
  const buildRevision = formatBuildRevisionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)

  return (
    <div className="panel" id="maintenance-diagnostics">
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon="build" filled />
          <span>{t('settings.diagnosticsTitle')}</span>
        </span>
      </div>
      <div className="panel-body panel-body--compact">
        <p className="dashboard-next-action">{t('settings.diagnosticsBody')}</p>
        <ReviewPathActionRow
          copyFeedback={copyFeedback}
          copyKey="maintenance:app-root"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.dataDirectory')}
          onCopy={(key, value) => {
            void onCopyPath(key, value)
          }}
          onOpenPath={onOpenPath}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.appRoot}
        />
        <ReviewPathActionRow
          copyFeedback={copyFeedback}
          copyKey="maintenance:archive-database"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.archiveDatabase')}
          onCopy={(key, value) => {
            void onCopyPath(key, value)
          }}
          onOpenPath={onOpenPath}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.archiveDatabasePath}
        />
        <ReviewPathActionRow
          copyFeedback={copyFeedback}
          copyKey="maintenance:audit-repo"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.auditRepository')}
          onCopy={(key, value) => {
            void onCopyPath(key, value)
          }}
          onOpenPath={onOpenPath}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.auditRepoPath}
        />
        <ReviewPathActionRow
          copyFeedback={copyFeedback}
          copyKey="maintenance:logs-dir"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.logsDirectory')}
          onCopy={(key, value) => {
            void onCopyPath(key, value)
          }}
          onOpenPath={onOpenPath}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.logsDir}
        />
        {onRevealLogs ? (
          <div className="config-row">
            <button
              aria-label={t('settings.revealLogsAriaLabel')}
              className="btn-secondary"
              type="button"
              onClick={onRevealLogs}
            >
              {t('settings.revealLogsButton')}
            </button>
          </div>
        ) : null}
        <ReviewPathActionRow
          copyFeedback={copyFeedback}
          copyKey="maintenance:crash-reports"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.crashReports')}
          onCopy={(key, value) => {
            void onCopyPath(key, value)
          }}
          onOpenPath={onOpenPath}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.crashReportsDir}
        />
        {snapshot.runtimeDiagnostics.latestCrashReport ? (
          <StatusCallout
            tone="warning"
            title={t('settings.latestCrashTitle')}
            body={t('settings.latestCrashBody', {
              source:
                snapshot.runtimeDiagnostics.latestCrashReport.source ===
                'rust-panic'
                  ? t('settings.latestCrashSourceRust')
                  : t('settings.latestCrashSourceFrontend'),
              time:
                formatDateTime(
                  snapshot.runtimeDiagnostics.latestCrashReport.recordedAt,
                  language,
                ) ?? snapshot.runtimeDiagnostics.latestCrashReport.recordedAt,
              message: snapshot.runtimeDiagnostics.latestCrashReport.message,
            })}
            actions={
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  onOpenPath(
                    snapshot.runtimeDiagnostics.latestCrashReport?.path ??
                      snapshot.directories.crashReportsDir,
                  )
                }}
              >
                {t('settings.openCrashReport')}
              </button>
            }
          />
        ) : (
          <StatusCallout
            tone="info"
            title={t('settings.latestCrashClearTitle')}
            body={t('settings.latestCrashClearBody')}
          />
        )}
        <div className="config-row">
          <span className="config-label">{t('settings.mcpServer')}</span>
          <span className="config-value">
            {snapshot.config.ai.mcpEnabled
              ? t('settings.enabled')
              : t('settings.disabled')}
          </span>
        </div>
        <div className="config-row">
          <span className="config-label">{t('settings.version')}</span>
          <span className="config-value mono">
            {buildInfo?.version ?? t('common.notAvailable')}
          </span>
        </div>
        <div className="config-row">
          <span className="config-label">{t('settings.gitCommit')}</span>
          <span className="config-value mono" title={buildTitle ?? undefined}>
            {buildRevision ?? t('common.notAvailable')}
          </span>
        </div>
      </div>
    </div>
  )
}
