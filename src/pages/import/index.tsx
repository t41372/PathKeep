/**
 * This module renders the Import route, including preview-first review, rollback/restore follow-through, and browser/takeout entry points.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ImportPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { backend } from '../../lib/backend-client'
import { subscribeToImportProgress } from '../../lib/ipc/import-progress'
import { useI18n } from '../../lib/i18n'
import {
  healthCheckStatusKey,
  healthCheckStatusTone,
  importBatchStatusKey,
  importBatchStatusTone,
} from '../../lib/trust-review'
import type {
  BrowserProfile,
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  ImportProgressEvent,
  TakeoutInspection,
} from '../../lib/types'
import { OperationWorkflow, PreviewEntryList } from '../../components/ui'
import { BusyOverlay } from '../../components/primitives/busy-overlay'

/**
 * Defines the type-level contract for import method.
 *
 * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
 */
type ImportMethod = 'takeout' | 'browser'
/**
 * Defines the type-level contract for wizard step.
 *
 * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
 */
type WizardStep = 'select' | 'scan' | 'preview' | 'confirm' | 'done'

function localizedImportProgressDetail(
  progress: ImportProgressEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
) {
  switch (progress.phase) {
    case 'prepare':
      return t('import.importProgressPrepareDetail', {
        files: progress.total.toLocaleString(language),
      })
    case 'import-file':
      return t('import.importProgressImportDetail', {
        current: progress.current.toLocaleString(language),
        total: progress.total.toLocaleString(language),
        source: progress.sourcePath ?? '',
      })
    case 'finalize':
      return t('import.importProgressFinalizeDetail')
    case 'complete':
      return t('import.importProgressCompleteDetail')
    default:
      return progress.detail
  }
}

/**
 * Renders the import route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Import expectations in the design docs.
 */
