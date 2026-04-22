/**
 * @file pme-panel.tsx
 * @description Render-only PME workflow panel for the Schedule route.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Render Preview / Manual / Execute / Verify tab content for schedule automation.
 * - Keep generated artifacts, direct execution commands, and verify affordances together in one owner.
 * - Surface copy/open-path interactions without owning the actual side effects.
 *
 * ## Not responsible for
 * - Fetching schedule status or preview data.
 * - Running apply/remove side effects directly.
 * - Rendering route-level loading, unavailable, or top-level status callouts.
 *
 * ## Dependencies
 * - Depends on shared review primitives for generated files, path rows, PME tabs, and verify checklists.
 * - Depends on the route owner for all mutations and copy/open-path side effects.
 *
 * ## Performance notes
 * - Render-only owner that works from already-loaded plan/status snapshots and avoids extra backend reads.
 */

import {
  GeneratedArtifactViewer,
  PmeTabBar,
  ReviewPathActionRow,
  type ReviewCopyFeedback,
  VerifyCheckList,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../../lib/types'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Stable tab ids for the schedule PME workflow.
 */
export type SchedulePmeTab = 'preview' | 'manual' | 'execute' | 'verify'

/**
 * Last schedule execution result surfaced back into the PME review flow.
 */
export interface ScheduleExecutionState {
  mode: 'apply' | 'remove'
  result: ApplyResult
}

interface SchedulePmePanelProps {
  actionError: string | null
  busy: string | null
  copyFeedback: ReviewCopyFeedback | null
  executionResult: ScheduleExecutionState | null
  installDescription: string
  lastBackupLabel: string
  latestAuditPath: string | null
  onApply: () => void
  onCopyValue: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => Promise<void>
  onRemove: () => void
  plan: SchedulePlan
  pmeTab: SchedulePmeTab
  setPmeTab: (tab: SchedulePmeTab) => void
  snapshotInitialized: boolean
  status: ScheduleStatus
  t: Translator
}

function joinCommand(command: string[]) {
  return command
    .map((part) => (part.includes(' ') ? `"${part}"` : part))
    .join(' ')
}

/**
 * Renders the schedule PME workflow from already-loaded route state.
 */
export function SchedulePmePanel({
  actionError,
  busy,
  copyFeedback,
  executionResult,
  installDescription,
  lastBackupLabel,
  latestAuditPath,
  onApply,
  onCopyValue,
  onOpenPath,
  onRemove,
  plan,
  pmeTab,
  setPmeTab,
  snapshotInitialized,
  status,
  t,
}: SchedulePmePanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('schedule.pmeTitle')}</span>
        <PmeTabBar
          activeTab={pmeTab}
          onChange={setPmeTab}
          tabs={[
            { key: 'preview', label: t('common.previewTab') },
            { key: 'manual', label: t('common.manualTab') },
            { key: 'execute', label: t('common.executeTab') },
            { key: 'verify', label: t('common.verifyTab') },
          ]}
        />
      </div>
      <div className="panel-body">
        {pmeTab === 'preview' ? (
          <>
            <div className="summary-label">{t('schedule.previewBoundary')}</div>
            <p className="dashboard-next-action">{t('schedule.previewBody')}</p>
            {plan.generatedFiles.length > 0 ? (
              <GeneratedArtifactViewer
                copyFeedback={copyFeedback}
                copyLabel={t('common.copyAction')}
                copyPathLabel={t('common.copyAction')}
                errorMessage={t('audit.copyFailed')}
                files={plan.generatedFiles}
                onCopy={(key, value) => {
                  void onCopyValue(key, value)
                }}
                onOpenPath={(path) => {
                  void onOpenPath(path)
                }}
                openPathLabel={t('common.openPath')}
                successMessage={t('common.copiedNotice')}
              />
            ) : (
              <div className="dim" style={{ fontSize: '12px' }}>
                {t('schedule.noGeneratedFiles')}
              </div>
            )}
          </>
        ) : null}

        {pmeTab === 'manual' ? (
          <div className="manual-steps">
            {status.manualSteps.map((step, index) => (
              <div key={step} className="manual-step">
                <span className="step-num-inline mono">{index + 1}</span>
                <span>{step}</span>
              </div>
            ))}
            {status.detectedFiles.map((path) => (
              <ReviewPathActionRow
                key={path}
                copyFeedback={copyFeedback}
                copyKey={`schedule:detected:${path}`}
                copyLabel={t('common.copyAction')}
                errorMessage={t('audit.copyFailed')}
                label={t('common.fileStepLabel')}
                onCopy={(key, value) => {
                  void onCopyValue(key, value)
                }}
                onOpenPath={(nextPath) => {
                  void onOpenPath(nextPath)
                }}
                openPathLabel={t('common.openPath')}
                successMessage={t('common.copiedNotice')}
                value={path}
              />
            ))}
            {status.auditPath ? (
              <ReviewPathActionRow
                copyFeedback={copyFeedback}
                copyKey="schedule:audit-path"
                copyLabel={t('common.copyAction')}
                errorMessage={t('audit.copyFailed')}
                label={t('schedule.openLatestAudit')}
                onCopy={(key, value) => {
                  void onCopyValue(key, value)
                }}
                onOpenPath={(path) => {
                  void onOpenPath(path)
                }}
                openPathLabel={t('common.openPath')}
                successMessage={t('common.copiedNotice')}
                value={status.auditPath}
              />
            ) : null}
          </div>
        ) : null}

        {pmeTab === 'execute' ? (
          <div className="manual-steps">
            <div className="manual-step">
              <span className="step-num-inline mono">
                {t('schedule.executeRun')}
              </span>
              <span>{t('schedule.executeBody')}</span>
            </div>
            {plan.applyCommands.map((command, index) => (
              <div key={`${command.join(' ')}-${index}`} className="code-panel">
                <div className="summary-label">
                  {t('schedule.applyCommand', { index: index + 1 })}
                </div>
                <pre className="code-block">
                  <code>{joinCommand(command)}</code>
                </pre>
              </div>
            ))}
            {plan.rollbackCommands.map((command, index) => (
              <div
                key={`${command.join(' ')}-rollback-${index}`}
                className="code-panel"
              >
                <div className="summary-label">
                  {t('schedule.rollbackCommand', { index: index + 1 })}
                </div>
                <pre className="code-block">
                  <code>{joinCommand(command)}</code>
                </pre>
              </div>
            ))}
            {actionError ? (
              <p className="inline-error" role="alert">
                {actionError}
              </p>
            ) : null}
            {executionResult ? (
              <div className="warning-box">
                <div className="warning-icon">ℹ</div>
                <div className="warning-text">
                  <strong>
                    {executionResult.mode === 'apply'
                      ? t('schedule.applySchedule')
                      : t('schedule.removeSchedule')}
                  </strong>{' '}
                  {executionResult.result.message}
                </div>
              </div>
            ) : null}
            <div className="wizard-actions">
              <button
                className="btn-primary"
                type="button"
                disabled={
                  busy !== null || !snapshotInitialized || !plan.applySupported
                }
                onClick={onApply}
              >
                {busy === t('schedule.applySchedule')
                  ? busy
                  : t('schedule.applySchedule')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={
                  busy !== null ||
                  !plan.applySupported ||
                  (status.installState === 'not-installed' &&
                    status.detectedFiles.length === 0)
                }
                onClick={onRemove}
              >
                {busy === t('schedule.removeSchedule')
                  ? busy
                  : t('schedule.removeSchedule')}
              </button>
              {executionResult?.result.auditPath ? (
                <ReviewPathActionRow
                  copyFeedback={copyFeedback}
                  copyKey="schedule:execution-audit"
                  copyLabel={t('common.copyAction')}
                  errorMessage={t('audit.copyFailed')}
                  label={t('schedule.openSchedulerAudit')}
                  onCopy={(key, value) => {
                    void onCopyValue(key, value)
                  }}
                  onOpenPath={(path) => {
                    void onOpenPath(path)
                  }}
                  openPathLabel={t('common.openPath')}
                  successMessage={t('common.copiedNotice')}
                  value={executionResult.result.auditPath}
                />
              ) : null}
            </div>
            {!snapshotInitialized ? (
              <p className="mono-support">
                {t('schedule.initializeArchiveFirst')}
              </p>
            ) : null}
          </div>
        ) : null}

        {pmeTab === 'verify' ? (
          <div className="settings-result-list">
            <div className="summary-label">{t('common.verifyTab')}</div>
            <p className="dashboard-next-action">{installDescription}</p>
            <VerifyCheckList
              items={[
                {
                  key: 'install-state',
                  label: t('schedule.installState'),
                  status:
                    status.installState === 'installed'
                      ? t('schedule.installedBadge')
                      : status.installState === 'manual-review'
                        ? t('schedule.manualReviewBadge')
                        : status.installState === 'not-installed'
                          ? t('schedule.notInstalledBadge')
                          : t('schedule.attentionBadge'),
                  body: installDescription,
                },
                {
                  key: 'last-triggered',
                  label: t('schedule.lastTriggered'),
                  status: lastBackupLabel,
                },
                {
                  key: 'detected-files',
                  label: t('common.filesLabel'),
                  status:
                    status.detectedFiles.length > 0
                      ? String(status.detectedFiles.length)
                      : t('common.notAvailable'),
                  body:
                    status.detectedFiles.length > 0
                      ? status.detectedFiles.join(' · ')
                      : undefined,
                },
              ]}
            />
            {status.detectedFiles.length > 0 ? (
              <div className="manual-steps">
                {status.detectedFiles.map((path) => (
                  <ReviewPathActionRow
                    key={path}
                    copyFeedback={copyFeedback}
                    copyKey={`schedule:verify-detected:${path}`}
                    copyLabel={t('common.copyAction')}
                    errorMessage={t('audit.copyFailed')}
                    label={t('common.fileStepLabel')}
                    onCopy={(key, value) => {
                      void onCopyValue(key, value)
                    }}
                    onOpenPath={(nextPath) => {
                      void onOpenPath(nextPath)
                    }}
                    openPathLabel={t('common.openPath')}
                    successMessage={t('common.copiedNotice')}
                    value={path}
                  />
                ))}
              </div>
            ) : null}
            {status.warnings.length > 0 ? (
              <div className="warning-box">
                <div className="warning-icon">⚠</div>
                <div className="warning-text">
                  {status.warnings.map((warning) => (
                    <div key={warning}>{warning}</div>
                  ))}
                </div>
              </div>
            ) : (
              <StatusCallout
                tone="success"
                title={t('common.statusClear')}
                body={installDescription}
              />
            )}
            {executionResult ? (
              <div className="warning-box">
                <div className="warning-icon">ℹ</div>
                <div className="warning-text">
                  <strong>
                    {executionResult.mode === 'apply'
                      ? t('schedule.applySchedule')
                      : t('schedule.removeSchedule')}
                  </strong>{' '}
                  {executionResult.result.message}
                </div>
              </div>
            ) : null}
            {latestAuditPath ? (
              <ReviewPathActionRow
                copyFeedback={copyFeedback}
                copyKey="schedule:latest-audit"
                copyLabel={t('common.copyAction')}
                errorMessage={t('audit.copyFailed')}
                label={t('schedule.openLatestAudit')}
                onCopy={(key, value) => {
                  void onCopyValue(key, value)
                }}
                onOpenPath={(path) => {
                  void onOpenPath(path)
                }}
                openPathLabel={t('common.openPath')}
                successMessage={t('common.copiedNotice')}
                value={latestAuditPath}
              />
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  )
}
