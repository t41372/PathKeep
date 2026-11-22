/**
 * Fake `schedule_status` and `preview_schedule` for the 自動備份 card. The
 * plans, install states, issues, checks and files follow what the backend
 * emits (`vault-platform/src/scheduler/{macos,windows,linux}.rs` and the
 * worker's attempt issues in `vault-worker/src/schedule.rs`); only the paths,
 * times and evidence are made up.
 */
import type { ScheduleIssue, ScheduleLastAction, ScheduleManualStep, SchedulePlan, ScheduleStatus, ScheduleVerificationCheck, ScheduledBackupAttempt } from '@/lib/types'
import { DAY, NOW } from '~/data/fixture'

export type Platform = 'macos' | 'windows' | 'linux'
export type InstallState = 'installed' | 'not-installed' | 'mismatch' | 'permission-warning' | 'legacy-install-detected' | 'manual-review'
/** Why the backend says `permission-warning`: three causes on macOS, two on Windows. */
export type PermissionCause = 'not-loaded' | 'unreadable' | 'missing-loaded' | 'access-denied' | 'inspection-failed'
/** What the worker's attempt ledger says about recent scheduled runs. */
export type AttemptsCase = 'normal' | 'running' | 'failed' | 'partial' | 'interrupted' | 'overdue' | 'unreadable'

export const LABEL = 'com.yi-ting.pathkeep.backup'
const LEGACY_LABEL = 'dev.codex.pathkeep.backup'
const HOME = '/Users/mei'
const DATA = `${HOME}/Library/Application Support/com.yi-ting.pathkeep`
const MAC_WORKER = '/Applications/PathKeep.app/Contents/MacOS/pathkeep-worker'
const WIN_WORKER = 'C:\\Users\\mei\\AppData\\Local\\PathKeep\\pathkeep-worker.exe'
const LINUX_WORKER = '/usr/lib/pathkeep/pathkeep-worker'
const PLIST = `${HOME}/Library/LaunchAgents/${LABEL}.plist`
const LEGACY_PLIST = `${HOME}/Library/LaunchAgents/${LEGACY_LABEL}.plist`
const TASK = `Task Scheduler:${LABEL}`
/** `config.schedule_check_interval_hours`: the native schedule never wakes less often than this. */
const CHECK_HOURS = 6

const iso = (t: number) => new Date(t).toISOString()
const audit = (action: string, t: number) => `${DATA}/audit/scheduler/${action}-${iso(t).replace(/:/g, '-')}.json`

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b)
}

/** `linux_on_calendar`: the calendar expression and the spacing it really gives. */
function linuxCalendar(minutes: number): [string, number] {
  if (minutes % 60 === 0) {
    const hours = minutes / 60
    if (hours === 1) return ['*-*-* *:00:00', 60]
    if (hours <= 24 && 24 % hours === 0) return [`*-*-* 00/${hours}:00:00`, minutes]
  }
  const step = Math.max(1, gcd(minutes, 60))
  return step === 1 ? ['*-*-* *:*:00', 1] : [`*-*-* *:00/${step}:00`, step]
}

/** Minutes between the scheduler's wake-ups for a backup interval. */
export function wakeMinutes(platform: Platform, hours: number) {
  const minutes = Math.round(Math.min(hours, CHECK_HOURS) * 60)
  return platform === 'linux' ? linuxCalendar(minutes)[1] : minutes
}

function macPlist(wake: number) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
\t<key>Label</key>
\t<string>${LABEL}</string>
\t<key>ProgramArguments</key>
\t<array>
\t\t<string>${MAC_WORKER}</string>
\t\t<string>backup</string>
\t\t<string>--due-only</string>
\t</array>
\t<key>RunAtLoad</key>
\t<true/>
\t<key>StartInterval</key>
\t<integer>${wake * 60}</integer>
\t<key>StandardOutPath</key>
\t<string>${DATA}/schedule/logs/worker.stdout.log</string>
\t<key>StandardErrorPath</key>
\t<string>${DATA}/schedule/logs/worker.stderr.log</string>
</dict>
</plist>`
}

function windowsXml(wake: number) {
  const h = Math.floor(wake / 60)
  const m = wake % 60
  const repetition = h === 0 ? `PT${m}M` : m === 0 ? `PT${h}H` : `PT${h}H${m}M`
  return `<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <UserId>DESKTOP-MEI\\mei</UserId>
    </LogonTrigger>
    <TimeTrigger>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>${repetition}</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <StartBoundary>2026-01-01T09:00:00</StartBoundary>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>DESKTOP-MEI\\mei</UserId>
      <RunLevel>LeastPrivilege</RunLevel>
      <LogonType>InteractiveToken</LogonType>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>${WIN_WORKER}</Command>
      <Arguments>backup --due-only</Arguments>
    </Exec>
  </Actions>
