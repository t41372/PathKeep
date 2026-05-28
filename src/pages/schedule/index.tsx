/**
 * @file index.tsx
 * @description State-machine Scheduled Backup Settings route.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Render exactly one schedule state at a time: checking, not installed, OK, warning, or error.
 * - Keep automatic actions, manual paths, verification, and result feedback inline.
 * - Show selected browser profiles as read-only configuration with a route back to Settings.
 *
 * ## Not responsible for
 * - Mutating backup worker behavior.
 * - Editing browser profile selection directly.
 * - Translating backend issue codes outside the schedule namespace.
 *
 * ## Dependencies
 * - `useScheduleWorkflow` owns load/action state and native scheduler command calls.
 * - Shared interval selector keeps options aligned with onboarding.
 * - Router `Link` sends profile edits to the Settings profile section.
 *
 * ## Performance notes
 * - The route renders small scheduler/profile summaries only. It never loads
 *   archive history or unbounded browser data.
 */

import { Link, useNavigate } from 'react-router-dom'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { BackupIntervalSelector } from '../../components/schedule/backup-interval-selector'
import { BrowserIcon } from '../../lib/browser-icons'
import { backend } from '../../lib/backend-client'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type { ResolvedLanguage } from '../../lib/i18n'
import { backupIntervalHoursToMinutes } from '../../lib/schedule-options'
import type {
  AppSnapshot,
  BrowserProfile,
  ScheduleIssue,
  ScheduleManualStep,
  SchedulePlan,
  ScheduleStatus,
} from '../../lib/types'
import { useScheduleWorkflow } from './use-schedule-workflow'
import type { ScheduleUiState } from './schedule-ui-state'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Renders the state-driven Scheduled Backup Settings page.
 */
