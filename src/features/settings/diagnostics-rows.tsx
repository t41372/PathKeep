/**
 * Settings → About diagnostics: where the logs and crash reports are (with
 * open and copy), the latest crash, and a metadata-only report to paste into
 * a bug report.
 *
 * PathKeep has no automatic support bundle (SUPPORT.md): the report is
 * built here from facts the app already shows (build, platform, archive
 * mode, feature switches, latest run, paths) and the user sees exactly what
 * is copied before copying. No history, passwords, keys or URLs go in it.
 * Not responsible for writing the logs or reports themselves.
 */
import { useMutation } from '@tanstack/react-query'
import { Copy } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AppBuildInfo, AppSnapshot } from '@/lib/types'
import { reveal } from './reveal'

function useCopy() {
  return (text: string, done: string, failed: string) =>
    navigator.clipboard.writeText(text).then(
      () => toast.success(done),
      (error: unknown) =>
        toast.error(failed, { description: describeError(error) }),
    )
}

/** One labelled path with a copy button. */
function PathLine({ label, path }: { label: string; path: string }) {
  const { t } = useI18n()
  const copy = useCopy()
  return (
    <li className="flex items-center gap-2">
      <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
      <code className="min-w-0 flex-1 font-mono text-xs break-all">{path}</code>
      <Button
        size="icon-xs"
        variant="ghost"
        title={t('settingsAbout.paths.copy')}
        aria-label={t('settingsAbout.paths.copyLabel', { name: label })}
        onClick={() =>
          void copy(
            path,
            t('settingsAbout.paths.copied'),
            t('settingsAbout.paths.copyFailed'),
          )
        }
      >
        <Copy />
      </Button>
    </li>
  )
}

export function LogsRow() {
  const { t } = useI18n()
  const diagnostics = useSnapshot().runtimeDiagnostics
  const revealLogs = useMutation({
    mutationFn: supportClient.revealLogs,
    onError: (error) =>
      toast.error(t('settingsAbout.logs.failed'), {
        description: describeError(error, 'reveal_logs'),
      }),
  })
  return (
    <SettingRow
      title={t('settingsAbout.logs.title')}
      description={t('settingsAbout.logs.description')}
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={revealLogs.isPending}
          onClick={() => revealLogs.mutate()}
        >
          {t('settingsAbout.logs.action')}
        </Button>
      }
    >
      <ul className="flex flex-col gap-1.5 border-t pt-3 text-[13px]">
        <PathLine
          label={t('settingsAbout.logs.folder')}
          path={diagnostics.logDirectory}
        />
        <PathLine
          label={t('settingsAbout.logs.appLog')}
          path={diagnostics.rustLogPath}
        />
        <PathLine
          label={t('settingsAbout.logs.windowLog')}
          path={diagnostics.frontendLogPath}
        />
      </ul>
    </SettingRow>
  )
}

export function CrashReportsRow() {
  const { t } = useI18n()
  const format = useFormat()
  const diagnostics = useSnapshot().runtimeDiagnostics
  const latest = diagnostics.latestCrashReport
  return (
    <SettingRow
      title={t('settingsAbout.crashes.title')}
      description={t('settingsAbout.crashes.description')}
      control={
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            void reveal(
              diagnostics.crashReportsDirectory,
              t('settingsAbout.crashes.failed'),
            )
          }
        >
          {t('settingsAbout.crashes.show')}
        </Button>
      }
    >
      <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
        <ul>
          <PathLine
            label={t('settingsAbout.logs.folder')}
            path={diagnostics.crashReportsDirectory}
          />
        </ul>
        {latest ? (
          <p className="break-words">
            {t('settingsAbout.crashes.latest', {
              when: format.dayAndTime(latest.recordedAt),
              message: latest.message,
            })}
            {latest.fatal && ` ${t('settingsAbout.crashes.closed')}`}
          </p>
        ) : (
          <p className="text-muted-foreground">
            {t('settingsAbout.crashes.none')}
          </p>
        )}
      </div>
    </SettingRow>
  )
}

/**
 * The report text. English on purpose: it is read by maintainers, not shown
 * as app copy. Metadata only, per SUPPORT.md's redaction rules.
 */
function diagnosticsReport(snapshot: AppSnapshot, build: AppBuildInfo | null) {
  const { config, runtimeDiagnostics: diag } = snapshot
  const latestRun = snapshot.recentRuns[0]
  const crash = diag.latestCrashReport
  const on = (value: boolean | undefined) => (value ? 'on' : 'off')
  const lines = [
    `PathKeep ${build ? `${build.version} (${build.gitCommitShort}${build.gitDirty ? ', modified' : ''})` : 'unknown build'}`,
    `Platform: ${navigator.userAgent}`,
    `Archive: ${config.archiveMode}, initialized ${config.initialized ? 'yes' : 'no'}`,
    `App Lock: ${on(snapshot.appLockStatus.enabled)}`,
    `Keyring: ${snapshot.keyringStatus.backend}`,
    `Browsers selected: ${config.selectedProfileIds.length}`,
    `Background work: ${config.ai.jobQueuePaused ? 'paused' : 'running'}, ${config.ai.jobQueueConcurrency} at a time`,
    `AI: ${on(config.ai.enabled)}, semantic search ${on(config.ai.semanticIndexEnabled)}, assistant ${on(config.ai.assistantEnabled)}, MCP ${on(config.ai.mcpEnabled)}`,
    `Online: link previews ${on(config.ogImage?.fetchEnabled)}, page summaries ${on(config.ai.contentFetchEnabled)}`,
    latestRun
      ? `Latest run: #${latestRun.id} ${latestRun.runType ?? 'backup'} ${latestRun.status} at ${latestRun.startedAt}`
      : 'Latest run: none',
    `Logs: ${diag.logDirectory}`,
    `Crash reports: ${diag.crashReportsDirectory}`,
    crash
      ? `Latest crash: ${crash.recordedAt} ${crash.source}${crash.fatal ? ' (fatal)' : ''}: ${crash.message}`
      : 'Latest crash: none',
  ]
  return lines.join('\n')
}

export function DiagnosticsReportRow() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const { buildInfo } = useSession()
  const copy = useCopy()
  const [shown, setShown] = useState(false)
  const report = diagnosticsReport(snapshot, buildInfo)
  return (
    <SettingRow
      title={t('settingsAbout.report.title')}
      description={t('settingsAbout.report.description')}
      control={
        <>
          <Button
            size="sm"
            variant="ghost"
            aria-expanded={shown}
            onClick={() => setShown((value) => !value)}
          >
            {shown
              ? t('settingsAbout.report.hide')
              : t('settingsAbout.report.show')}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              void copy(
                report,
                t('settingsAbout.report.copied'),
                t('settingsAbout.report.copyFailed'),
              )
            }
          >
            {t('settingsAbout.report.action')}
          </Button>
        </>
      }
    >
      {shown && (
        <pre className="max-h-64 overflow-auto rounded-lg bg-muted p-3 font-mono text-xs break-all whitespace-pre-wrap">
          {report}
        </pre>
      )}
    </SettingRow>
  )
}
