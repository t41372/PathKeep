/**
 * General settings section: language, paths, diagnostics, version.
 *
 * Why this file exists:
 * - Extracted from the monolithic Settings route to keep each panel's
 *   presentational contract explicit and independently reviewable.
 * - General settings are low-stakes informational rows, so they get the
 *   lightest visual treatment.
 *
 * Main declarations:
 * - `GeneralSection`
 */

import {
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { formatDateTime } from '../../lib/format'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import type { AppBuildInfo, AppSnapshot } from '../../lib/types'

export interface GeneralSectionProps {
  buildInfo: AppBuildInfo | null
  saving: boolean
  snapshot: AppSnapshot
  supportCopyFeedback: ReviewCopyFeedback | null
  onCopyReviewValue: (
    value: string,
    opts: { key: string; onFeedback: (fb: ReviewCopyFeedback | null) => void },
  ) => Promise<void>
  onLanguageChange: (language: string) => Promise<void>
}

export function GeneralSection({
  buildInfo,
  saving,
  snapshot,
  supportCopyFeedback,
  onCopyReviewValue,
  onLanguageChange,
}: GeneralSectionProps) {
  const { language, t } = useI18n()
  const buildRevision = formatBuildRevisionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)

  return (
    <div className="panel" id="settings-general">
      <div className="panel-header">
        <span className="panel-title">{t('settings.general')}</span>
      </div>
      <div className="panel-body">
        <p className="dashboard-next-action">
          {t('settings.generalDescription')}
        </p>
        <div className="config-row">
          <span className="config-label">
            {t('settings.interfaceLanguage')}
          </span>
          <select
            aria-label={t('settings.interfaceLanguage')}
            className="settings-select"
            disabled={saving}
            value={snapshot.config.preferredLanguage}
            onChange={(event) => {
              void onLanguageChange(event.target.value)
            }}
          >
            <option value="system">{t('common.followSystem')}</option>
            {supportedLanguages.map((entry) => (
              <option key={entry} value={entry}>
                {languageLabel(entry, language)}
              </option>
            ))}
          </select>
        </div>
        <div className="config-row">
          <span className="config-label">{t('settings.currentLanguage')}</span>
          <span className="config-value">
            {languageLabel(language, language)}
          </span>
        </div>
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:app-root"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.dataDirectory')}
          onCopy={(key, value) => {
            void onCopyReviewValue(value, {
              key,
              onFeedback: () => {},
            })
          }}
          onOpenPath={(path) => {
            void backend.openPathInFileManager(path)
          }}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.appRoot}
        />
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:archive-database"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.archiveDatabase')}
          onCopy={(key, value) => {
            void onCopyReviewValue(value, {
              key,
              onFeedback: () => {},
            })
          }}
          onOpenPath={(path) => {
            void backend.openPathInFileManager(path)
          }}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.archiveDatabasePath}
        />
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:audit-repo"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.auditRepository')}
          onCopy={(key, value) => {
            void onCopyReviewValue(value, {
              key,
              onFeedback: () => {},
            })
          }}
          onOpenPath={(path) => {
            void backend.openPathInFileManager(path)
          }}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.auditRepoPath}
        />
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:logs-dir"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.logsDirectory')}
          onCopy={(key, value) => {
            void onCopyReviewValue(value, {
              key,
              onFeedback: () => {},
            })
          }}
          onOpenPath={(path) => {
            void backend.openPathInFileManager(path)
          }}
          openPathLabel={t('settings.openDirectory')}
          successMessage={t('common.copiedNotice')}
          value={snapshot.directories.logsDir}
        />
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:crash-reports"
          copyLabel={t('common.copyAction')}
          errorMessage={t('audit.copyFailed')}
          label={t('settings.crashReports')}
          onCopy={(key, value) => {
            void onCopyReviewValue(value, {
              key,
              onFeedback: () => {},
            })
          }}
          onOpenPath={(path) => {
            void backend.openPathInFileManager(path)
          }}
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
                  void backend.openPathInFileManager(
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