export function SchedulePage() {
  const { language, t } = useI18n()
  const workflow = useScheduleWorkflow()
  const {
    actionResult,
    copyDiagnostics,
    detectSchedule,
    dismissIssue,
    draftDueAfterHours,
    error,
    hasNeverRun,
    intervalDirty,
    lastCheckedAt,
    loading,
    operation,
    plan,
    runNativeAction,
    setDraftDueAfterHours,
    snapshot,
    status,
    uiState,
    visibleIssues,
  } = workflow

  const selectedProfiles = selectedBrowserProfiles(
    snapshot?.browserProfiles ?? [],
    snapshot?.config.selectedProfileIds ?? [],
  )
  const lastBackup =
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt ??
    null

  if (loading || uiState === 'CHECKING') {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
        data-testid="schedule-page"
      >
        <ScheduleHeader t={t} />
        <div className="border-border-light bg-paper rounded-paper flex flex-col items-start gap-2 border px-4 py-4">
          <LoadingState label={t('schedule.detectingStatus')} />
          <p className="text-ink-muted m-0 font-mono text-[11px]">
            {t('schedule.detectingBody')}
          </p>
        </div>
      </div>
    )
  }

  if (!plan || !status) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
        data-testid="schedule-page"
      >
        <ScheduleHeader t={t} />
        <StatusCallout
          tone="blocked"
          title={t('schedule.unavailableTitle')}
          body={error ?? t('schedule.unavailableBody')}
          actions={
            <button
              className="border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px]"
              type="button"
              onClick={() => {
                void detectSchedule('detect')
              }}
            >
              {t('schedule.redetect')}
            </button>
          }
        />
      </div>
    )
  }

  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
      data-testid="schedule-page"
    >
      <ScheduleHeader t={t} />
      <ScheduleStatusBar
        actionResult={actionResult}
        lastCheckedAt={lastCheckedAt}
        operation={operation}
        status={status}
        t={t}
        uiState={uiState}
      />

      {uiState === 'NOT_INSTALLED' ? (
        <NotInstalledState
          draftDueAfterHours={draftDueAfterHours}
          intervalDirty={intervalDirty}
          onInstall={() => {
            void runNativeAction('install')
          }}
          onManualComplete={() => {
            void detectSchedule('verify')
          }}
          onRedetect={() => {
            void detectSchedule('detect')
          }}
          onStepAutoRun={() => {
            void runNativeAction('install')
          }}
          onStepVerify={() => {
            void detectSchedule('verify')
          }}
          onUpdateInterval={setDraftDueAfterHours}
          operationActive={operation !== null}
          plan={plan}
          profiles={selectedProfiles}
          snapshot={snapshot}
          snapshotInitialized={Boolean(snapshot?.config.initialized)}
          t={t}
        />
      ) : null}

      {uiState === 'INSTALLED_OK' ? (
        <InstalledOkState
          draftDueAfterHours={draftDueAfterHours}
          hasNeverRun={hasNeverRun}
          intervalDirty={intervalDirty}
          language={language}
          lastBackup={lastBackup}
          onRedetect={() => {
            void detectSchedule('detect')
          }}
          onRemove={() => {
            void runNativeAction('remove')
          }}
          onUpdate={() => {
            void runNativeAction('update')
          }}
          onUpdateInterval={setDraftDueAfterHours}
          onVerify={() => {
            void detectSchedule('verify')
          }}
          operationActive={operation !== null}
          plan={plan}
          profiles={selectedProfiles}
          snapshot={snapshot}
          status={status}
          t={t}
        />
      ) : null}

      {uiState === 'INSTALLED_WARN' ? (
        <InstalledWarnState
          dismissIssue={dismissIssue}
          hasNeverRun={hasNeverRun}
          issues={visibleIssues}
          language={language}
          lastBackup={lastBackup}
          onManualComplete={() => {
            void detectSchedule('verify')
          }}
          onRedetect={() => {
            void detectSchedule('detect')
          }}
          onRemove={() => {
            void runNativeAction('remove')
          }}
          onRepair={() => {
            void runNativeAction('repair')
          }}
          onStepAutoRun={() => {
            void runNativeAction(
              hasLegacyRepair(visibleIssues) ? 'repair' : 'update',
            )
          }}
          onStepVerify={() => {
            void detectSchedule('verify')
          }}
          onUpdate={() => {
            void runNativeAction('update')
          }}
          operationActive={operation !== null}
          plan={plan}
          profiles={selectedProfiles}
          snapshot={snapshot}
          status={status}
          t={t}
        />
      ) : null}

      {uiState === 'INSTALLED_ERROR' ? (
        <InstalledErrorState
          issues={visibleIssues}
          onCopyDiagnostics={() => {
            void copyDiagnostics()
          }}
          onManualComplete={() => {
            void detectSchedule('verify')
          }}
          onRedetect={() => {
            void detectSchedule('detect')
          }}
          onReinstall={() => {
            void runNativeAction('update')
          }}
          onRemove={() => {
            void runNativeAction('remove')
          }}
          onStepAutoRun={() => {
            void runNativeAction('remove')
          }}
          onStepVerify={() => {
            void detectSchedule('verify')
          }}
          operationActive={operation !== null}
          plan={plan}
          status={status}
          t={t}
        />
      ) : null}
    </div>
  )
}

function ScheduleHeader({ t }: { t: Translator }) {
  return (
    <header className="schedule-header">
      <div>
        <div className="summary-label">{t('schedule.backupSchedule')}</div>
        <h2>{t('schedule.pageTitle')}</h2>
        <p className="dashboard-next-action">{t('schedule.pageIntro')}</p>
      </div>
    </header>
  )
}

