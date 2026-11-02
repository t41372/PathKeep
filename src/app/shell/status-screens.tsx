/**
 * Full-screen states that replace the app before it is usable: boot,
 * boot failure, the one-time archive upgrade, a schema update while the
 * archive opens, and launch recovery.
 */
import { useState } from 'react'
import { backdropDragRegion } from '@/components/app/drag-region'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { subscribeToArchiveUpgradeProgress } from '@/lib/ipc/archive-upgrade-progress'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type {
  AppSnapshot,
  ArchiveConversionAssessment,
  ArchiveUpgradeProgress,
} from '@/lib/types'
import { parseRecoveryReport } from '../recovery-report'
import { useSession } from '../session'

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div
      {...backdropDragRegion()}
      className="fixed inset-0 flex items-center justify-center p-8"
    >
      <div className="flex w-full max-w-md animate-rise flex-col items-center gap-3 text-center">
        <BrandMark className="mb-2 size-12" />
        {children}
      </div>
    </div>
  )
}

export function BootScreen() {
  const { t } = useI18n()
  return (
    <Centered>
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner />
        {t('shell.boot.loading')}
      </div>
    </Centered>
  )
}

/** Shown while a schema update runs as the archive opens; see `useSchemaUpdate`. */
export function SchemaUpdateScreen({
  progress,
}: {
  progress: ArchiveUpgradeProgress
}) {
  const { t } = useI18n()
  const format = useFormat()
  return (
    <Centered>
      <h1 className="text-lg font-semibold">
        {t('shell.upgrade.schema.title')}
      </h1>
      <p className="text-sm text-muted-foreground">
        {t('shell.upgrade.schema.body')}
      </p>
      <div className="mt-2 flex w-full flex-col gap-2" role="status">
        <Progress
          value={
            progress.total > 0
              ? (progress.processed / progress.total) * 100
              : undefined
          }
          className="h-1.5"
        />
        {progress.total > 0 && (
          <span className="text-xs text-muted-foreground tabular">
            {t('shell.upgrade.progress', {
              done: format.number(progress.processed),
              total: format.number(progress.total),
            })}
          </span>
        )}
      </div>
    </Centered>
  )
}

export function BootErrorScreen({ message }: { message: string }) {
  const { t } = useI18n()
  const session = useSession()
  return (
    <Centered>
      <h1 className="text-lg font-semibold">{t('shell.boot.errorTitle')}</h1>
      <pre className="max-h-48 w-full overflow-auto rounded-lg bg-muted p-3 text-left font-mono text-xs whitespace-pre-wrap text-muted-foreground">
        {message}
      </pre>
      <div className="flex gap-2">
        <Button onClick={() => void session.refresh()}>
          {t('shell.boot.retry')}
        </Button>
        <Button
          variant="outline"
          onClick={() => void supportClient.revealLogs()}
        >
          {t('shell.boot.revealLogs')}
        </Button>
      </div>
    </Centered>
  )
}

/**
 * Converting an archive still in the old SQLCipher format. It must happen
 * before the archive can open; the old files are copied, never changed, and
 * an interrupted run picks up where it stopped.
 */
