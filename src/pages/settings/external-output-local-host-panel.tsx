/**
 * Trusted local-host subsection for Core Intelligence external outputs.
 *
 * Why this file exists:
 * - `SettingsExternalOutputsPanel` already owns the manual review/copy-export
 *   baseline, but the first reusable host flow needs its own focused component
 *   so the Settings route does not become another unreadable mega-file.
 * - This subsection keeps Preview / Manual / Execute / Verify in one place
 *   while reusing the same shared profile scope and local time window as the
 *   manual output tabs above it.
 *
 * Main declarations:
 * - `SettingsExternalOutputLocalHostPanel`
 *
 * Source-of-truth notes:
 * - Keep the trusted-local-only boundary aligned with
 *   `docs/features/core-intelligence-ultimate-design.md`.
 * - Keep the PME grammar aligned with `docs/design/ux-principles.md`.
 */

import { useEffect, useMemo, useState } from 'react'
import {
  copyReviewValue,
  GeneratedArtifactViewer,
  ReviewCopyStatus,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../components/review'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { formatDateTime } from '../../lib/format'
import {
  buildIntelligenceLocalHost,
  previewIntelligenceLocalHost,
  useAsyncData,
  type DateRange,
  type IntelligenceInstalledLocalHost,
} from '../../lib/core-intelligence'
import { useI18n } from '../../lib/i18n/hooks'
import { profileIdLabel } from '../../lib/profile-scope-context'
import { localizeIntelligenceLocalHostPreview } from './helpers'

interface SettingsExternalOutputLocalHostPanelProps {
  activeProfileId: string | null
  dateRange: DateRange
  ready: boolean
}

function toFileUrl(path: string) {
  const normalized = path.replace(/\\/g, '/')
  if (normalized.startsWith('/')) {
    return `file://${encodeURI(normalized)}`
  }
  return `file:///${encodeURI(normalized)}`
}

/**
 * Renders the first reusable trusted local host flow for external outputs.
 */
export function SettingsExternalOutputLocalHostPanel({
  activeProfileId,
  dateRange,
  ready,
}: SettingsExternalOutputLocalHostPanelProps) {
  const { language, ns } = useI18n()
  const t = ns('settings')
  const commonT = ns('common')
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )
  const [buildState, setBuildState] = useState<{
    tone: 'success' | 'error'
    message: string
  } | null>(null)
  const [lastBuiltHost, setLastBuiltHost] =
    useState<IntelligenceInstalledLocalHost | null>(null)
  const [building, setBuilding] = useState(false)

  const preview = useAsyncData(async () => {
    if (!ready) {
      return null
    }
    return previewIntelligenceLocalHost(dateRange, language, activeProfileId)
  }, [ready, dateRange, language, activeProfileId])

  useEffect(() => {
    setCopyFeedback(null)
    setBuildState(null)
    setLastBuiltHost(null)
  }, [dateRange.start, dateRange.end, language, activeProfileId])

  const currentPreview = useMemo(
    () =>
      preview.data
        ? localizeIntelligenceLocalHostPreview(preview.data, t)
        : null,
    [preview.data, t],
  )
  const generatedFiles = currentPreview?.generatedFiles ?? []
  const installedHost = currentPreview?.installedHost ?? lastBuiltHost ?? null
  const scopeValue = activeProfileId
    ? profileIdLabel(activeProfileId)
    : t('externalOutputsArchiveWideTitle')

  async function handleCopy(key: string, value: string) {
    await copyReviewValue(value, {
      key,
      onFeedback: setCopyFeedback,
    })
  }

  async function handleBuild() {
    setBuilding(true)
    setBuildState(null)
    try {
      const result = await buildIntelligenceLocalHost(
        dateRange,
        language,
        activeProfileId,
      )
      setLastBuiltHost(result.installedHost ?? null)
      setBuildState({
        tone: 'success',
        message: t('externalOutputsLocalHostBuilt'),
      })
      preview.refresh()
    } catch (error) {
      setBuildState({
        tone: 'error',
        message: describeError(error, 'build_intelligence_local_host'),
      })
    } finally {
      setBuilding(false)
    }
  }

  const manualSteps = useMemo(
    () => currentPreview?.manualSteps ?? [],
    [currentPreview?.manualSteps],
  )

  if (!ready) {
    return null
  }

  return (
    <>
      <StatusCallout
        tone="info"
        title={t('externalOutputsLocalHostSummaryTitle')}
        body={t('externalOutputsLocalHostSummaryBody')}
      />

      {preview.loading && !currentPreview ? (
        <LoadingState label={t('externalOutputsLocalHostLoading')} />
      ) : preview.error || !currentPreview ? (
        <StatusCallout
          tone="warning"
          title={t('externalOutputsLocalHostUnavailableTitle')}
          body={preview.error ?? t('externalOutputsLocalHostUnavailableBody')}
          actions={
            <button
              className="btn-secondary"
              type="button"
              onClick={() => preview.refresh()}
            >
              {commonT('refreshAction')}
            </button>
          }
        />
      ) : (
        <>
          <ReviewSection
            headerMeta={
              <span className="panel-badge">
                {t('externalOutputsLocalHostBadge')}
              </span>
            }
            title={t('externalOutputsLocalHostPreviewTitle')}
          >
            <p>
              {t('externalOutputsLocalHostPreviewBody', {
                path: currentPreview.artifactRoot,
              })}
            </p>
            <div className="settings-field-grid">
              <div className="config-row">
                <span className="config-label">
                  {t('externalOutputsLocalHostScopeLabel')}
                </span>
                <span className="config-value mono">{scopeValue}</span>
              </div>
              <div className="config-row">
                <span className="config-label">
                  {t('externalOutputsLocalHostWindowLabel')}
                </span>
                <span className="config-value mono">
                  {dateRange.start} → {dateRange.end}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">
                  {t('externalOutputsLocalHostArtifactRootLabel')}
                </span>
                <span className="config-value mono">
                  {currentPreview.artifactRoot}
                </span>
              </div>
            </div>
          </ReviewSection>

          <ReviewSection title={t('externalOutputsLocalHostBoundaryTitle')}>
            <div className="inline-note-list">
              {currentPreview.boundaryNotes.map((note) => (
                <p key={note}>{note}</p>
              ))}
            </div>
          </ReviewSection>

          {currentPreview.warnings.length > 0 ? (
            <ReviewSection title={t('externalOutputsLocalHostWarningsTitle')}>
              <div className="inline-note-list">
                {currentPreview.warnings.map((warning) => (
                  <p key={warning}>{warning}</p>
                ))}
              </div>
            </ReviewSection>
          ) : null}

          <ReviewSection title={t('externalOutputsLocalHostManualTitle')}>
            {manualSteps.length > 0 ? (
              <div className="inline-note-list">
                {manualSteps.map((step) => (
                  <p key={step}>{step}</p>
                ))}
              </div>
            ) : null}
            {generatedFiles.length > 0 ? (
              <GeneratedArtifactViewer
                copyFeedback={copyFeedback}
                copyLabel={commonT('copyAction')}
                copyPathLabel={t('externalOutputsLocalHostCopyPathAction')}
                errorMessage={t('externalOutputsCopyFailed')}
                files={generatedFiles}
                onCopy={handleCopy}
                onOpenPath={(path) => {
                  void backend.openPathInFileManager(path)
                }}
                openPathLabel={commonT('openPath')}
                successMessage={commonT('copiedNotice')}
              />
            ) : null}
          </ReviewSection>

          <StatusCallout
            tone="info"
            title={t('externalOutputsLocalHostExecuteTitle')}
            body={t('externalOutputsLocalHostExecuteBody')}
            actions={
              <button
                className="btn-secondary"
                type="button"
                disabled={building}
                onClick={() => {
                  void handleBuild()
                }}
              >
                {building
                  ? t('externalOutputsLocalHostBuilding')
                  : installedHost
                    ? t('externalOutputsLocalHostUpdateAction')
                    : t('externalOutputsLocalHostCreateAction')}
              </button>
            }
          />

          {buildState ? (
            <StatusCallout
              tone={buildState.tone === 'success' ? 'success' : 'warning'}
              title={
                buildState.tone === 'success'
                  ? t('externalOutputsLocalHostVerifyTitle')
                  : t('externalOutputsLocalHostUnavailableTitle')
              }
              body={buildState.message}
            />
          ) : null}

          {installedHost ? (
            <ReviewSection title={t('externalOutputsLocalHostVerifyTitle')}>
              <div className="settings-field-grid">
                <div className="config-row">
                  <span className="config-label">
                    {t('externalOutputsLocalHostGeneratedAtLabel')}
                  </span>
                  <span className="config-value mono">
                    {formatDateTime(
                      installedHost.bundle.generatedAt,
                      language,
                    ) ?? installedHost.bundle.generatedAt}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('externalOutputsLocalHostScopeLabel')}
                  </span>
                  <span className="config-value mono">
                    {installedHost.bundle.profileId
                      ? profileIdLabel(installedHost.bundle.profileId)
                      : t('externalOutputsArchiveWideTitle')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('externalOutputsLocalHostWindowLabel')}
                  </span>
                  <span className="config-value mono">
                    {installedHost.bundle.dateRange.start} →{' '}
                    {installedHost.bundle.dateRange.end}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('externalOutputsLocalHostEntryPathLabel')}
                  </span>
                  <span className="config-value mono">
                    {installedHost.entryFilePath}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('externalOutputsLocalHostArtifactRootLabel')}
                  </span>
                  <span className="config-value mono">
                    {installedHost.artifactRoot}
                  </span>
                </div>
              </div>
              <div className="code-actions">
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={() => {
                    void backend.openExternalUrl(
                      toFileUrl(installedHost.entryFilePath),
                    )
                  }}
                >
                  {t('externalOutputsLocalHostOpenAction')}
                </button>
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={() => {
                    void backend.openPathInFileManager(
                      installedHost.artifactRoot,
                    )
                  }}
                >
                  {t('openDirectory')}
                </button>
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={() => {
                    void handleCopy(
                      'installed-entry-path',
                      installedHost.entryFilePath,
                    )
                  }}
                >
                  {commonT('copyAction')}
                </button>
              </div>
              <ReviewCopyStatus
                copyFeedback={copyFeedback}
                copyKey="installed-entry-path"
                errorMessage={t('externalOutputsCopyFailed')}
                successMessage={commonT('copiedNotice')}
              />
            </ReviewSection>
          ) : (
            <StatusCallout
              tone="info"
              title={t('externalOutputsLocalHostVerifyTitle')}
              body={t('externalOutputsLocalHostVerifyUnavailable')}
            />
          )}
        </>
      )}
    </>
  )
}
