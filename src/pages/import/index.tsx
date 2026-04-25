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
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { copyReviewValue } from '../../components/review'
import { EmptyState } from '../../components/primitives/empty-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { clearIntelligenceOverviewCache } from '../../lib/core-intelligence/api'
import { subscribeToImportProgress } from '../../lib/ipc/import-progress'
import { useI18n } from '../../lib/i18n'
import {
  isBrowserProfileReadable,
  macosFullDiskAccessSettingsUrl,
} from '../../lib/platform-guidance'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import type {
  BrowserHistoryImportRequest,
  BrowserProfile,
  ImportProgressEvent,
  TakeoutInspection,
} from '../../lib/types'
import { ImportReviewPanels } from './review-panels'
import {
  buildImportWorkflowSteps,
  type ImportMethod,
  type ImportWizardStepDefinition,
  type WizardStep,
} from './shared'
import { useImportReviewState } from './use-import-review-state'
import { ImportWorkflowPanel } from './workflow-panel'

function isValidatedBrowserDirectProfile(profile: BrowserProfile) {
  const browserName = profile.browserName.toLocaleLowerCase()
  return (
    profile.browserFamily === 'safari' ||
    browserName === 'google chrome' ||
    browserName === 'chatgpt atlas' ||
    browserName === 'perplexity comet' ||
    profile.profileId.startsWith('atlas:') ||
    profile.profileId.startsWith('comet:') ||
    profile.profileId.startsWith('chrome:')
  )
}

function isFullDiskAccessError(message: string) {
  return (
    message.includes('Full Disk Access') ||
    message.includes('完全磁盘访问权限') ||
    message.includes('完整磁碟取用權') ||
    message.includes('全盤讀取權限')
  )
}

/**
 * Renders the import route.
 *
 * This route should keep its deep links, loading states, trust copy, and
 * repair affordances aligned with the Import expectations in the design docs.
 */
