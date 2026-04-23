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
import { Glyph } from '../../components/ui'
import {
  explorerBackgroundPrefetchPageOptions,
  normalizeExplorerBackgroundPrefetchPages,
} from '../../lib/explorer-preferences'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { formatDateTime } from '../../lib/format'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import type { AppBuildInfo, AppSnapshot } from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Props for the extracted general Settings section.
 *
 * The route still owns mutations and side effects; this component only renders
 * the panel and forwards section-local interactions back to the route.
 */
export interface GeneralSectionProps {
  buildInfo: AppBuildInfo | null
  explorerBackgroundPrefetchPages: number
  navItem: SettingsSectionNavItem
  onCopyPath: (key: string, value: string) => Promise<void>
  onExplorerBackgroundPrefetchPagesChange: (pages: number) => Promise<void>
  saving: boolean
  snapshot: AppSnapshot
  supportCopyFeedback: ReviewCopyFeedback | null
  onLanguageChange: (language: string) => Promise<void>
  onOpenPath: (path: string) => void
}

/**
 * Renders the general Settings panel from route-owned state and callbacks.
 *
 * This keeps diagnostics rows, language controls, and build metadata in a
 * dedicated render module while the route retains the actual mutation logic.
 */
export function GeneralSection({
  buildInfo,
  explorerBackgroundPrefetchPages,
  navItem,
  onCopyPath,
  onExplorerBackgroundPrefetchPagesChange,
  saving,
  snapshot,
  supportCopyFeedback,
  onLanguageChange,
  onOpenPath,
}: GeneralSectionProps) {
  const { language, t } = useI18n()
  const buildRevision = formatBuildRevisionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)

  return (
    <div className="panel" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
      </div>
      <div className="panel-body panel-body--compact">
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
        <div className="config-row">
          <span className="config-label">
            {t('settings.explorerBackgroundPrefetchPages')}
          </span>
          <select
            aria-label={t('settings.explorerBackgroundPrefetchPages')}
            className="settings-select"
            disabled={saving}
            value={normalizeExplorerBackgroundPrefetchPages(
              explorerBackgroundPrefetchPages,
            )}
            onChange={(event) => {
              void onExplorerBackgroundPrefetchPagesChange(
                Number.parseInt(event.target.value, 10),
              )
            }}
          >
            {explorerBackgroundPrefetchPageOptions.map((option) => (
              <option key={option} value={option}>
                {option === 0
                  ? t('settings.explorerBackgroundPrefetchDisabled')
                  : t('settings.explorerBackgroundPrefetchOption', {
                      count: option,
                    })}
              </option>
            ))}
          </select>
        </div>
        <p className="dashboard-next-action">
          {t('settings.explorerBackgroundPrefetchBody')}
        </p>
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:app-root"
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
          copyFeedback={supportCopyFeedback}
          copyKey="settings:archive-database"
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
          copyFeedback={supportCopyFeedback}
          copyKey="settings:audit-repo"
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
          copyFeedback={supportCopyFeedback}
          copyKey="settings:logs-dir"
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
        <ReviewPathActionRow
          copyFeedback={supportCopyFeedback}
          copyKey="settings:crash-reports"
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
