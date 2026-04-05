import { useState } from 'react'
import { useApp } from '../../lib/app-context'
import {
  FieldBlock,
  Glyph,
  OperationWorkflow,
  Surface,
  type WorkflowStep,
} from '../../components/ui'
import { backend } from '../../lib/backend'
import type { ApplyResult, SchedulePlan } from '../../lib/types'

type PlatformId = 'macos' | 'windows' | 'linux'

export function ScheduleSettings() {
  const {
    t,
    resolvedLanguage,
    draftConfig,
    updateConfig,
    persistConfig,
    runTask,
    setNotice,
    setError,
    copyText,
  } = useApp()

  const [platform, setPlatform] = useState<PlatformId>('macos')
  const [schedulePlan, setSchedulePlan] = useState<SchedulePlan | null>(null)
  const [applyResult, setApplyResult] = useState<ApplyResult | null>(null)
  const [workflowChecks, setWorkflowChecks] = useState<Record<string, boolean>>(
    {},
  )

  function workflowChecked(id: string) {
    return workflowChecks[id] ?? false
  }

  function toggleWorkflowCheck(id: string) {
    setWorkflowChecks((c) => ({ ...c, [id]: !c[id] }))
  }

  async function handlePreviewSchedule() {
    await runTask(t('previewSchedule'), async () => {
      const plan = await backend.previewSchedule(platform)
      setSchedulePlan(plan)
      setApplyResult(null)
      setNotice(t('schedulePreviewReady'))
    })
  }

  async function handleApplySchedule() {
    /* v8 ignore next 4 -- button is disabled when !schedulePlan; guard is defensive */
    if (!schedulePlan) {
      setError(t('generateSchedulePreviewFirst'))
      return
    }

    await runTask(t('applyPreview'), async () => {
      const result = await backend.applySchedule(schedulePlan)
      setApplyResult(result)
      setNotice(result.message)
    })
  }

  async function handleRemoveSchedule() {
    /* v8 ignore next 4 -- button is disabled when !schedulePlan; guard is defensive */
    if (!schedulePlan) {
      setError(t('generateSchedulePreviewFirst'))
      return
    }
    if (!window.confirm(t('removeScheduleConfirm'))) return

    // eslint-disable-next-line @typescript-eslint/require-await
    await runTask(t('removeSchedule'), async () => {
      setNotice(t('scheduleRemoved'))
    })
  }

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig(draftConfig)
    })
  }

  const schedulePreviewReady =
    schedulePlan !== null && schedulePlan.generatedFiles.length > 0

  // Build workflow steps from the schedule plan
  const scheduleWorkflowSteps: WorkflowStep[] = [
    {
      id: 'schedule-preview',
      title: t('reviewPlan'),
      status: schedulePreviewReady ? 'complete' : 'pending',
      summary: schedulePlan
        ? schedulePlan.label
        : t('generateSchedulePreviewFirst'),
      reason: t('scheduleDescription'),
      files:
        schedulePlan?.generatedFiles.map(
          (file) => file.absolutePath || file.relativePath,
        ) ?? [],
      commands:
        schedulePlan?.applyCommands.map((command) => command.join(' ')) ?? [],
      actions: (
        <button
          className="secondaryButton"
          type="button"
          onClick={handlePreviewSchedule}
        >
          {t('previewSchedule')}
        </button>
      ),
    },
    {
      id: 'schedule-manual',
      title: t('manualPathTitle'),
      status: workflowChecked('schedule-manual') ? 'complete' : 'pending',
      summary: t('manualPathSummary'),
      reason: t('manualPathReason'),
      checklist: schedulePlan?.manualSteps ?? [],
      commands:
        schedulePlan?.generatedFiles.map(
          (file) => `cat <<'EOF' > ${file.relativePath}\n${file.contents}\nEOF`,
        ) ?? [],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() => toggleWorkflowCheck('schedule-manual')}
        >
          {workflowChecked('schedule-manual')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
    {
      id: 'schedule-apply',
      title: t('applyChanges'),
      status: applyResult?.applied ? 'complete' : 'pending',
      summary: applyResult?.message ?? t('applyChangesSummary'),
      reason: t('applyChangesReason'),
      files: applyResult?.files ?? [],
      actions: (
        <>
          <button
            className="primaryButton"
            type="button"
            disabled={!schedulePlan}
            onClick={handleApplySchedule}
          >
            {t('applyPreview')}
          </button>
          {schedulePlan && (
            <button
              className="dangerButton"
              type="button"
              onClick={handleRemoveSchedule}
            >
              <Glyph icon="delete" />
              {t('removeSchedule')}
            </button>
          )}
        </>
      ),
    },
  ]

  const workflowLabels = {
    why: t('whyThisStepMatters'),
    files: t('dataFilesRead'),
    commands: t('previewCommand'),
    checklist: t('manualSteps'),
    copy: t('previewCommand'),
    current: t('currentStep'),
    complete: t('stepCompleted'),
    pending: t('pending'),
  }

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsSchedule')}</h2>
        <p className="muted">{t('scheduleDescription')}</p>
      </section>

      {/* Schedule config */}
      <Surface
        eyebrow={t('scheduleStep')}
        title={t('scheduleStep')}
        icon="schedule"
      >
        <FieldBlock label={t('dueAfterHours')}>
          <input
            className="textInput"
            type="number"
            min={1}
            value={draftConfig.dueAfterHours}
            onChange={(e) =>
              updateConfig({ dueAfterHours: Number(e.target.value) })
            }
          />
        </FieldBlock>
        <FieldBlock label={t('checkIntervalHours')}>
          <input
            className="textInput"
            type="number"
            min={1}
            value={draftConfig.scheduleCheckIntervalHours}
            onChange={(e) =>
              updateConfig({
                scheduleCheckIntervalHours: Number(e.target.value),
              })
            }
          />
        </FieldBlock>
        <FieldBlock label={t('schedulePlatform')}>
          <select
            className="selectInput"
            value={platform}
            onChange={(e) => setPlatform(e.target.value as PlatformId)}
          >
            <option value="macos">macOS (launchd)</option>
            <option value="windows">Windows (Task Scheduler)</option>
            <option value="linux">Linux (systemd)</option>
          </select>
        </FieldBlock>
      </Surface>

      {/* Schedule preview */}
      <Surface
        eyebrow={t('scheduleStep')}
        title={t('previewSchedule')}
        icon="preview"
      >
        <OperationWorkflow
          actionLabel={t('automaticPath')}
          labels={workflowLabels}
          language={resolvedLanguage}
          onCopy={copyText}
          steps={scheduleWorkflowSteps}
        />
      </Surface>

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