export function UpgradeScreen() {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const conversion = session.upgrade?.conversion ?? null
  const needsPassword = conversion?.sourceEncrypted ?? false
  const [password, setPassword] = useState('')
  const [progress, setProgress] = useState<ArchiveUpgradeProgress | null>(null)
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const [checking, setChecking] = useState(false)
  // Files the password doesn't open, shown for a decision before anything starts.
  const [locked, setLocked] = useState<
    ArchiveConversionAssessment['filesLocked'] | null
  >(null)

  const check = async () => {
    setChecking(true)
    setError(null)
    try {
      const assessment = await archiveClient.assessArchiveUpgrade(password)
      const filesLocked = assessment.conversion?.filesLocked ?? []
      if (filesLocked.length > 0) setLocked(filesLocked)
      else await start()
    } catch (checkError) {
      setError(checkError)
    } finally {
      setChecking(false)
    }
  }

  const start = async () => {
    const snapshot = queryClient.getQueryData<AppSnapshot>(queryKeys.snapshot)
    if (!snapshot) return
    setRunning(true)
    setError(null)
    const unsubscribe = await subscribeToArchiveUpgradeProgress(setProgress)
    try {
      session.enter(
        await archiveClient.initializeArchive(
          snapshot.config,
          needsPassword ? password : null,
        ),
      )
    } catch (upgradeError) {
      // Something the old app left half-done turned out to be beyond repair:
      // boot again, which lands on the recovery screen.
      if (parseRecoveryReport(upgradeError)) {
        await session.restart()
        return
      }
      setError(upgradeError)
    } finally {
      unsubscribe?.()
      setRunning(false)
    }
  }

  const percent =
    progress && progress.total > 0
      ? (progress.processed / progress.total) * 100
      : null
  const wrongPassword =
    error !== null && commandErrorCode(error) === 'archive-password-wrong'

  return (
    <Centered>
      <h1 className="text-lg font-semibold">{t('shell.upgrade.title')}</h1>
      <p className="text-sm text-muted-foreground">{t('shell.upgrade.body')}</p>
      {conversion && (
        <dl className="grid w-full grid-cols-[1fr_auto] gap-x-4 gap-y-1 rounded-lg bg-muted px-4 py-3 text-left text-[13px]">
          <dt className="text-muted-foreground">{t('shell.upgrade.size')}</dt>
          <dd className="tabular">{format.bytes(conversion.sourceBytes)}</dd>
          <dt className="text-muted-foreground">
            {t('shell.upgrade.freeSpace')}
          </dt>
          <dd className="tabular">
            {format.bytes(conversion.requiredFreeBytes)}
          </dd>
          <dd className="col-span-2 pt-1 text-muted-foreground">
            {t('shell.upgrade.kept')}
            {conversion.resumable && ` ${t('shell.upgrade.resumable')}`}
          </dd>
        </dl>
      )}
      {locked && !running ? (
        <div
          role="alertdialog"
          aria-labelledby="upgrade-locked-title"
          className="mt-2 flex w-full flex-col gap-3 rounded-lg border border-border px-4 py-3 text-left"
        >
          <h2 id="upgrade-locked-title" className="text-sm font-semibold">
            {t('shell.upgrade.locked.title')}
          </h2>
          <p className="text-[13px] text-muted-foreground">
            {t('shell.upgrade.locked.body')}
          </p>
          <ul className="flex flex-col gap-1 text-[13px]">
            {locked.map((file) => (
              <li key={file.path}>
                {t(`shell.upgrade.locked.files.${file.database}`)}
                <span className="block font-mono text-xs break-all text-muted-foreground">
                  {file.path}
                </span>
              </li>
            ))}
          </ul>
          <p className="text-[13px] text-muted-foreground">
            {t('shell.upgrade.locked.kept')}
          </p>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setLocked(null)}>
              {t('shell.upgrade.locked.back')}
            </Button>
            <Button onClick={() => void start()}>
              {t('shell.upgrade.locked.continue')}
            </Button>
          </div>
        </div>
      ) : running ? (
        <div className="mt-2 flex w-full flex-col gap-2">
          <Progress value={percent ?? undefined} className="h-1.5" />
          <span className="text-xs text-muted-foreground tabular">
            {progress && t(`shell.upgrade.phases.${progress.phase}`)}
            {progress?.table && ` · ${progress.table}`}
            {progress && progress.total > 0
              ? ` · ${t('shell.upgrade.progress', {
                  done: format.number(progress.processed),
                  total: format.number(progress.total),
                })}`
              : ''}
          </span>
        </div>
      ) : (
        <form
          className="mt-2 flex w-full flex-col items-stretch gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            if (!needsPassword) void start()
            else if (password) void check()
          }}
        >
          {needsPassword && (
            <div className="flex flex-col gap-1.5 text-left">
              <label htmlFor="upgrade-password" className="text-[13px]">
                {t('shell.upgrade.password')}
              </label>
              <Input
                id="upgrade-password"
                type="password"
                autoComplete="current-password"
                value={password}
                aria-invalid={wrongPassword}
                aria-describedby="upgrade-password-hint"
                onChange={(event) => setPassword(event.target.value)}
              />
              <p
                id="upgrade-password-hint"
                className="text-xs text-muted-foreground"
              >
                {t('shell.upgrade.passwordHint')}
              </p>
            </div>
          )}
          <Button
            type="submit"
            disabled={checking || (needsPassword && !password)}
          >
            {checking && <Spinner />}
            {checking ? t('shell.upgrade.checking') : t('shell.upgrade.start')}
          </Button>
        </form>
      )}
      {error !== null && (
        <p role="alert" className="text-sm text-destructive">
          {wrongPassword
            ? t('shell.upgrade.wrongPassword')
            : `${t('shell.upgrade.failed')} ${describeError(error, 'initialize_archive')}`}
        </p>
      )}
    </Centered>
  )
}

export function RecoveryScreen() {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const [restoring, setRestoring] = useState<string | null>(null)
  const [key, setKey] = useState('')
  const [error, setError] = useState<string | null>(null)
  const snapshots = session.recovery?.recoverySnapshots ?? []
  const needsKey = snapshots.some((snapshot) => snapshot.encrypted)

  const restore = async (snapshotPath: string) => {
    setRestoring(snapshotPath)
    setError(null)
    try {
      await archiveClient.runFullArchiveRestore({ snapshotPath }, key || null)
      await session.refresh()
    } catch (restoreError) {
      setError(describeError(restoreError, 'run_full_archive_restore'))
    } finally {
      setRestoring(null)
    }
  }

  return (
    <Centered>
      <h1 className="text-lg font-semibold">{t('shell.recovery.title')}</h1>
      <p className="text-sm text-muted-foreground">
        {t('shell.recovery.body')}
      </p>
      {snapshots.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('shell.recovery.none')}
        </p>
      ) : (
        <div className="mt-2 flex w-full flex-col gap-2">
          {needsKey && (
            <Input
              type="password"
              value={key}
              onChange={(event) => setKey(event.target.value)}
              placeholder={t('shell.lock.password')}
              aria-label={t('shell.lock.password')}
            />
          )}
          {snapshots.map((snapshot) => (
            <div
              key={snapshot.id}
              className="flex items-center justify-between gap-3 rounded-lg border bg-card px-3 py-2 text-left"
            >
              <span className="text-sm">
                {t('shell.recovery.snapshot', {
                  date: snapshot.createdAt
                    ? format.date(snapshot.createdAt, {
                        dateStyle: 'medium',
                        timeStyle: 'short',
                      })
                    : snapshot.label,
                })}
                <span className="block text-xs text-muted-foreground">
                  {format.bytes(snapshot.sizeBytes)}
                </span>
              </span>
              <Button
                size="sm"
                disabled={restoring !== null || (snapshot.encrypted && !key)}
                onClick={() => void restore(snapshot.path)}
              >
                {restoring === snapshot.path
                  ? t('shell.recovery.restoring')
                  : t('shell.recovery.restore')}
              </Button>
            </div>
          ))}
        </div>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <Button
        variant="ghost"
        size="sm"
        onClick={() => void supportClient.revealLogs()}
      >
        {t('shell.boot.revealLogs')}
      </Button>
    </Centered>
  )
}