</Task>`
}

const step = (id: string, name: string, extra: Partial<ScheduleManualStep>): ScheduleManualStep => ({
  id,
  titleKey: `schedule.${name}Title`,
  summaryKey: `schedule.${name}Summary`,
  whyKey: `schedule.${name}Why`,
  canAutoRun: false,
  canVerify: true,
  ...extra,
})

/** What `preview_schedule` returns for an interval: the exact files and the same thing as manual steps. */
export function planFor(platform: Platform, hours: number): SchedulePlan {
  const wake = wakeMinutes(platform, hours)
  const base = { platform, label: LABEL, wakeIntervalSeconds: wake * 60, applyCommands: [], rollbackCommands: [] }
  if (platform === 'macos') {
    const plist = macPlist(wake)
    return {
      ...base,
      executablePath: MAC_WORKER,
      generatedFiles: [{ relativePath: `launchd/${LABEL}.plist`, purpose: '', contents: plist }],
      manualSteps: [],
      manualStepDetails: [
        step('macos-save-plist', 'manualMacosSavePlist', { filePath: `~/Library/LaunchAgents/${LABEL}.plist`, fileContents: plist }),
        step('macos-load-launch-agent', 'manualMacosLoad', { command: ['launchctl', 'bootstrap', 'gui/$(id -u)', `~/Library/LaunchAgents/${LABEL}.plist`], canAutoRun: true }),
      ],
      applySupported: true,
    }
  }
  if (platform === 'windows') {
    const xml = windowsXml(wake)
    return {
      ...base,
      executablePath: WIN_WORKER,
      generatedFiles: [{ relativePath: `windows/${LABEL}.task.xml`, purpose: '', contents: xml }],
      manualSteps: [],
      manualStepDetails: [
        step('windows-save-xml', 'manualWindowsSaveXml', { filePath: `windows/${LABEL}.task.xml`, fileContents: xml }),
        step('windows-register-task', 'manualWindowsRegister', { command: ['schtasks', '/Create', '/TN', LABEL, '/XML', `windows/${LABEL}.task.xml`, '/F'], canAutoRun: true }),
      ],
      applySupported: true,
    }
  }
  const [calendar] = linuxCalendar(Math.round(Math.min(hours, CHECK_HOURS) * 60))
  const service = `[Unit]\nDescription=PathKeep backup worker\n\n[Service]\nType=oneshot\nExecStart=${LINUX_WORKER} backup --due-only\n`
  const timer = `[Unit]\nDescription=PathKeep periodic backup\n\n[Timer]\nOnCalendar=${calendar}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n`
  return {
    ...base,
    executablePath: LINUX_WORKER,
    generatedFiles: [
      { relativePath: `systemd/${LABEL}.service`, purpose: '', contents: service },
      { relativePath: `systemd/${LABEL}.timer`, purpose: '', contents: timer },
    ],
    manualSteps: [],
    manualStepDetails: [
      step('linux-copy-service', 'manualLinuxCopyService', { filePath: `~/.config/systemd/user/${LABEL}.service`, fileContents: service }),
      step('linux-copy-timer', 'manualLinuxCopyTimer', { filePath: `~/.config/systemd/user/${LABEL}.timer`, fileContents: timer }),
      step('linux-enable-timer', 'manualLinuxEnable', { command: ['systemctl', '--user', 'enable', '--now', `${LABEL}.timer`] }),
    ],
    applySupported: false,
  }
}

const check = (key: string, status: ScheduleVerificationCheck['status'], name: string, detail: string, evidence: string[] = []): ScheduleVerificationCheck => ({
  key,
  status,
  labelKey: `schedule.${name}`,
  detailKey: `schedule.${detail}`,
  evidence,
})

const issue = (code: string, severity: ScheduleIssue['severity'], name: string, evidence: string[], detail = `${name}Detail`, consequence = `${name}Consequence`): ScheduleIssue => ({
  code,
  severity,
  titleKey: `schedule.${name}Title`,
  detailKey: `schedule.${detail}`,
  consequenceKey: `schedule.${consequence}`,
  evidence,
  dismissible: false,
})

/** Install state, files, issues and checks the platform's status read produces. */
function inspect(platform: Platform, state: InstallState, cause: PermissionCause) {
  const issues: ScheduleIssue[] = []
  const checks: ScheduleVerificationCheck[] = []
  const files: string[] = []
  if (platform === 'linux') {
    issues.push({ ...issue('status-detection-unavailable', 'warning', 'issueStatusDetectionUnavailable', ['linux']), dismissible: true })
    return { state: 'manual-review' as const, issues, checks, files }
  }
  if (platform === 'windows') {
    if (state === 'installed' || state === 'mismatch') files.push(TASK)
    if (state === 'installed') checks.push(check('windows-task-xml', 'ok', 'verifyWindowsTaskXml', 'verifyWindowsTaskXmlOk', [TASK]))
    if (state === 'mismatch') {
      issues.push(issue('windows-task-mismatch', 'warning', 'issueTaskMismatch', ['exit code: 0']))
      checks.push(check('windows-task-xml', 'warning', 'verifyWindowsTaskXml', 'verifyWindowsTaskXmlMismatch', [TASK]))
    }
    if (state === 'not-installed') checks.push(check('windows-task', 'pending', 'verifyWindowsTask', 'verifyWindowsTaskMissing'))
    if (state === 'permission-warning') {
      const denied = cause !== 'inspection-failed'
      issues.push(
        issue(denied ? 'windows-task-access-denied' : 'windows-task-inspection-failed', 'error', 'issueInspectionFailed', [denied ? 'ERROR: Access is denied.' : 'exit code 1'], denied ? 'issueWindowsAccessDeniedDetail' : 'issueWindowsInspectionFailedDetail'),
      )
      checks.push(check('windows-task', 'error', 'verifyWindowsTask', 'verifyWindowsTaskFailed', [denied ? 'ERROR: Access is denied.' : 'ERROR: The system cannot find the path specified.']))
    }
    return { state: state === 'legacy-install-detected' || state === 'manual-review' ? 'installed' : state, issues, checks, files }
  }
  // macOS
  if (state === 'not-installed') checks.push(check('macos-plist-content', 'pending', 'verifyMacosPlist', 'verifyMacosPlistMissing'))
  if (state === 'installed' || state === 'legacy-install-detected') {
    files.push(PLIST)
    checks.push(check('macos-launch-agent-loaded', 'ok', 'verifyMacosLoaded', 'verifyMacosLoadedOk', [LABEL]))
  }
  if (state === 'mismatch') {
    files.push(PLIST)
    issues.push(issue('macos-plist-mismatch', 'warning', 'issuePlistMismatch', [PLIST]))
    checks.push(check('macos-plist-content', 'warning', 'verifyMacosPlist', 'verifyMacosPlistMismatch', [PLIST]))
  }
  if (state === 'permission-warning') {
    if (cause === 'unreadable') {
      files.push(PLIST)
      issues.push(issue('macos-plist-unreadable', 'error', 'issueInspectionFailed', [PLIST], 'issueMacosInspectionFailedDetail'))
    } else if (cause === 'missing-loaded') {
      issues.push(issue('macos-plist-missing-loaded', 'error', 'issuePlistMissingLoaded', [`LaunchAgent:${LABEL}`]))
      checks.push(check('macos-plist-content', 'error', 'verifyMacosPlist', 'verifyMacosPlistMissingLoaded', [`LaunchAgent:${LABEL}`]))
      checks.push(check('macos-launch-agent-loaded', 'warning', 'verifyMacosLoaded', 'verifyMacosLoadedWithoutPlist', [LABEL]))
    } else {
      files.push(PLIST)
      issues.push(issue('macos-launch-agent-not-loaded', 'error', 'issueLaunchAgentNotLoaded', [LABEL]))
      checks.push(check('macos-launch-agent-loaded', 'error', 'verifyMacosLoaded', 'verifyMacosLoadedFailed', [LABEL]))
    }
  }
  if (state === 'legacy-install-detected') {
    files.push(LEGACY_PLIST)
    issues.push(issue('legacy-launch-agent', 'warning', 'issueLegacyAgent', [LEGACY_PLIST]))
    checks.push(check('macos-legacy-agent', 'warning', 'verifyMacosLegacy', 'verifyMacosLegacyFound', [`${LEGACY_LABEL} (loaded)`]))
  }
  return { state: state === 'manual-review' ? 'installed' : state, issues, checks, files }
}

const H = 3600_000
const attempt = (n: number, outcome: ScheduledBackupAttempt['outcome'], at: number): ScheduledBackupAttempt => ({
  id: `attempt-${n}`,
  startedAt: iso(at),
  finishedAt: outcome === 'running' ? null : iso(at + 90_000),
  outcome,
  phase: outcome === 'running' ? 'backup' : 'done',
})

/** The attempt ledger, newest first, and the health issue it raises. */
function ledger(kind: AttemptsCase): { attempts: ScheduledBackupAttempt[]; issue?: ScheduleIssue } {
  const history = (from: number) =>
    (['success', 'skipped', 'success', 'deferred', 'success', 'skipped', 'success', 'success', 'success'] as const).map((outcome, i) => attempt(i + 10, outcome, from - i * 6 * H))
  switch (kind) {
    case 'normal':
      return { attempts: history(NOW - 78 * 60_000) }
    case 'running':
      return { attempts: [attempt(1, 'running', NOW - 60_000), ...history(NOW - 6 * H)] }
    case 'failed':
      return {
        attempts: [attempt(1, 'failed', NOW - 78 * 60_000), ...history(NOW - 6 * H - 78 * 60_000)],
        issue: issue('latest-attempt-failed', 'error', 'issueScheduleAttemptFailed', ['archive locked: the keychain did not return the archive key']),
      }
    case 'partial':
      return { attempts: [attempt(1, 'partial', NOW - 78 * 60_000), ...history(NOW - 6 * H - 78 * 60_000)], issue: issue('latest-attempt-incomplete', 'warning', 'issueScheduleAttemptIncomplete', []) }
    case 'interrupted':
      return {
        attempts: [attempt(1, 'interrupted', NOW - 5 * H), ...history(NOW - 11 * H)],
        issue: issue('worker-interrupted', 'warning', 'issueScheduleAttemptInterrupted', [iso(NOW - 5 * H)]),
      }
    case 'overdue':
      return { attempts: history(NOW - 3 * DAY), issue: issue('scheduled-backup-overdue', 'error', 'issueScheduleOverdue', [iso(NOW - 3 * DAY)]) }
    case 'unreadable':
      return {
        attempts: [],
        issue: issue('attempt-ledger-unreadable', 'warning', 'issueScheduleAttemptLedgerUnreadable', [`${DATA}/schedule/attempts.jsonl: expected value at line 14 column 1`]),
      }
  }
}

/** The last install / remove / repair record each fixture carries. */
function lastActionFor(platform: Platform, state: InstallState, cause: PermissionCause): ScheduleLastAction | null {
  if (platform === 'linux' || state === 'not-installed') return null
  const files = platform === 'windows' ? [TASK] : [PLIST]
  const at = NOW - 9 * DAY
  const outcome = state === 'permission-warning' ? (cause === 'unreadable' || cause === 'inspection-failed' ? 'unknown' : 'failed') : state === 'mismatch' && platform === 'windows' ? 'unknown' : 'ok'
  const action = cause === 'missing-loaded' && state === 'permission-warning' ? 'remove' : 'apply'
  return { action, status: outcome, message: '', at: iso(at), auditPath: audit(action, at), files }
}

export interface ScheduleFixture {
  platform: Platform
  state: InstallState
  cause: PermissionCause
  attempts: AttemptsCase
  /** Saved `dueAfterHours`. */
  hours: number
  /** Overrides the fixture's record after a change made in the dialog. */
  lastAction?: ScheduleLastAction | null
  checkedAt: number
}

/** The `ScheduleStatus` the card, the dialog and the sheet read. */
export function statusFor(f: ScheduleFixture): ScheduleStatus {
  const plan = planFor(f.platform, f.hours)
  const seen = inspect(f.platform, f.state, f.cause)
  const log = f.platform === 'linux' || f.state === 'not-installed' ? { attempts: [] } : ledger(f.attempts)
  const lastSuccess = log.attempts.find((a) => a.outcome === 'success')
  const wake = plan.wakeIntervalSeconds * 1000
  let next: number | null = null
  if (seen.state === 'installed') {
    next = (lastSuccess ? Date.parse(lastSuccess.startedAt) : NOW) + f.hours * H
    while (next <= NOW) next += wake
  }
  return {
    platform: f.platform,
    label: LABEL,
    dueAfterHours: f.hours,
    checkIntervalHours: CHECK_HOURS,
    applySupported: plan.applySupported,
    installState: seen.state,
    detectedFiles: seen.files,
    manualSteps: plan.manualSteps,
    manualStepDetails: plan.manualStepDetails,
    nextScheduledBackupAt: next ? iso(next) : null,
    wakeIntervalSeconds: plan.wakeIntervalSeconds,
    lastScheduledSuccessAt: lastSuccess?.startedAt ?? null,
    recentAttempts: log.attempts,
    warnings: [],
    issues: [...seen.issues, ...('issue' in log && log.issue ? [log.issue] : [])],
    verificationChecks: seen.checks,
    checkedAt: iso(f.checkedAt),
    lastAction: f.lastAction === undefined ? lastActionFor(f.platform, f.state, f.cause) : f.lastAction,
  }
}

export const auditPath = audit
