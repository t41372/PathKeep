/**
 * Full-screen states that replace the app before it is usable: boot,
 * boot failure, the one-time archive upgrade, and launch recovery.
 */
import { useState } from 'react'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { subscribeToArchiveUpgradeProgress } from '@/lib/ipc/archive-upgrade-progress'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type { AppSnapshot, ArchiveUpgradeProgress } from '@/lib/types'
import { useSession } from '../session'

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="fixed inset-0 flex items-center justify-center p-8">
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

export function UpgradeScreen() {
  const { t } = useI18n()
  const session = useSession()
  const [progress, setProgress] = useState<ArchiveUpgradeProgress | null>(null)
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const start = async () => {
    const snapshot = queryClient.getQueryData<AppSnapshot>(queryKeys.snapshot)
    if (!snapshot) return
    setRunning(true)
    setError(null)
    const unsubscribe = await subscribeToArchiveUpgradeProgress(setProgress)
    try {
      session.enter(await archiveClient.initializeArchive(snapshot.config))
    } catch (upgradeError) {
      setError(describeError(upgradeError, 'initialize_archive'))
    } finally {
      unsubscribe?.()
      setRunning(false)
    }
  }

  const percent =
    progress && progress.total > 0
      ? (progress.processed / progress.total) * 100
      : null

  return (
    <Centered>
      <h1 className="text-lg font-semibold">{t('shell.upgrade.title')}</h1>
      <p className="text-sm text-muted-foreground">{t('shell.upgrade.body')}</p>
      {running ? (
        <div className="mt-2 flex w-full flex-col gap-2">
          <Progress value={percent ?? undefined} className="h-1.5" />
          <span className="text-xs text-muted-foreground tabular">
            {progress?.phaseLabel}{' '}
            {progress && progress.total > 0
              ? t('shell.upgrade.progress', {
                  done: progress.processed,
                  total: progress.total,
                })
              : ''}
          </span>
        </div>
      ) : (
        <Button className="mt-2" onClick={() => void start()}>
          {t('shell.upgrade.start')}
        </Button>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {t('shell.upgrade.failed')} {error}
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