export function ImportPage() {
  const { refreshAppData, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [searchParams, setSearchParams] = useSearchParams()
  const [method, setMethod] = useState<ImportMethod>('takeout')
  const [step, setStep] = useState<WizardStep>('select')
  const [sourcePath, setSourcePath] = useState('')
  const [workflowExpanded, setWorkflowExpanded] = useState(false)
  const [manualPathExpanded, setManualPathExpanded] = useState(false)
  const [selectedBrowserProfileId, setSelectedBrowserProfileId] = useState<
    string | null
  >(null)
  const [inspection, setInspection] = useState<TakeoutInspection | null>(null)
  const [importing, setImporting] = useState(false)
  const [importProgress, setImportProgress] =
    useState<ImportProgressEvent | null>(null)
  const [importResult, setImportResult] = useState<TakeoutInspection | null>(
    null,
  )
  const [actionError, setActionError] = useState<string | null>(null)
  const [selectedBatchId, setSelectedBatchId] = useState<number | null>(null)
  const [selectedBatchDetail, setSelectedBatchDetail] =
    useState<ImportBatchDetail | null>(null)
  const [loadingBatch, setLoadingBatch] = useState(false)
  const [healthReport, setHealthReport] = useState<HealthReport | null>(null)
  const [repairNotice, setRepairNotice] = useState<string | null>(null)

  const wizardSteps: { key: WizardStep; label: string }[] = [
    { key: 'select', label: t('import.stepUpload') },
    { key: 'scan', label: t('import.stepScan') },
    { key: 'preview', label: t('import.stepPreview') },
    { key: 'confirm', label: t('import.stepConfirm') },
    { key: 'done', label: t('import.stepImport') },
  ]

  const stepIndex = wizardSteps.findIndex(
    (wizardStep) => wizardStep.key === step,
  )
  const recentImportBatches = snapshot?.recentImportBatches
  const batchIdFromParams = (() => {
    const raw = searchParams.get('batch')
    if (!raw) return null
    const parsed = Number.parseInt(raw, 10)
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null
  })()
  const detectedBrowserProfiles = useMemo(
    () =>
      [...(snapshot?.browserProfiles ?? [])]
        .filter((profile) => profile.historyExists && profile.historyPath)
        .sort((left, right) =>
          `${left.browserFamily}:${left.profileName}`.localeCompare(
            `${right.browserFamily}:${right.profileName}`,
          ),
        ),
    [snapshot?.browserProfiles],
  )
  const selectedBrowserProfile = useMemo(
    () =>
      detectedBrowserProfiles.find(
        (profile) => profile.profileId === selectedBrowserProfileId,
      ) ?? null,
    [detectedBrowserProfiles, selectedBrowserProfileId],
  )

  useEffect(() => {
    const recentBatches = recentImportBatches ?? []
    if (!recentBatches.length) {
      setSelectedBatchId(null)
      setSelectedBatchDetail(null)
      return
    }

    const requestedBatchId =
      batchIdFromParams &&
      recentBatches.some((batch) => batch.id === batchIdFromParams)
        ? batchIdFromParams
        : null

    setSelectedBatchId(
      (current) => requestedBatchId ?? current ?? recentBatches[0]?.id ?? null,
    )
  }, [batchIdFromParams, recentImportBatches])

  useEffect(() => {
    if (!selectedBatchId) return
    const currentBatch = searchParams.get('batch')
    if (currentBatch === String(selectedBatchId)) return
    const nextParams = new URLSearchParams(searchParams)
    nextParams.set('batch', String(selectedBatchId))
    setSearchParams(nextParams, { replace: true })
  }, [searchParams, selectedBatchId, setSearchParams])

  useEffect(() => {
    if (!selectedBatchId) {
      setSelectedBatchDetail(null)
      return
    }

    let cancelled = false
    /**
     * Loads batch.
     *
     * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadBatch = async () => {
      setSelectedBatchDetail(null)
      setActionError(null)
      setLoadingBatch(true)
      try {
        const detail = await backend.previewImportBatch(selectedBatchId)
        if (!cancelled) {
          setSelectedBatchDetail(detail)
        }
      } catch (nextError) {
        if (!cancelled) {
          setSelectedBatchDetail(null)
          setActionError(
            nextError instanceof Error
              ? nextError.message
              : t('common.unavailable'),
          )
        }
      } finally {
        if (!cancelled) {
          setLoadingBatch(false)
        }
      }
    }

    void loadBatch()
    return () => {
      cancelled = true
    }
  }, [selectedBatchId, t])

  useEffect(() => {
    if (method !== 'browser') return
    if (selectedBrowserProfileId || sourcePath.trim()) return
    const firstProfile = detectedBrowserProfiles[0]
    if (!firstProfile?.historyPath) return
    setSelectedBrowserProfileId(firstProfile.profileId)
    setSourcePath(firstProfile.historyPath)
  }, [detectedBrowserProfiles, method, selectedBrowserProfileId, sourcePath])

  const activeBatchDetail = useMemo(
    () =>
      selectedBatchDetail ??
      (importResult?.importBatch
        ? {
            batch: importResult.importBatch,
            previewEntries: importResult.previewEntries,
            recognizedFiles: importResult.recognizedFiles,
            quarantinedFiles: importResult.quarantinedFiles,
            notes: importResult.notes,
          }
        : null),
    [importResult, selectedBatchDetail],
  )

  const workflowSteps = useMemo(
    () => [
      {
        id: 'preview',
        title: t('import.workflowPreviewTitle'),
        status:
          step === 'preview' || step === 'confirm' || step === 'done'
            ? ('complete' as const)
            : ('pending' as const),
        summary: t('import.workflowPreviewSummary'),
        reason: t('import.workflowPreviewReason'),
        files: inspection?.recognizedFiles.map((file) => file.path),
      },
      {
        id: 'manual',
        title: t('import.workflowManualTitle'),
        status:
          inspection !== null ? ('complete' as const) : ('pending' as const),
        summary: t('import.workflowManualSummary'),
        reason: t('import.workflowManualReason'),
        checklist: [
          t('import.manualLocateStep'),
          t('import.manualInspectStep'),
          t('import.manualContinueStep'),
        ],
      },
      {
        id: 'execute',
        title: t('import.workflowExecuteTitle'),
        status: step === 'done' ? ('complete' as const) : ('pending' as const),
        summary: t('import.workflowExecuteSummary'),
        reason: t('import.workflowExecuteReason'),
      },
      {
        id: 'verify',
        title: t('import.workflowVerifyTitle'),
        status:
          activeBatchDetail !== null || importResult !== null
            ? ('complete' as const)
            : ('pending' as const),
        summary: t('import.workflowVerifySummary'),
        reason: t('import.workflowVerifyReason'),
      },
      {
        id: 'finish',
        title: t('import.workflowFinishTitle'),
        status:
          activeBatchDetail !== null && healthReport !== null
            ? ('complete' as const)
            : ('pending' as const),
        summary: t('import.workflowFinishSummary'),
        reason: t('import.workflowFinishReason'),
      },
    ],
    [activeBatchDetail, healthReport, importResult, inspection, step, t],
  )

  /**
   * Handles scan.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleScan() {
    if (!sourcePath.trim()) return
    setActionError(null)
    setStep('scan')
    try {
      const result = await backend.inspectTakeout({ sourcePath, dryRun: true })
      setInspection(result)
      setImportResult(null)
      setStep('preview')
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
      setStep('select')
    }
  }

  /**
   * Applies a new import source path and resets any stale inspection state that
   * no longer belongs to that source.
   *
   * This keeps the route honest: once the user changes the file or browser
   * profile they are about to import, the previous preview should stop
   * pretending it still matches.
   */
  function applySourcePath(
    nextPath: string,
    options?: { browserProfileId?: string | null },
  ) {
    setSourcePath(nextPath)
    setInspection(null)
    setImportResult(null)
    setActionError(null)
    setStep('select')
    setSelectedBrowserProfileId(options?.browserProfileId ?? null)
  }

  /**
   * Switches between the takeout-first and browser-path import flows.
   *
   * The two entry points share later review steps, but they start with
   * different defaults and affordances, so changing method also resets the
   * route back to a clean selection state.
   */
  function handleMethodChange(nextMethod: ImportMethod) {
    if (nextMethod === method) return
    setMethod(nextMethod)
    setStep('select')
    setInspection(null)
    setImportResult(null)
    setActionError(null)
    setManualPathExpanded(false)
    if (nextMethod === 'browser') {
      const firstProfile = detectedBrowserProfiles[0]
      if (firstProfile?.historyPath) {
        setSelectedBrowserProfileId(firstProfile.profileId)
        setSourcePath(firstProfile.historyPath)
      } else {
        setSelectedBrowserProfileId(null)
        setSourcePath('')
        setManualPathExpanded(true)
      }
      return
    }

    setSelectedBrowserProfileId(null)
    setSourcePath('')
  }

  /**
   * Handles select browser profile.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function handleSelectBrowserProfile(profile: BrowserProfile) {
    if (!profile.historyPath) return
    applySourcePath(profile.historyPath, {
      browserProfileId: profile.profileId,
    })
  }

  /**
   * Handles browse source.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleBrowseSource(options: { directory: boolean }) {
    setActionError(null)
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: options.directory,
        multiple: false,
        title: options.directory
          ? t('import.chooseTakeoutFolder')
          : method === 'takeout'
            ? t('import.chooseTakeoutFile')
            : t('import.chooseHistoryFile'),
      })
      if (typeof selected !== 'string' || !selected.trim()) return
      applySourcePath(selected)
      if (method === 'browser') {
        setManualPathExpanded(false)
      }
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('import.filePickerUnavailable'),
      )
    }
  }

  /**
   * Handles import.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleImport() {
    if (!sourcePath.trim()) return
    setActionError(null)
    setImporting(true)
    setImportProgress(null)
    setStep('confirm')
    let unsubscribe = () => {}
    try {
      unsubscribe = await subscribeToImportProgress((progress) => {
        setImportProgress(progress)
      })
      const result = await backend.importTakeout({ sourcePath, dryRun: false })
      setImportResult(result)
      await refreshAppData()
      setStep('done')
      if (result.importBatch) {
        setSelectedBatchId(result.importBatch.id)
        try {
          setSelectedBatchDetail(
            await backend.previewImportBatch(result.importBatch.id),
          )
        } catch (nextError) {
          setSelectedBatchDetail(null)
          setActionError(
            nextError instanceof Error
              ? nextError.message
              : t('common.unavailable'),
          )
        }
      }
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
      setStep('preview')
    } finally {
      unsubscribe()
      setImporting(false)
    }
  }

  /**
   * Handles run doctor.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRunDoctor() {
    setActionError(null)
    try {
      const report = await backend.doctor()
      setHealthReport(report)
      setRepairNotice(null)
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    }
  }

  /**
   * Handles repair health.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRepairHealth() {
    setActionError(null)
    try {
      const report = await backend.repairHealth()
      setRepairNotice(
        t('import.repairSummary', {
          derivedRows: report.clearedDerivedRows,
          visibilityRows: report.repairedVisibilityRows,
          importAudits: report.repairedImportAudits,
        }),
      )
      const nextHealthReport = await backend.doctor()
      setHealthReport(nextHealthReport)
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    }
  }

  /**
   * Handles batch mutation.
   *
   * Keeping this as a named declaration makes the Import surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleBatchMutation(
    batch: ImportBatchOverview,
    action: 'revert' | 'restore',
  ) {
    const message =
      action === 'revert'
        ? t('import.revertConfirm')
        : t('import.restoreConfirm')

    if (typeof window !== 'undefined' && 'confirm' in window) {
      if (!window.confirm(message)) {
        return
      }
    }

    setActionError(null)
    try {
      const detail =
        action === 'revert'
          ? await backend.revertImportBatch(batch.id)
          : await backend.restoreImportBatch(batch.id)
      await refreshAppData()
      setSelectedBatchId(detail.batch.id)
      setSelectedBatchDetail(detail)
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    }
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          description={t('import.archiveNotInitializedBody')}
          eyebrow={t('navigation.importLabel')}
          title={t('import.archiveNotInitialized')}
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('import.goToSetup')}
            </Link>
          }
        />
      </section>
    )
  }

  return (
    <section className="page-shell import-page" data-testid="import-page">
      <StatusCallout
        tone="info"
        title={t('import.trustTitle')}
        body={t('import.trustBody')}
      />

      <div className="panel">
        <div className="panel-header panel-header--toggle">
          <span className="panel-title">{t('import.workflowLabel')}</span>
          <button
            aria-expanded={workflowExpanded}
            className="btn-ghost"
            type="button"
            onClick={() => setWorkflowExpanded((current) => !current)}
          >
            {workflowExpanded
              ? t('import.hideWorkflow')
              : t('import.showWorkflow')}
          </button>
        </div>
        {workflowExpanded ? (
          <div className="panel-body">
            <OperationWorkflow
              actionLabel={t('import.workflowLabel')}
              labels={{
                why: t('common.whyThisStepMatters'),
                files: t('common.filesLabel'),
                commands: t('common.commandsLabel'),
                checklist: t('common.checklistLabel'),
                copy: t('common.copyAction'),
                current: t('common.current'),
                complete: t('common.complete'),
                pending: t('common.pending'),
                command: (index) => t('common.commandStepLabel', { index }),
              }}
              onCopy={async (value) => {
                await navigator.clipboard.writeText(value)
              }}
              steps={workflowSteps}
            />
          </div>
        ) : (
          <div className="panel-body panel-body--compact">
            <p className="dashboard-next-action">
              {t('import.workflowCollapsedHint')}
            </p>
          </div>
        )}
      </div>

      <div className="import-container">
        <div className="import-methods">
          <button
            className={`import-card ${method === 'takeout' ? 'active-import' : ''}`}
            type="button"
            aria-pressed={method === 'takeout'}
            onClick={() => handleMethodChange('takeout')}
          >
            <div className="import-card-icon">↓</div>
            <div className="import-card-title">
              {t('import.takeoutMethodTitle')}
            </div>
            <div className="import-card-desc dim">
              {t('import.takeoutMethodBody')}
            </div>
          </button>
          <button
            className={`import-card ${method === 'browser' ? 'active-import' : ''}`}
            type="button"
            aria-pressed={method === 'browser'}
            onClick={() => handleMethodChange('browser')}
          >
            <div className="import-card-icon">⊕</div>
            <div className="import-card-title">
              {t('import.browserMethodTitle')}
            </div>
            <div className="import-card-desc dim">
              {t('import.browserMethodBody')}
            </div>
          </button>
        </div>
        <p className="dim" style={{ marginTop: 'var(--space-2)' }}>
          {method === 'takeout'
            ? t('import.takeoutPreparationHint')
            : t('import.browserPreparationHint')}
        </p>

        <div className="wizard-panel">
          <div className="wizard-steps">
            {wizardSteps.map((wizardStep, index) => (
              <div key={wizardStep.key} style={{ display: 'contents' }}>
                {index > 0 && (
                  <div
                    className={`wizard-step-line ${
                      index <= stepIndex
                        ? 'completed'
                        : index === stepIndex + 1
                          ? 'active'
                          : ''
                    }`}
                  />
                )}
                <div
                  aria-current={index === stepIndex ? 'step' : undefined}
                  className={`wizard-step ${
                    index < stepIndex
                      ? 'completed'
                      : index === stepIndex
                        ? 'active-step'
                        : ''
                  }`}
                >
                  <div className="step-number">{index + 1}</div>
                  <div className="step-label">{wizardStep.label}</div>
                </div>
              </div>
            ))}
          </div>

          <div className="wizard-body">
            {step === 'select' && (
              <>
                <div className="wizard-title">{t('import.selectTitle')}</div>
                <div className="wizard-description dim">
                  {method === 'takeout'
                    ? t('import.takeoutSelectBody')
                    : t('import.browserSelectBody')}
                </div>
                {method === 'browser' ? (
                  <div
                    className="import-source-stack"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <div className="row-between">
                      <span className="mono-kicker">
                        {t('import.detectedBrowserProfiles')}
                      </span>
                      <span className="mono-support">
                        {t('import.detectedBrowserProfilesCount', {
                          count:
                            detectedBrowserProfiles.length.toLocaleString(
                              language,
                            ),
                        })}
                      </span>
                    </div>
                    {detectedBrowserProfiles.length > 0 ? (
                      <div className="import-profile-list">
                        {detectedBrowserProfiles.map((profile) => (
                          <button
                            key={profile.profileId}
                            className={`result-row import-profile-card ${
                              selectedBrowserProfileId === profile.profileId
                                ? 'result-row--active'
                                : ''
                            }`}
                            type="button"
                            onClick={() => handleSelectBrowserProfile(profile)}
                          >
                            <div className="result-row__header">
                              <strong>
                                {profile.browserName} · {profile.profileName}
                              </strong>
                              <span className="status-badge">
                                {t('import.browserProfileReady')}
                              </span>
                            </div>
                            <div className="result-row__meta">
                              <span className="mono-support">
                                {profile.profileId}
                              </span>
                              <span className="mono-support">
                                {profile.historyFileName}
                              </span>
                            </div>
                            <p className="mono-support">
                              {profile.historyPath}
                            </p>
                          </button>
                        ))}
                      </div>
                    ) : (
                      <StatusCallout
                        tone="warning"
                        title={t('import.noDetectedBrowserProfilesTitle')}
                        body={t('import.noDetectedBrowserProfilesBody')}
                      />
                    )}
                  </div>
                ) : null}
                <div className="import-source-actions">
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void handleBrowseSource({ directory: false })
                    }}
                  >
                    {method === 'takeout'
                      ? t('import.chooseTakeoutFile')
                      : t('import.chooseHistoryFile')}
                  </button>
                  {method === 'takeout' ? (
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleBrowseSource({ directory: true })
                      }}
                    >
                      {t('import.chooseTakeoutFolder')}
                    </button>
                  ) : (
                    <button
                      aria-expanded={manualPathExpanded}
                      className="btn-ghost"
                      type="button"
                      onClick={() =>
                        setManualPathExpanded((current) => !current)
                      }
                    >
                      {manualPathExpanded
                        ? t('import.hideManualPath')
                        : t('import.showManualPath')}
                    </button>
                  )}
                </div>
                {sourcePath.trim() ? (
                  <div className="import-source-summary">
                    <span className="mono-kicker">
                      {t('import.selectedSource')}
                    </span>
                    <span className="mono-support">{sourcePath}</span>
                    {method === 'browser' && selectedBrowserProfile ? (
                      <span className="mono-support">
                        {selectedBrowserProfile.browserName} ·{' '}
                        {selectedBrowserProfile.profileName}
                      </span>
                    ) : null}
                  </div>
                ) : null}
                {(method === 'takeout' ||
                  manualPathExpanded ||
                  detectedBrowserProfiles.length === 0) && (
                  <label
                    className="field-stack import-manual-path"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <span className="mono-kicker">
                      {t('import.sourcePath')}
                    </span>
                    <input
                      type="text"
                      value={sourcePath}
                      onChange={(event) => {
                        applySourcePath(event.target.value)
                        if (method === 'browser') {
                          setManualPathExpanded(true)
                        }
                      }}
                      placeholder={
                        method === 'takeout'
                          ? t('import.takeoutPathPlaceholder')
                          : t('import.browserPathPlaceholder')
                      }
                    />
                  </label>
                )}
                <div className="wizard-actions">
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={() => {
                      void handleScan()
                    }}
                    disabled={!sourcePath.trim()}
                    aria-disabled={!sourcePath.trim()}
                  >
                    {t('import.scanSource')}
                  </button>
                </div>
              </>
            )}

            {step === 'scan' && (
              <div style={{ position: 'relative', minHeight: '120px' }}>
                <BusyOverlay
                  label={t('import.scanningTitle')}
                  detail={t('import.workflowPreviewReason')}
                  progressLabel={`2 / ${wizardSteps.length.toLocaleString(language)}`}
                  progressValue={(2 / wizardSteps.length) * 100}
                  steps={wizardSteps
                    .slice(0, 3)
                    .map((wizardStep) => wizardStep.label)}
                  activeStep={1}
                />
              </div>
            )}

            {step === 'preview' && inspection && (
              <>
                <div className="wizard-title">{t('import.previewTitle')}</div>
                <div className="wizard-description dim">
                  {t('import.previewBody')}
                </div>

                <div className="preview-stats">
                  <div className="preview-stat">
                    <div className="preview-stat-label">
                      {t('import.recordsFound')}
                    </div>
                    <div className="preview-stat-value mono">
                      {inspection.candidateItems.toLocaleString(language)}
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">
                      {t('import.duplicates')}
                    </div>
                    <div className="preview-stat-value mono">
                      {inspection.duplicateItems.toLocaleString(language)}
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">
                      {t('import.newRecords')}
                    </div>
                    <div className="preview-stat-value mono accent">
                      {(
                        inspection.candidateItems - inspection.duplicateItems
                      ).toLocaleString(language)}
                    </div>
                  </div>
                </div>

                {inspection.recognizedFiles.length > 0 && (
                  <div className="preview-files">
                    <div
                      className="panel-header"
                      style={{ marginTop: 'var(--space-4)' }}
                    >
                      <span className="panel-title">
                        {t('import.detectedFiles')}
                      </span>
                    </div>
                    {inspection.recognizedFiles.map((file) => (
                      <div key={file.path} className="file-item">
                        <span
                          className="file-status ok"
                          aria-label={t('common.statusSuccess')}
                        >
                          ✓
                        </span>
                        <span
                          className="file-name mono"
                          title={file.path}
                          style={{
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                        >
                          {file.path}
                        </span>
                        <span className="file-detail dim">
                          {file.records.toLocaleString(language)} · {file.kind}
                        </span>
                      </div>
                    ))}
                  </div>
                )}

                {inspection.quarantinedFiles.length > 0 && (
                  <div className="preview-files">
                    <div
                      className="panel-header"
                      style={{ marginTop: 'var(--space-4)' }}
                    >
                      <span className="panel-title">
                        {t('import.quarantinedFiles')}
                      </span>
                    </div>
                    {inspection.quarantinedFiles.map((file) => (
                      <div key={file.path} className="file-item">
                        <span
                          className="file-status warn"
                          aria-label={t('common.warning')}
                        >
                          ⚠
                        </span>
                        <span
                          className="file-name mono"
                          title={file.path}
                          style={{
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                        >
                          {file.path}
                        </span>
                        <span className="file-detail dim">{file.status}</span>
                      </div>
                    ))}
                  </div>
                )}

                {inspection.previewEntries.length > 0 ? (
                  <div
                    className="panel"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <div className="panel-header">
                      <span className="panel-title">
                        {t('import.previewRows')}
                      </span>
                    </div>
                    <div className="panel-body">
                      <PreviewEntryList
                        entries={inspection.previewEntries}
                        language={language}
                        statusLabel={(status) =>
                          t(importBatchStatusKey(status))
                        }
                        statusTone={importBatchStatusTone}
                      />
                    </div>
                  </div>
                ) : null}

                {inspection.notes.length > 0 && (
                  <div className="inline-note-list dim">
                    {inspection.notes.map((note) => (
                      <div key={note}>{note}</div>
                    ))}
                  </div>
                )}

                <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
                  <div className="panel-header">
                    <span className="panel-title">
                      {t('import.confirmSummaryTitle')}
                    </span>
                  </div>
                  <div className="panel-body">
                    <p className="dim">{t('import.confirmSummaryBody')}</p>
                    <div className="manifest-grid">
                      <div className="manifest-field">
                        <span className="field-label">
                          {t('import.confirmSummaryNewRecords')}
                        </span>
                        <span className="field-value mono accent">
                          {(
                            inspection.candidateItems -
                            inspection.duplicateItems
                          ).toLocaleString(language)}
                        </span>
                      </div>
                      <div className="manifest-field">
                        <span className="field-label">
                          {t('import.confirmSummaryDuplicates')}
                        </span>
                        <span className="field-value mono">
                          {inspection.duplicateItems.toLocaleString(language)}
                        </span>
                      </div>
                      <div className="manifest-field">
                        <span className="field-label">
                          {t('import.confirmSummaryFiles')}
                        </span>
                        <span className="field-value mono">
                          {inspection.recognizedFiles.length.toLocaleString(
                            language,
                          )}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="wizard-actions">
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => setStep('select')}
                    disabled={importing}
                  >
                    {t('import.backAction')}
                  </button>
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={() => {
                      void handleImport()
                    }}
                    disabled={importing}
                  >
                    {t('import.confirmImport')}
                  </button>
                </div>
              </>
            )}

            {step === 'confirm' && importing && (
              <div style={{ position: 'relative', minHeight: '120px' }}>
                <BusyOverlay
                  label={t('import.importingTitle')}
                  detail={
                    importProgress
                      ? localizedImportProgressDetail(
                          importProgress,
                          t,
                          language,
                        )
                      : t('import.importingProgressDetail', {
                          records: (
                            inspection?.candidateItems ?? 0
                          ).toLocaleString(language),
                          files: (
                            inspection?.recognizedFiles.length ?? 0
                          ).toLocaleString(language),
                        })
                  }
                  logLines={
                    importProgress
                      ? [
                          localizedImportProgressDetail(
                            importProgress,
                            t,
                            language,
                          ),
                        ]
                      : []
                  }
                  progressLabel={
                    importProgress
                      ? `${importProgress.current.toLocaleString(language)} / ${importProgress.total.toLocaleString(language)}`
                      : `4 / ${wizardSteps.length.toLocaleString(language)}`
                  }
                  progressValue={
                    importProgress?.progressPercent ??
                    (4 / wizardSteps.length) * 100
                  }
                  steps={wizardSteps
                    .slice(2)
                    .map((wizardStep) => wizardStep.label)}
                  activeStep={1}
                />
              </div>
            )}

            {step === 'done' && importResult && (
              <>
                <div className="wizard-title">{t('import.completeTitle')}</div>
                <div className="wizard-description dim">
                  {t('import.completeBody')}
                </div>
                <div className="preview-stats">
                  <div className="preview-stat">
                    <div className="preview-stat-label">
                      {t('import.imported')}
                    </div>
                    <div className="preview-stat-value mono accent">
                      {importResult.importedItems.toLocaleString(language)}
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">
                      {t('import.duplicatesSkipped')}
                    </div>
                    <div className="preview-stat-value mono">
                      {importResult.duplicateItems.toLocaleString(language)}
                    </div>
                  </div>
                </div>
                <div className="wizard-actions">
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={() => {
                      setStep('select')
                      setInspection(null)
                      setImportResult(null)
                      setSourcePath('')
                    }}
                  >
                    {t('import.importAnother')}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="dashboard-grid">
        <div className="dashboard-left">
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{t('import.recentBatches')}</span>
            </div>
            <div className="panel-body">
              <p className="dashboard-next-action">
                {t('import.recentBatchesBody')}
              </p>
              {(recentImportBatches?.length ?? 0) === 0 ? (
                <p className="dim">{t('import.noImportBatches')}</p>
              ) : (
                <div className="result-list">
                  {(recentImportBatches ?? []).map((batch) => (
                    <button
                      key={batch.id}
                      className={`result-row ${
                        selectedBatchId === batch.id ? 'result-row--active' : ''
                      }`}
                      type="button"
                      onClick={() => setSelectedBatchId(batch.id)}
                    >
                      <div className="result-row__header">
                        <strong>
                          {t('import.batchIdLabel', {
                            id: String(batch.id),
                          })}
                        </strong>
                        <span
                          aria-label={t(importBatchStatusKey(batch.status))}
                          className="status-badge"
                        >
                          {t(importBatchStatusKey(batch.status))}
                        </span>
                      </div>
                      <p
                        className="mono"
                        style={{
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                        title={batch.sourcePath}
                      >
                        {batch.sourcePath}
                      </p>
                      <div className="result-row__meta dim">
                        <span>
                          {t('import.importedRows')}: {batch.importedItems}
                        </span>
                        <span>
                          {t('import.visibleRows')}: {batch.visibleItems}
                        </span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{t('import.healthReport')}</span>
            </div>
            <div className="panel-body">
              <p className="dashboard-next-action">
                {t('import.healthReportBody')}
              </p>
              <div className="wizard-actions">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => {
                    void handleRunDoctor()
                  }}
                >
                  {t('import.runHealthCheckAction')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={!healthReport}
                  aria-disabled={!healthReport}
                  onClick={() => {
                    void handleRepairHealth()
                  }}
                >
                  {t('common.repairAction')}
                </button>
              </div>
              <p className="dim" style={{ marginTop: 'var(--space-2)' }}>
                {t('import.repairDescription')}
              </p>
              {healthReport ? (
                <div
                  className="manual-steps"
                  style={{ marginTop: 'var(--space-4)' }}
                >
                  {healthReport.checks.map((check) => (
                    <StatusCallout
                      key={check.name}
                      tone={healthCheckStatusTone(check.status)}
                      title={`${t(healthCheckStatusKey(check.status))} — ${check.name}`}
                      body={check.message}
                    />
                  ))}
                </div>
              ) : (
                <p className="dim">{t('import.noHealthChecks')}</p>
              )}
              {repairNotice ? (
                <p className="mono-support" role="status">
                  {repairNotice}
                </p>
              ) : null}
            </div>
          </div>
        </div>

        <div className="dashboard-right">
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{t('import.selectedBatch')}</span>
            </div>
            <div className="panel-body">
              <p className="dashboard-next-action">
                {t('import.selectedBatchBody')}
              </p>
              {loadingBatch ? (
                <p className="dim">{t('common.loading')}</p>
              ) : activeBatchDetail ? (
                <>
                  <div className="manifest-grid">
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.candidateRows')}
                      </span>
                      <span className="field-value mono">
                        {activeBatchDetail.batch.candidateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.importedRows')}
                      </span>
                      <span className="field-value mono">
                        {activeBatchDetail.batch.importedItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.duplicateRows')}
                      </span>
                      <span className="field-value mono">
                        {activeBatchDetail.batch.duplicateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.visibleRows')}
                      </span>
                      <span className="field-value mono">
                        {activeBatchDetail.batch.visibleItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                  </div>
                  <div className="detail-divider" />
                  {activeBatchDetail.previewEntries.length > 0 ? (
                    <PreviewEntryList
                      entries={activeBatchDetail.previewEntries}
                      language={language}
                      statusLabel={(status) => t(importBatchStatusKey(status))}
                      statusTone={importBatchStatusTone}
                    />
                  ) : (
                    <p className="dim">{t('import.noPreviewRows')}</p>
                  )}
                  {activeBatchDetail.batch.auditPath ? (
                    <div className="code-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => {
                          void backend.openPathInFileManager(
                            activeBatchDetail.batch.auditPath ?? '',
                          )
                        }}
                      >
                        {t('common.openAction')}
                      </button>
                    </div>
                  ) : null}
                  <div className="wizard-actions">
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleBatchMutation(
                          activeBatchDetail.batch,
                          'revert',
                        )
                      }}
                      disabled={activeBatchDetail.batch.status === 'reverted'}
                      aria-disabled={
                        activeBatchDetail.batch.status === 'reverted'
                      }
                    >
                      {t('import.revertBatch')}
                    </button>
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleBatchMutation(
                          activeBatchDetail.batch,
                          'restore',
                        )
                      }}
                      disabled={activeBatchDetail.batch.status !== 'reverted'}
                      aria-disabled={
                        activeBatchDetail.batch.status !== 'reverted'
                      }
                    >
                      {t('import.restoreBatch')}
                    </button>
                  </div>
                </>
              ) : (
                <p className="dim">{t('import.noImportBatches')}</p>
              )}
              {actionError ? (
                <p className="inline-error" role="alert">
                  {actionError}
                </p>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
