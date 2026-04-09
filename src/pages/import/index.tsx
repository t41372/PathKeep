import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { backend } from '../../lib/backend'
import { useI18n } from '../../lib/i18n'
import {
  healthCheckStatusKey,
  healthCheckStatusTone,
  importBatchStatusKey,
  importBatchStatusTone,
} from '../../lib/trust-review'
import type {
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  TakeoutInspection,
} from '../../lib/types'
import { OperationWorkflow, PreviewEntryList } from '../../components/ui'
import { BusyOverlay } from '../../components/primitives/busy-overlay'

type ImportMethod = 'takeout' | 'browser'
type WizardStep = 'select' | 'scan' | 'preview' | 'confirm' | 'done'

export function ImportPage() {
  const { refreshAppData, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [method, setMethod] = useState<ImportMethod>('takeout')
  const [step, setStep] = useState<WizardStep>('select')
  const [sourcePath, setSourcePath] = useState('')
  const [inspection, setInspection] = useState<TakeoutInspection | null>(null)
  const [importing, setImporting] = useState(false)
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

  useEffect(() => {
    const recentBatches = recentImportBatches ?? []
    if (!recentBatches.length) {
      setSelectedBatchId(null)
      setSelectedBatchDetail(null)
      return
    }

    setSelectedBatchId((current) => current ?? recentBatches[0]?.id ?? null)
  }, [recentImportBatches])

  useEffect(() => {
    if (!selectedBatchId) {
      setSelectedBatchDetail(null)
      return
    }

    let cancelled = false
    const loadBatch = async () => {
      setLoadingBatch(true)
      try {
        const detail = await backend.previewImportBatch(selectedBatchId)
        if (!cancelled) {
          setSelectedBatchDetail(detail)
        }
      } catch (nextError) {
        if (!cancelled) {
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
          selectedBatchDetail !== null || importResult !== null
            ? ('complete' as const)
            : ('pending' as const),
        summary: t('import.workflowVerifySummary'),
        reason: t('import.workflowVerifyReason'),
      },
      {
        id: 'finish',
        title: t('import.workflowFinishTitle'),
        status:
          selectedBatchDetail !== null && healthReport !== null
            ? ('complete' as const)
            : ('pending' as const),
        summary: t('import.workflowFinishSummary'),
        reason: t('import.workflowFinishReason'),
      },
    ],
    [healthReport, importResult, inspection, selectedBatchDetail, step, t],
  )

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

  async function handleImport() {
    if (!sourcePath.trim()) return
    setActionError(null)
    setImporting(true)
    setStep('confirm')
    try {
      const result = await backend.importTakeout({ sourcePath, dryRun: false })
      setImportResult(result)
      await refreshAppData()
      setStep('done')
      if (result.importBatch) {
        setSelectedBatchId(result.importBatch.id)
      }
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
      setStep('preview')
    } finally {
      setImporting(false)
    }
  }

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
        <div className="panel-header">
          <span className="panel-title">{t('import.workflowLabel')}</span>
        </div>
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
            }}
            language={language}
            onCopy={async (value) => {
              await navigator.clipboard.writeText(value)
            }}
            steps={workflowSteps}
          />
        </div>
      </div>

      <div className="import-container">
        <div className="import-methods">
          <button
            className={`import-card ${method === 'takeout' ? 'active-import' : ''}`}
            type="button"
            aria-pressed={method === 'takeout'}
            onClick={() => setMethod('takeout')}
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
            onClick={() => setMethod('browser')}
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
                <label
                  className="field-stack"
                  style={{
                    marginTop: 'var(--space-4)',
                    border: 'none',
                    background: 'transparent',
                    padding: 0,
                  }}
                >
                  <span className="mono-kicker">{t('import.sourcePath')}</span>
                  <input
                    type="text"
                    value={sourcePath}
                    onChange={(event) => setSourcePath(event.target.value)}
                    placeholder={
                      method === 'takeout'
                        ? t('import.takeoutPathPlaceholder')
                        : t('import.browserPathPlaceholder')
                    }
                  />
                </label>
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
                  detail={t('import.importingProgressDetail', {
                    records: (inspection?.candidateItems ?? 0).toLocaleString(
                      language,
                    ),
                    files: (
                      inspection?.recognizedFiles.length ?? 0
                    ).toLocaleString(language),
                  })}
                  progressLabel={`4 / ${wizardSteps.length.toLocaleString(language)}`}
                  progressValue={(4 / wizardSteps.length) * 100}
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
              ) : selectedBatchDetail ? (
                <>
                  <div className="manifest-grid">
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.candidateRows')}
                      </span>
                      <span className="field-value mono">
                        {selectedBatchDetail.batch.candidateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.importedRows')}
                      </span>
                      <span className="field-value mono">
                        {selectedBatchDetail.batch.importedItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.duplicateRows')}
                      </span>
                      <span className="field-value mono">
                        {selectedBatchDetail.batch.duplicateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.visibleRows')}
                      </span>
                      <span className="field-value mono">
                        {selectedBatchDetail.batch.visibleItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                  </div>
                  <div className="detail-divider" />
                  {selectedBatchDetail.previewEntries.length > 0 ? (
                    <PreviewEntryList
                      entries={selectedBatchDetail.previewEntries}
                      language={language}
                      statusLabel={(status) => t(importBatchStatusKey(status))}
                      statusTone={importBatchStatusTone}
                    />
                  ) : (
                    <p className="dim">{t('import.noPreviewRows')}</p>
                  )}
                  {selectedBatchDetail.batch.auditPath ? (
                    <div className="code-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => {
                          void backend.openPathInFileManager(
                            selectedBatchDetail.batch.auditPath ?? '',
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
                          selectedBatchDetail.batch,
                          'revert',
                        )
                      }}
                      disabled={selectedBatchDetail.batch.status === 'reverted'}
                      aria-disabled={
                        selectedBatchDetail.batch.status === 'reverted'
                      }
                    >
                      {t('import.revertBatch')}
                    </button>
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleBatchMutation(
                          selectedBatchDetail.batch,
                          'restore',
                        )
                      }}
                      disabled={selectedBatchDetail.batch.status !== 'reverted'}
                      aria-disabled={
                        selectedBatchDetail.batch.status !== 'reverted'
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
