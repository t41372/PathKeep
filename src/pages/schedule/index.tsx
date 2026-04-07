import { useEffect, useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import type { SchedulePlan } from '../../lib/types'

interface ScheduleLoadState {
  requestKey: number
  plan: SchedulePlan | null
  error: string | null
}

export function SchedulePage() {
  const { refreshKey } = useShellData()
  const [loadState, setLoadState] = useState<ScheduleLoadState>({
    requestKey: -1,
    plan: null,
    error: null,
  })
  const [selectedFileIndex, setSelectedFileIndex] = useState(0)

  useEffect(() => {
    let cancelled = false
    const loadPlan = async () => {
      try {
        const nextPlan = await backend.previewSchedule()
        if (!cancelled) {
          setLoadState({
            requestKey: refreshKey,
            plan: nextPlan,
            error: null,
          })
          setSelectedFileIndex(0)
        }
      } catch (nextError) {
        if (!cancelled) {
          setLoadState({
            requestKey: refreshKey,
            plan: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : 'PathKeep could not preview the native schedule.',
          })
        }
      }
    }

    void loadPlan()

    return () => {
      cancelled = true
    }
  }, [refreshKey])

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const error = loadState.requestKey === refreshKey ? loadState.error : null
  const loading = loadState.requestKey !== refreshKey

  if (loading && !plan) {
    return (
      <section className="page-shell">
        <LoadingState label="Rendering native schedule preview" />
      </section>
    )
  }

  if (error || !plan) {
    return (
      <section className="page-shell">
        <ErrorState
          title="Schedule preview unavailable"
          description={
            error ?? 'PathKeep could not render the native schedule artifacts.'
          }
        />
      </section>
    )
  }

  const selectedFile = plan.generatedFiles[selectedFileIndex] ?? null

  return (
    <section className="page-shell schedule-page" data-testid="schedule-page">
      <div className="content-grid">
        <section className="shell-panel shell-panel--accent">
          <div className="panel-header">
            <span className="panel-title">SCHEDULE PREVIEW</span>
            <span className="panel-action">{plan.platform}</span>
          </div>
          <div className="panel-body stack-list">
            <article className="list-item">
              <strong>Execution label</strong>
              <span className="mono-support">{plan.label}</span>
            </article>
            <article className="list-item">
              <strong>Worker executable</strong>
              <span className="mono-support">{plan.executablePath}</span>
            </article>
            <article className="list-item">
              <strong>Apply mode</strong>
              <span className="mono-support">
                {plan.applySupported
                  ? 'This platform can apply the schedule directly after explicit review.'
                  : 'This platform stays manual-first in M1. Review the artifact and install it yourself.'}
              </span>
            </article>
          </div>
        </section>

        <aside className="stacked-column">
          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">MANUAL STEPS</span>
              <span className="panel-action">
                {plan.manualSteps.length} steps
              </span>
            </div>
            <div className="panel-body stack-list">
              {plan.manualSteps.map((step) => (
                <article key={step} className="list-item">
                  <strong>Manual step</strong>
                  <span className="mono-support">{step}</span>
                </article>
              ))}
            </div>
          </section>

          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">GENERATED FILES</span>
              <span className="panel-action">
                {plan.generatedFiles.length} artifacts
              </span>
            </div>
            <div className="panel-body stack-list">
              <div className="generated-file-tabs">
                {plan.generatedFiles.map((file, index) => (
                  <button
                    key={file.relativePath}
                    className={`chip-button ${
                      selectedFileIndex === index ? 'chip-button--active' : ''
                    }`}
                    type="button"
                    onClick={() => setSelectedFileIndex(index)}
                  >
                    {file.relativePath}
                  </button>
                ))}
              </div>
              {selectedFile ? (
                <article className="code-panel">
                  <div className="row-between">
                    <strong>{selectedFile.purpose}</strong>
                    <span className="mono-support">
                      {selectedFile.relativePath}
                    </span>
                  </div>
                  <pre>{selectedFile.contents}</pre>
                </article>
              ) : null}
            </div>
          </section>
        </aside>
      </div>
    </section>
  )
}
