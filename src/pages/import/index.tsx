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
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { backend } from '../../lib/backend-client'
import { subscribeToImportProgress } from '../../lib/ipc/import-progress'
import { useI18n } from '../../lib/i18n'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import type {
  BrowserProfile,
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  ImportProgressEvent,
  TakeoutInspection,
} from '../../lib/types'
import { ImportReviewPanels } from './review-panels'
import {
  type ImportMethod,
  type ImportWizardStepDefinition,
  type WizardStep,
} from './shared'
import { ImportWorkflowPanel } from './workflow-panel'

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
  const [supportCopyFeedback, setSupportCopyFeedback] =
    useState<ReviewCopyFeedback | null>(null)

  const wizardSteps: ImportWizardStepDefinition[] = [
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
      await waitForNextPaint()
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
      await waitForNextPaint()
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

  /**
   * Copies one review-path value through the shared review grammar so follow-through
   * surfaces keep the same feedback behavior as the rest of the app.
   */
  async function handleSupportPathCopy(key: string, value: string) {
    await copyReviewValue(value, {
      key,
      onFeedback: setSupportCopyFeedback,
    })
  }

  /**
   * Opens a reviewed support path through the host file manager.
   */
  function handleSupportPathOpen(path: string) {
    void backend.openPathInFileManager(path)
  }

  /**
   * Resets the import wizard after one completed run without touching the
   * route-level batch review state.
   */
  function handleImportAnother() {
    setStep('select')
    setInspection(null)
    setImportResult(null)
    setSourcePath('')
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

      <ImportWorkflowPanel
        detectedBrowserProfiles={detectedBrowserProfiles}
        importing={importing}
        importProgress={importProgress}
        importResult={importResult}
        inspection={inspection}
        language={language}
        manualPathExpanded={manualPathExpanded}
        method={method}
        selectedBrowserProfile={selectedBrowserProfile}
        selectedBrowserProfileId={selectedBrowserProfileId}
        sourcePath={sourcePath}
        step={step}
        stepIndex={stepIndex}
        wizardSteps={wizardSteps}
        workflowExpanded={workflowExpanded}
        workflowSteps={workflowSteps}
        onBrowseSource={handleBrowseSource}
        onCopyWorkflowValue={async (value) => {
          await copyReviewValue(value)
        }}
        onImport={handleImport}
        onImportAnother={handleImportAnother}
        onManualPathExpandedChange={setManualPathExpanded}
        onMethodChange={handleMethodChange}
        onScan={handleScan}
        onSelectBrowserProfile={handleSelectBrowserProfile}
        onSourcePathChange={applySourcePath}
        onStepChange={setStep}
        onWorkflowExpandedChange={setWorkflowExpanded}
      />

      <ImportReviewPanels
        activeBatchDetail={activeBatchDetail}
        actionError={actionError}
        healthReport={healthReport}
        language={language}
        loadingBatch={loadingBatch}
        recentImportBatches={recentImportBatches}
        repairNotice={repairNotice}
        selectedBatchId={selectedBatchId}
        supportCopyFeedback={supportCopyFeedback}
        onBatchMutation={handleBatchMutation}
        onCopyPath={handleSupportPathCopy}
        onOpenPath={handleSupportPathOpen}
        onRepairHealth={handleRepairHealth}
        onRunDoctor={handleRunDoctor}
        onSelectBatch={setSelectedBatchId}
      />
    </section>
  )
}