export function ImportPage() {
  const { refreshAppData, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [method, setMethod] = useState<ImportMethod>('takeout')
  const [step, setStep] = useState<WizardStep>('select')
  const [sourcePath, setSourcePath] = useState('')
  const [workflowExpanded, setWorkflowExpanded] = useState(false)
  const [historyExpanded, setHistoryExpanded] = useState(false)
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
  const detectedBrowserProfiles = useMemo(
    () =>
      [...(snapshot?.browserProfiles ?? [])]
        .filter(isValidatedBrowserDirectProfile)
        .sort((left, right) =>
          `${left.browserFamily}:${left.profileName}`.localeCompare(
            `${right.browserFamily}:${right.profileName}`,
          ),
        ),
    [snapshot?.browserProfiles],
  )
  const readyBrowserProfiles = useMemo(
    () =>
      detectedBrowserProfiles.filter(
        (profile) => isBrowserProfileReadable(profile) && profile.historyPath,
      ),
    [detectedBrowserProfiles],
  )
  const selectedBrowserProfile = useMemo(
    () =>
      detectedBrowserProfiles.find(
        (profile) => profile.profileId === selectedBrowserProfileId,
      ) ?? null,
    [detectedBrowserProfiles, selectedBrowserProfileId],
  )
  const {
    actionError,
    activeBatchDetail,
    clearActionError,
    handleBatchMutation,
    handleRepairHealth,
    handleRunDoctor,
    handleSupportPathCopy,
    handleSupportPathOpen,
    healthReport,
    loadingBatch,
    repairNotice,
    reportActionError,
    selectBatchId,
    selectedBatchId,
    setLoadedBatchDetail,
    supportCopyFeedback,
  } = useImportReviewState({
    importResult,
    recentImportBatches,
    refreshAppData,
    t,
  })

  useEffect(() => {
    if (method !== 'browser') return
    if (selectedBrowserProfileId || sourcePath.trim()) return
    const firstProfile = readyBrowserProfiles[0]
    if (!firstProfile?.historyPath) return
    setSelectedBrowserProfileId(firstProfile.profileId)
    setSourcePath(firstProfile.historyPath)
  }, [method, readyBrowserProfiles, selectedBrowserProfileId, sourcePath])

  const workflowSteps = useMemo(
    () =>
      buildImportWorkflowSteps({
        activeBatchDetail,
        healthReport,
        importResult,
        inspection,
        step,
        t,
      }),
    [activeBatchDetail, healthReport, importResult, inspection, step, t],
  )

  function buildBrowserHistoryRequest(options: {
    dryRun: boolean
  }): BrowserHistoryImportRequest {
    return {
      sourcePath,
      dryRun: options.dryRun,
      browserFamily: selectedBrowserProfile?.browserFamily ?? null,
      profileId: selectedBrowserProfile?.profileId ?? null,
      browserName: selectedBrowserProfile?.browserName ?? null,
      profileName: selectedBrowserProfile?.profileName ?? null,
    }
  }

  /**
   * Handles scan.
   *
   * Keeping this as a named declaration makes the Import surface easier to
   * review and test than burying the behavior inside another anonymous
   * callback.
   */
  async function handleScan() {
    if (!sourcePath.trim()) return
    clearActionError()
    setStep('scan')
    try {
      await waitForNextPaint()
      const result =
        method === 'takeout'
          ? await backend.inspectTakeout({ sourcePath, dryRun: true })
          : await backend.inspectBrowserHistory(
              buildBrowserHistoryRequest({ dryRun: true }),
            )
      setInspection(result)
      setImportResult(null)
      setStep('preview')
    } catch (nextError) {
      reportActionError(nextError)
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
    clearActionError()
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
    clearActionError()
    setManualPathExpanded(false)
    if (nextMethod === 'browser') {
      const firstProfile = readyBrowserProfiles[0]
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
   * Keeping this as a named declaration makes the Import surface easier to
   * review and test than burying the behavior inside another anonymous
   * callback.
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
   * Keeping this as a named declaration makes the Import surface easier to
   * review and test than burying the behavior inside another anonymous
   * callback.
   */
  async function handleBrowseSource(options: { directory: boolean }) {
    clearActionError()
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
      reportActionError(
        nextError instanceof Error
          ? nextError
          : new Error(t('import.filePickerUnavailable')),
      )
    }
  }

  /**
   * Opens the exact macOS settings pane Safari needs for Browser Direct access.
   */
  async function handleOpenFullDiskAccessSettings() {
    clearActionError()
    try {
      await backend.openExternalUrl(macosFullDiskAccessSettingsUrl)
    } catch (nextError) {
      reportActionError(nextError)
    }
  }

  /**
   * Handles import.
   *
   * Keeping this as a named declaration makes the Import surface easier to
   * review and test than burying the behavior inside another anonymous
   * callback.
   */
  async function handleImport() {
    if (!sourcePath.trim()) return
    clearActionError()
    setImporting(true)
    setImportProgress(null)
    setStep('confirm')
    let unsubscribe = () => {}
    try {
      await waitForNextPaint()
      unsubscribe = await subscribeToImportProgress((progress) => {
        setImportProgress(progress)
      })
      const result =
        method === 'takeout'
          ? await backend.importTakeout({ sourcePath, dryRun: false })
          : await backend.importBrowserHistory(
              buildBrowserHistoryRequest({ dryRun: false }),
            )
      setImportResult(result)
      setStep('done')
      clearIntelligenceOverviewCache()
      void refreshAppData().catch((nextError) => {
        reportActionError(nextError)
      })
      if (result.importBatch) {
        selectBatchId(result.importBatch.id)
        try {
          setLoadedBatchDetail(
            await backend.previewImportBatch(result.importBatch.id),
          )
        } catch (nextError) {
          setLoadedBatchDetail(null)
          reportActionError(nextError)
        }
      }
    } catch (nextError) {
      reportActionError(nextError)
      setStep('preview')
    } finally {
      unsubscribe()
      setImporting(false)
    }
  }

  /**
   * Resets the import wizard after one completed run without touching the
   * route-level batch review state.
   */
  function handleImportAnother() {
    setStep('select')
    setInspection(null)
    setImportResult(null)
    setImportProgress(null)
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
        onOpenFullDiskAccessSettings={handleOpenFullDiskAccessSettings}
        onScan={handleScan}
        onSelectBrowserProfile={handleSelectBrowserProfile}
        onSourcePathChange={applySourcePath}
        onStepChange={setStep}
        onWorkflowExpandedChange={setWorkflowExpanded}
      />

      {actionError ? (
        <StatusCallout
          tone="danger"
          title={t('import.actionErrorTitle')}
          body={actionError}
          actions={
            isFullDiskAccessError(actionError) ? (
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  void handleOpenFullDiskAccessSettings()
                }}
              >
                {t('import.openFullDiskAccessSettings')}
              </button>
            ) : undefined
          }
        />
      ) : null}

      <ImportReviewPanels
        activeBatchDetail={activeBatchDetail}
        healthReport={healthReport}
        historyExpanded={historyExpanded}
        language={language}
        loadingBatch={loadingBatch}
        recentImportBatches={recentImportBatches}
        repairNotice={repairNotice}
        selectedBatchId={selectedBatchId}
        supportCopyFeedback={supportCopyFeedback}
        onBatchMutation={handleBatchMutation}
        onCopyPath={handleSupportPathCopy}
        onHistoryExpandedChange={setHistoryExpanded}
        onOpenPath={handleSupportPathOpen}
        onRepairHealth={handleRepairHealth}
        onRunDoctor={handleRunDoctor}
        onSelectBatch={selectBatchId}
      />
    </section>
  )
}