function ScheduleStatusBar({
  actionResult,
  lastCheckedAt,
  operation,
  status,
  t,
  uiState,
}: {
  actionResult: ReturnType<typeof useScheduleWorkflow>['actionResult']
  lastCheckedAt: Date | null
  operation: ReturnType<typeof useScheduleWorkflow>['operation']
  status: ScheduleStatus
  t: Translator
  uiState: ScheduleUiState
}) {
  return (
    <div
      className={`schedule-status-band schedule-state-${uiState.toLowerCase()}`}
    >
      <div>
        <div className="summary-label">{t('schedule.currentState')}</div>
        <h3>{t(scheduleStateTitleKey(uiState))}</h3>
        <p>{t(scheduleStateBodyKey(uiState))}</p>
      </div>
      <div className="schedule-status-meta">
        <span className="status-badge">
          {t(scheduleStateBadgeKey(uiState))}
        </span>
        <span className="mono">{checkedAtStatusText(lastCheckedAt, t)}</span>
        <span className="mono">{status.label}</span>
      </div>
      {operation ? (
        <div className="schedule-progress" role="status">
          <span>
            {t('schedule.progressStep', {
              current: operation.current,
              total: operation.total,
            })}
          </span>
          <strong>{t(operation.messageKey)}</strong>
        </div>
      ) : null}
      {actionResult ? (
        <div
          className={`schedule-result ${actionResult.status === 'success' ? 'success' : 'error'}`}
          role={actionResult.status === 'error' ? 'alert' : 'status'}
        >
          <strong>
            {actionResult.status === 'success'
              ? t('schedule.operationSucceeded')
              : t('schedule.operationFailed')}
          </strong>
          <span>{translateMaybe(t, actionResult.message)}</span>
          {actionResult.auditPath ? (
            <span className="mono">{actionResult.auditPath}</span>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

function NotInstalledState({
  draftDueAfterHours,
  intervalDirty,
  onInstall,
  onManualComplete,
  onRedetect,
  onStepAutoRun,
  onStepVerify,
  onUpdateInterval,
  operationActive,
  plan,
  profiles,
  snapshot,
  snapshotInitialized,
  t,
}: {
  draftDueAfterHours: number
  intervalDirty: boolean
  onInstall: () => void
  onManualComplete: () => void
  onRedetect: () => void
  onStepAutoRun: () => void
  onStepVerify: () => void
  onUpdateInterval: (hours: number) => void
  operationActive: boolean
  plan: SchedulePlan
  profiles: BrowserProfile[]
  snapshot: AppSnapshot | null
  snapshotInitialized: boolean
  t: Translator
}) {
  return (
    <div className="schedule-state-layout">
      <EncryptedNoKeyringWarning snapshot={snapshot} t={t} />
      <LinuxManualOnlyCallout plan={plan} t={t} />
      <section className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.preInstallConfig')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <div>
            <div className="summary-label">{t('schedule.desiredInterval')}</div>
            <p className="dashboard-next-action">
              {formatSelectedScheduleInterval(t, draftDueAfterHours)}
            </p>
            <BackupIntervalSelector
              customInvalidMessage={t('schedule.intervalCustomInvalid')}
              customLabel={t('schedule.intervalCustomLabel')}
              customUnitLabel={t('schedule.intervalCustomUnit')}
              disabled={operationActive}
              formatLabel={(hours) =>
                t('schedule.intervalChipLabel', { hours })
              }
              value={draftDueAfterHours}
              onChange={onUpdateInterval}
            />
            {intervalDirty ? (
              <p className="mono-support">
                {t('schedule.intervalWillApplyOnInstall')}
              </p>
            ) : null}
          </div>
          <BrowserProfileSummary profiles={profiles} t={t} />
        </div>
      </section>

      <section className="panel schedule-action-panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.installBlockTitle')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <p className="dashboard-next-action">
            {t('schedule.installBlockBody')}
          </p>
          <div className="wizard-actions schedule-action-row">
            <button
              className="btn-primary"
              disabled={
                operationActive || !snapshotInitialized || !plan.applySupported
              }
              type="button"
              onClick={onInstall}
            >
              {t('schedule.autoInstall')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={onRedetect}
            >
              {t('schedule.redetect')}
            </button>
          </div>
          {!snapshotInitialized ? (
            <p className="inline-error">
              {t('schedule.initializeArchiveFirst')}
            </p>
          ) : null}
          <ManualMode
            actionLabel={t('schedule.autoRunAllSteps')}
            manualSteps={manualStepsFor(plan)}
            onAllAutoRun={onInstall}
            onManualComplete={onManualComplete}
            onOpenDirectory={(path) => {
              void backend.openPathInFileManager(path)
            }}
            onStepAutoRun={onStepAutoRun}
            onStepVerify={onStepVerify}
            operationActive={operationActive}
            t={t}
            title={t('schedule.manualInstall')}
          />
        </div>
      </section>
    </div>
  )
}

function InstalledOkState({
  draftDueAfterHours,
  hasNeverRun,
  intervalDirty,
  language,
  lastBackup,
  onRedetect,
  onRemove,
  onUpdate,
  onUpdateInterval,
  onVerify,
  operationActive,
  plan,
  profiles,
  snapshot,
  status,
  t,
}: {
  draftDueAfterHours: number
  hasNeverRun: boolean
  intervalDirty: boolean
  language: ResolvedLanguage
  lastBackup: string | null
  onRedetect: () => void
  onRemove: () => void
  onUpdate: () => void
  onUpdateInterval: (hours: number) => void
  onVerify: () => void
  operationActive: boolean
  plan: SchedulePlan
  profiles: BrowserProfile[]
  snapshot: AppSnapshot | null
  status: ScheduleStatus
  t: Translator
}) {
  return (
    <div className="schedule-state-layout">
      <EncryptedNoKeyringWarning snapshot={snapshot} t={t} />
      <section className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.installedSummary')}</span>
        </div>
        <div className="panel-body">
          <ScheduleSummary
            hasNeverRun={hasNeverRun}
            language={language}
            lastBackup={lastBackup}
            plan={plan}
            profiles={profiles}
            status={status}
            t={t}
          />
        </div>
      </section>
      <section className="panel schedule-action-panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.availableActions')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <div>
            <div className="summary-label">{t('schedule.desiredInterval')}</div>
            <BackupIntervalSelector
              customInvalidMessage={t('schedule.intervalCustomInvalid')}
              customLabel={t('schedule.intervalCustomLabel')}
              customUnitLabel={t('schedule.intervalCustomUnit')}
              disabled={operationActive}
              formatLabel={(hours) =>
                t('schedule.intervalChipLabel', { hours })
              }
              value={draftDueAfterHours}
              onChange={onUpdateInterval}
            />
            {intervalDirty ? (
              <p className="mono-support">
                {t('schedule.intervalChangedBody')}
              </p>
            ) : null}
          </div>
          <div className="wizard-actions schedule-action-row">
            <button
              className="btn-primary"
              disabled={operationActive}
              type="button"
              onClick={onVerify}
            >
              {t('schedule.verifyInstallation')}
            </button>
            <button
              className="btn-secondary"
              disabled={operationActive}
              type="button"
              onClick={onUpdate}
            >
              {intervalDirty
                ? t('schedule.updateInstalledSchedule')
                : t('schedule.modifyInstallation')}
            </button>
            <button
              className="btn-secondary"
              disabled={operationActive}
              type="button"
              onClick={onRemove}
            >
              {t('schedule.removeInstalledSchedule')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={onRedetect}
            >
              {t('schedule.redetect')}
            </button>
          </div>
          <InstallDetails plan={plan} status={status} t={t} />
        </div>
      </section>
    </div>
  )
}

function InstalledWarnState({
  dismissIssue,
  hasNeverRun,
  issues,
  language,
  lastBackup,
  onManualComplete,
  onRedetect,
  onRemove,
  onRepair,
  onStepAutoRun,
  onStepVerify,
  onUpdate,
  operationActive,
  plan,
  profiles,
  snapshot,
  status,
  t,
}: {
  dismissIssue: (code: string) => void
  hasNeverRun: boolean
  issues: ScheduleIssue[]
  language: ResolvedLanguage
  lastBackup: string | null
  onManualComplete: () => void
  onRedetect: () => void
  onRemove: () => void
  onRepair: () => void
  onStepAutoRun: () => void
  onStepVerify: () => void
  onUpdate: () => void
  operationActive: boolean
  plan: SchedulePlan
  profiles: BrowserProfile[]
  snapshot: AppSnapshot | null
  status: ScheduleStatus
  t: Translator
}) {
  return (
    <div className="schedule-state-layout">
      <EncryptedNoKeyringWarning snapshot={snapshot} t={t} />
      <section className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.problemSummary')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          {issues.length > 0 ? (
            <IssueList dismissIssue={dismissIssue} issues={issues} t={t} />
          ) : hasNeverRun ? (
            <StatusCallout
              tone="warning"
              title={t('schedule.issueNeverRunTitle')}
              body={t('schedule.issueNeverRunDetail')}
            />
          ) : (
            <StatusCallout
              tone="warning"
              title={t('schedule.issueNeedsReviewTitle')}
              body={t('schedule.issueNeedsReviewDetail')}
            />
          )}
          <ScheduleSummary
            hasNeverRun={hasNeverRun}
            language={language}
            lastBackup={lastBackup}
            plan={plan}
            profiles={profiles}
            status={status}
            t={t}
          />
        </div>
      </section>
      <section className="panel schedule-action-panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.recoveryActions')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <div className="wizard-actions schedule-action-row">
            {hasLegacyRepair(issues) ? (
              <button
                className="btn-primary"
                disabled={operationActive}
                type="button"
                onClick={onRepair}
              >
                {t('schedule.repairLegacy')}
              </button>
            ) : null}
            <button
              className="btn-primary"
              disabled={operationActive}
              type="button"
              onClick={onUpdate}
            >
              {t('schedule.reinstallSchedule')}
            </button>
            <button
              className="btn-secondary"
              disabled={operationActive}
              type="button"
              onClick={onRemove}
            >
              {t('schedule.removeInstalledSchedule')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={onRedetect}
            >
              {t('schedule.redetect')}
            </button>
          </div>
          <ManualMode
            actionLabel={
              hasLegacyRepair(issues)
                ? t('schedule.repairLegacy')
                : t('schedule.reinstallSchedule')
            }
            manualSteps={manualStepsFor(plan)}
            onAllAutoRun={hasLegacyRepair(issues) ? onRepair : onUpdate}
            onManualComplete={onManualComplete}
            onOpenDirectory={(path) => {
              void backend.openPathInFileManager(path)
            }}
            onStepAutoRun={onStepAutoRun}
            onStepVerify={onStepVerify}
            operationActive={operationActive}
            t={t}
            title={
              hasLegacyRepair(issues)
                ? t('schedule.manualRepair')
                : t('schedule.manualInstall')
            }
          />
        </div>
      </section>
    </div>
  )
}

function InstalledErrorState({
  issues,
  onCopyDiagnostics,
  onManualComplete,
  onRedetect,
  onReinstall,
  onRemove,
  onStepAutoRun,
  onStepVerify,
  operationActive,
  plan,
  status,
  t,
}: {
  issues: ScheduleIssue[]
  onCopyDiagnostics: () => void
  onManualComplete: () => void
  onRedetect: () => void
  onReinstall: () => void
  onRemove: () => void
  onStepAutoRun: () => void
  onStepVerify: () => void
  operationActive: boolean
  plan: SchedulePlan
  status: ScheduleStatus
  t: Translator
}) {
  return (
    <div className="schedule-state-layout">
      <section className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.errorSummary')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <IssueList issues={issues} t={t} />
          {issues.length === 0 ? (
            <StatusCallout
              tone="blocked"
              title={t('schedule.issueInspectionFailedTitle')}
              body={t('schedule.issueInspectionFailedConsequence')}
            />
          ) : null}
          <VerificationList status={status} t={t} />
        </div>
      </section>
      <section className="panel schedule-action-panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.recoveryActions')}</span>
        </div>
        <div className="panel-body schedule-config-stack">
          <div className="wizard-actions schedule-action-row">
            <button
              className="btn-primary"
              disabled={operationActive}
              type="button"
              onClick={onReinstall}
            >
              {t('schedule.reinstallSchedule')}
            </button>
            <button
              className="btn-secondary"
              disabled={operationActive}
              type="button"
              onClick={onRemove}
            >
              {t('schedule.manualRemove')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={onRedetect}
            >
              {t('schedule.redetect')}
            </button>
            <button
              className="btn-secondary"
              disabled={operationActive}
              type="button"
              onClick={onCopyDiagnostics}
            >
              {t('schedule.copyDiagnostics')}
            </button>
          </div>
          <ManualMode
            actionLabel={t('schedule.manualRemove')}
            manualSteps={manualStepsFor(plan)}
            onAllAutoRun={onRemove}
            onManualComplete={onManualComplete}
            onOpenDirectory={(path) => {
              void backend.openPathInFileManager(path)
            }}
            onStepAutoRun={onStepAutoRun}
            onStepVerify={onStepVerify}
            operationActive={operationActive}
            t={t}
            title={t('schedule.manualRemovePath')}
          />
        </div>
      </section>
    </div>
  )
}

function BrowserProfileSummary({
  profiles,
  t,
}: {
  profiles: BrowserProfile[]
  t: Translator
}) {
  return (
    <div className="schedule-profile-summary">
      <div className="schedule-profile-heading">
        <div>
          <div className="summary-label">{t('schedule.backupScope')}</div>
          <p className="dashboard-next-action">
            {t('schedule.backupScopeBody')}
          </p>
        </div>
        <Link className="btn-secondary" to="/settings#settings-profiles">
          {t('schedule.editProfilesInSettings')}
        </Link>
      </div>
      <div className="schedule-profile-list">
        {profiles.length > 0 ? (
          profiles.map((profile) => (
            <div className="schedule-profile-pill" key={profile.profileId}>
              <BrowserIcon browserName={profile.browserName} />
              <span>{profile.browserName}</span>
              <span className="mono">{profile.profileName}</span>
            </div>
          ))
        ) : (
          <p className="mono-support">{t('schedule.noProfilesSelected')}</p>
        )}
      </div>
    </div>
  )
}

function ScheduleSummary({
  hasNeverRun,
  language,
  lastBackup,
  plan,
  profiles,
  status,
  t,
}: {
  hasNeverRun: boolean
  language: ResolvedLanguage
  lastBackup: string | null
  plan: SchedulePlan
  profiles: BrowserProfile[]
  status: ScheduleStatus
  t: Translator
}) {
  const rows = [
    [
      t('schedule.interval'),
      formatScheduleSummaryInterval(t, status.dueAfterHours),
    ],
    [t('schedule.mechanism'), plan.platform],
    [
      t('schedule.lastTriggered'),
      lastBackupLabel({ hasNeverRun, language, lastBackup, t }),
    ],
    [t('schedule.label'), status.label],
    [
      t('schedule.profiles'),
      profiles.length > 0
        ? profiles
            .map((profile) => `${profile.browserName}:${profile.profileName}`)
            .join(', ')
        : t('schedule.noProfilesSelected'),
    ],
  ]
  return (
    <div className="schedule-summary-grid">
      {rows.map(([label, value]) => (
        <div className="config-row" key={label}>
          <span className="config-label">{label}</span>
          <span className="config-value mono">{value}</span>
        </div>
      ))}
    </div>
  )
}

function IssueList({
  dismissIssue,
  issues,
  t,
}: {
  dismissIssue?: (code: string) => void
  issues: ScheduleIssue[]
  t: Translator
}) {
  return (
    <div className="schedule-issue-list">
      {issues.map((issue) => (
        <div className={`schedule-issue ${issue.severity}`} key={issue.code}>
          <div>
            <strong>{t(issue.titleKey)}</strong>
            <p>{t(issue.detailKey)}</p>
            <p className="mono-support">{t(issue.consequenceKey)}</p>
            {issue.evidence.length > 0 ? (
              <div className="schedule-evidence-list">
                {issue.evidence.map((entry) => (
                  <span className="mono" key={entry}>
                    {entry}
                  </span>
                ))}
              </div>
            ) : null}
          </div>
          {issue.dismissible && dismissIssue ? (
            <button
              className="btn-secondary"
              type="button"
              onClick={() => dismissIssue(issue.code)}
            >
              {t('schedule.ignoreWarning')}
            </button>
          ) : null}
        </div>
      ))}
    </div>
  )
}

function ManualMode({
  actionLabel,
  manualSteps,
  onAllAutoRun,
  onManualComplete,
  onOpenDirectory,
  onStepAutoRun,
  onStepVerify,
  operationActive,
  t,
  title,
}: {
  actionLabel: string
  manualSteps: ScheduleManualStep[]
  onAllAutoRun: () => void
  onManualComplete: () => void
  onOpenDirectory: (path: string) => void
  onStepAutoRun: () => void
  onStepVerify: () => void
  operationActive: boolean
  t: Translator
  title: string
}) {
  return (
    <details className="schedule-manual-mode">
      <summary>{title}</summary>
      <div className="manual-steps schedule-manual-steps">
        <div className="wizard-actions schedule-action-row">
          <button
            className="btn-secondary"
            disabled={operationActive}
            type="button"
            onClick={onAllAutoRun}
          >
            {actionLabel}
          </button>
          <button
            className="btn-secondary"
            disabled={operationActive}
            type="button"
            onClick={onManualComplete}
          >
            {t('schedule.manualComplete')}
          </button>
        </div>
        {manualSteps.map((step, index) => (
          <div className="schedule-manual-step" key={step.id}>
            <span className="step-num-inline mono">{index + 1}</span>
            <div className="schedule-manual-step-body">
              <strong>{t(step.titleKey)}</strong>
              <p>{t(step.summaryKey)}</p>
              <details>
                <summary>{t('schedule.whyThisStep')}</summary>
                <p>{t(step.whyKey)}</p>
              </details>
              {step.command ? (
                <pre className="code-block">
                  <code>{joinCommand(step.command)}</code>
                </pre>
              ) : null}
              {step.filePath ? (
                <div className="code-panel">
                  <div className="summary-label">{step.filePath}</div>
                  {step.fileContents ? (
                    <pre className="code-block">
                      <code>{step.fileContents}</code>
                    </pre>
                  ) : null}
                </div>
              ) : null}
              {step.directoryPath ? (
                <div className="schedule-directory-row">
                  <span className="mono-support">
                    {t('schedule.openDirectoryHint', {
                      path: step.directoryPath,
                    })}
                  </span>
                  <button
                    className="btn-secondary"
                    disabled={operationActive}
                    type="button"
                    onClick={() => onOpenDirectory(step.directoryPath!)}
                  >
                    {t('common.openPath')}
                  </button>
                </div>
              ) : null}
              <div className="wizard-actions schedule-action-row">
                <button
                  className="btn-secondary"
                  disabled={operationActive || !step.canAutoRun}
                  type="button"
                  onClick={onStepAutoRun}
                >
                  {t('schedule.autoRunStep')}
                </button>
                <button
                  className="btn-secondary"
                  disabled={operationActive || !step.canVerify}
                  type="button"
                  onClick={onStepVerify}
                >
                  {t('schedule.verifyStep')}
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </details>
  )
}

function InstallDetails({
  plan,
  status,
  t,
}: {
  plan: SchedulePlan
  status: ScheduleStatus
  t: Translator
}) {
  return (
    <details className="schedule-install-details">
      <summary>{t('schedule.viewInstallDetails')}</summary>
      <VerificationList status={status} t={t} />
      {plan.generatedFiles.map((file) => (
        <div className="code-panel" key={file.relativePath}>
          <div className="summary-label">
            {file.absolutePath ?? file.relativePath}
          </div>
          <pre className="code-block">
            <code>{file.contents}</code>
          </pre>
        </div>
      ))}
    </details>
  )
}

function VerificationList({
  status,
  t,
}: {
  status: ScheduleStatus
  t: Translator
}) {
  const checks = status.verificationChecks ?? []
  return (
    <div className="schedule-verification-list">
      {checks.length > 0 ? (
        checks.map((check) => (
          <div className={`schedule-check ${check.status}`} key={check.key}>
            <strong>{t(check.labelKey)}</strong>
            <span>{t(check.detailKey)}</span>
          </div>
        ))
      ) : (
        <p className="mono-support">{t('schedule.noVerificationChecks')}</p>
      )}
    </div>
  )
}

/**
 * Warns when the archive is encrypted but the password is not saved to the
 * system keychain — background worker runs will fail because they cannot
 * unlock the database without user interaction.
 *
 * Renders nothing when the archive is plaintext or the keyring flag is on.
 */
function EncryptedNoKeyringWarning({
  snapshot,
  t,
}: {
  snapshot: AppSnapshot | null
  t: Translator
}) {
  const navigate = useNavigate()
  if (
    !snapshot ||
    snapshot.config.archiveMode !== 'Encrypted' ||
    snapshot.config.rememberDatabaseKeyInKeyring
  ) {
    return null
  }
  return (
    <StatusCallout
      tone="blocked"
      title={t('schedule.encryptedNoKeyringTitle')}
      body={t('schedule.encryptedNoKeyringBody')}
      actions={
        <button
          className="btn-primary"
          type="button"
          onClick={() => navigate('/security')}
        >
          {t('schedule.encryptedNoKeyringAction')}
        </button>
      }
    />
  )
}

/**
 * Explains that automatic schedule installation is not available on the
 * current platform (Linux) and directs the user to the manual steps below.
 *
 * Renders nothing when `plan.applySupported` is true (macOS / Windows).
 */
function LinuxManualOnlyCallout({
  plan,
  t,
}: {
  plan: SchedulePlan
  t: Translator
}) {
  if (plan.applySupported) return null
  return (
    <StatusCallout
      tone="warning"
      title={t('schedule.linuxManualOnlyTitle')}
      body={t('schedule.linuxManualOnlyBody')}
    />
  )
}

function selectedBrowserProfiles(
  profiles: BrowserProfile[],
  selectedIds: string[],
): BrowserProfile[] {
  const selected = new Set(selectedIds)
  return profiles.filter((profile) => selected.has(profile.profileId))
}

function manualStepsFor(plan: SchedulePlan): ScheduleManualStep[] {
  if (plan.manualStepDetails && plan.manualStepDetails.length > 0) {
    return plan.manualStepDetails
  }
  return plan.manualSteps.map((_, index) => ({
    id: `manual-${index}`,
    titleKey: 'schedule.manualGenericStepTitle',
    summaryKey: 'schedule.manualGenericStepSummary',
    whyKey: 'schedule.manualGenericStepWhy',
    command: plan.applyCommands[index] ?? null,
    filePath: null,
    fileContents: null,
    directoryPath: null,
    canAutoRun: Boolean(plan.applyCommands[index]),
    canVerify: true,
  }))
}

function hasLegacyRepair(issues: ScheduleIssue[]): boolean {
  return issues.some((issue) => issue.repairAction === 'repair-legacy')
}

function checkedAtStatusText(
  lastCheckedAt: Date | null,
  t: Translator,
): string {
  /* v8 ignore next -- status bar only mounts after detection records a timestamp. */
  if (!lastCheckedAt) return t('schedule.notCheckedYet')
  return t('schedule.detectCompleteAt', {
    time: lastCheckedAt.toLocaleTimeString(),
  })
}

function formatSelectedScheduleInterval(
  t: Translator,
  dueAfterHours: number,
): string {
  const minutes = backupIntervalHoursToMinutes(dueAfterHours)
  if (minutes % 60 === 0) {
    return t('schedule.selectedIntervalValue', { hours: minutes / 60 })
  }
  return t('schedule.selectedIntervalValueMinutes', { minutes })
}

function formatScheduleSummaryInterval(
  t: Translator,
  dueAfterHours: number,
): string {
  const minutes = backupIntervalHoursToMinutes(dueAfterHours)
  if (minutes % 60 === 0) {
    return t('schedule.intervalValue', { hours: minutes / 60 })
  }
  return t('schedule.intervalValueMinutes', { minutes })
}

function lastBackupLabel({
  hasNeverRun,
  language,
  lastBackup,
  t,
}: {
  hasNeverRun: boolean
  language: ResolvedLanguage
  lastBackup: string | null
  t: Translator
}): string {
  if (hasNeverRun) return t('schedule.neverRun')
  /* v8 ignore next -- hasNeverRun and lastBackup are derived from the same backend/snapshot fields. */
  if (lastBackup) return formatRelativeTime(lastBackup, language)
  /* v8 ignore next -- hasNeverRun and lastBackup are derived from the same backend/snapshot fields. */
  return t('common.notAvailable')
}

function scheduleStateTitleKey(state: ScheduleUiState): string {
  if (state === 'NOT_INSTALLED') return 'schedule.stateNotInstalledTitle'
  if (state === 'INSTALLED_OK') return 'schedule.stateInstalledOkTitle'
  if (state === 'INSTALLED_WARN') return 'schedule.stateInstalledWarnTitle'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  if (state === 'INSTALLED_ERROR') return 'schedule.stateInstalledErrorTitle'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  return 'schedule.stateCheckingTitle'
}

function scheduleStateBodyKey(state: ScheduleUiState): string {
  if (state === 'NOT_INSTALLED') return 'schedule.stateNotInstalledBody'
  if (state === 'INSTALLED_OK') return 'schedule.stateInstalledOkBody'
  if (state === 'INSTALLED_WARN') return 'schedule.stateInstalledWarnBody'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  if (state === 'INSTALLED_ERROR') return 'schedule.stateInstalledErrorBody'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  return 'schedule.stateCheckingBody'
}

function scheduleStateBadgeKey(state: ScheduleUiState): string {
  if (state === 'NOT_INSTALLED') return 'schedule.notInstalledBadge'
  if (state === 'INSTALLED_OK') return 'schedule.installedBadge'
  if (state === 'INSTALLED_WARN') return 'schedule.attentionBadge'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  if (state === 'INSTALLED_ERROR') return 'schedule.blockedBadge'
  /* v8 ignore next -- CHECKING is rendered before the status bar mounts. */
  return 'schedule.checkingBadge'
}

function joinCommand(command: string[]) {
  return command
    .map((part) => (part.includes(' ') ? `"${part}"` : part))
    .join(' ')
}

function translateMaybe(t: Translator, value: string): string {
  return value.startsWith('schedule.') ? t(value) : value
}
