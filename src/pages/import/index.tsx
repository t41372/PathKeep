import { useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { backend } from '../../lib/backend'
import type { TakeoutInspection } from '../../lib/types'

type ImportMethod = 'takeout' | 'browser'
type WizardStep = 'select' | 'scan' | 'preview' | 'confirm' | 'done'

const wizardSteps: { key: WizardStep; label: string }[] = [
  { key: 'select', label: 'Upload' },
  { key: 'scan', label: 'Scan' },
  { key: 'preview', label: 'Preview' },
  { key: 'confirm', label: 'Confirm' },
  { key: 'done', label: 'Import' },
]

export function ImportPage() {
  const { snapshot } = useShellData()
  const [method, setMethod] = useState<ImportMethod>('takeout')
  const [step, setStep] = useState<WizardStep>('select')
  const [sourcePath, setSourcePath] = useState('')
  const [inspection, setInspection] = useState<TakeoutInspection | null>(null)
  const [importing, setImporting] = useState(false)
  const [importResult, setImportResult] = useState<TakeoutInspection | null>(
    null,
  )

  const stepIndex = wizardSteps.findIndex((s) => s.key === step)

  async function handleScan() {
    if (!sourcePath.trim()) return
    setStep('scan')
    try {
      const result = await backend.inspectTakeout({ sourcePath, dryRun: true })
      setInspection(result)
      setStep('preview')
    } catch {
      setStep('select')
    }
  }

  async function handleImport() {
    if (!sourcePath.trim()) return
    setImporting(true)
    setStep('confirm')
    try {
      const result = await backend.importTakeout({ sourcePath, dryRun: false })
      setImportResult(result)
      setStep('done')
    } catch {
      setStep('preview')
    } finally {
      setImporting(false)
    }
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          description="Initialize the archive first before importing external history data."
          eyebrow="IMPORT"
          title="Archive not initialized"
        />
      </section>
    )
  }

  return (
    <section className="page-shell import-page" data-testid="import-page">
      {/* Import Method Cards */}
      <div className="import-container">
        <div className="import-methods">
          <div
            className={`import-card ${method === 'takeout' ? 'active-import' : ''}`}
            onClick={() => setMethod('takeout')}
          >
            <div className="import-card-icon">↓</div>
            <div className="import-card-title">Google Takeout</div>
            <div className="import-card-desc dim">
              Import from exported archive
            </div>
          </div>
          <div
            className={`import-card ${method === 'browser' ? 'active-import' : ''}`}
            onClick={() => setMethod('browser')}
          >
            <div className="import-card-icon">⊕</div>
            <div className="import-card-title">Browser Direct</div>
            <div className="import-card-desc dim">
              Import from local browser DB
            </div>
          </div>
        </div>

        {/* Wizard */}
        <div className="wizard-panel">
          <div className="wizard-steps">
            {wizardSteps.map((ws, i) => (
              <div key={ws.key} style={{ display: 'contents' }}>
                {i > 0 && (
                  <div
                    className={`wizard-step-line ${i <= stepIndex ? 'completed' : i === stepIndex + 1 ? 'active' : ''}`}
                  />
                )}
                <div
                  className={`wizard-step ${i < stepIndex ? 'completed' : i === stepIndex ? 'active-step' : ''}`}
                >
                  <div className="step-number">{i + 1}</div>
                  <div className="step-label">{ws.label}</div>
                </div>
              </div>
            ))}
          </div>

          <div className="wizard-body">
            {step === 'select' && (
              <>
                <div className="wizard-title">Step 1: Select Source</div>
                <div className="wizard-description dim">
                  {method === 'takeout'
                    ? 'Provide the path to your Google Takeout export (zip or folder).'
                    : 'Provide the path to a browser History database file.'}
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
                  <span className="mono-kicker">SOURCE PATH</span>
                  <input
                    type="text"
                    value={sourcePath}
                    onChange={(e) => setSourcePath(e.target.value)}
                    placeholder={
                      method === 'takeout'
                        ? '/path/to/takeout.zip'
                        : '/path/to/History'
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
                  >
                    Scan Source →
                  </button>
                </div>
              </>
            )}

            {step === 'scan' && (
              <>
                <div className="wizard-title">Step 2: Scanning...</div>
                <div className="wizard-description dim">
                  Inspecting source file for recognized history formats.
                </div>
              </>
            )}

            {step === 'preview' && inspection && (
              <>
                <div className="wizard-title">Step 3: Preview Import</div>
                <div className="wizard-description dim">
                  Review what will be imported before confirming.
                </div>

                <div className="preview-stats">
                  <div className="preview-stat">
                    <div className="preview-stat-label">Records Found</div>
                    <div className="preview-stat-value mono">
                      {inspection.candidateItems.toLocaleString()}
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">Duplicates</div>
                    <div className="preview-stat-value mono">
                      {inspection.duplicateItems.toLocaleString()} (will skip)
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">New Records</div>
                    <div className="preview-stat-value mono accent">
                      {(
                        inspection.candidateItems - inspection.duplicateItems
                      ).toLocaleString()}
                    </div>
                  </div>
                </div>

                {inspection.recognizedFiles.length > 0 && (
                  <div className="preview-files">
                    <div
                      className="panel-header"
                      style={{ marginTop: 'var(--space-4)' }}
                    >
                      <span className="panel-title">DETECTED FILES</span>
                    </div>
                    {inspection.recognizedFiles.map((f) => (
                      <div key={f.path} className="file-item">
                        <span className="file-status ok">✓</span>
                        <span className="file-name mono">{f.path}</span>
                        <span className="file-detail dim">
                          {f.records} entries · {f.kind}
                        </span>
                      </div>
                    ))}
                  </div>
                )}

                {inspection.quarantinedFiles.length > 0 && (
                  <div className="preview-files">
                    {inspection.quarantinedFiles.map((f) => (
                      <div key={f.path} className="file-item">
                        <span className="file-status warn">⚠</span>
                        <span className="file-name mono">{f.path}</span>
                        <span className="file-detail dim">
                          quarantined · {f.status}
                        </span>
                      </div>
                    ))}
                  </div>
                )}

                {inspection.notes.length > 0 && (
                  <div
                    style={{ marginTop: 'var(--space-3)', fontSize: '11px' }}
                    className="dim"
                  >
                    {inspection.notes.map((n) => (
                      <div key={n}>{n}</div>
                    ))}
                  </div>
                )}

                <div className="wizard-actions">
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => setStep('select')}
                  >
                    ← Back
                  </button>
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={() => {
                      void handleImport()
                    }}
                  >
                    Confirm Import →
                  </button>
                </div>
              </>
            )}

            {step === 'confirm' && importing && (
              <>
                <div className="wizard-title">Step 4: Importing...</div>
                <div className="wizard-description dim">
                  Writing records to the archive. This may take a moment.
                </div>
              </>
            )}

            {step === 'done' && importResult && (
              <>
                <div className="wizard-title">Step 5: Import Complete</div>
                <div className="wizard-description dim">
                  Records have been written to the archive.
                </div>
                <div className="preview-stats">
                  <div className="preview-stat">
                    <div className="preview-stat-label">Imported</div>
                    <div className="preview-stat-value mono accent">
                      {importResult.importedItems.toLocaleString()}
                    </div>
                  </div>
                  <div className="preview-stat">
                    <div className="preview-stat-label">Duplicates Skipped</div>
                    <div className="preview-stat-value mono">
                      {importResult.duplicateItems.toLocaleString()}
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
                    Import Another
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  )
}
