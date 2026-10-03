/**
 * App updates: check, download and install, then restart. Uses the helpers in
 * `lib/update.ts`, which already handle builds and previews that cannot update
 * themselves (those get a link to the releases page instead).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { AppUpdateCheckResult, UpdateInstallState } from '@/lib/types'
import {
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  relaunchAfterUpdate,
  RELEASES_PAGE_URL,
} from '@/lib/update'

type State =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'checked'; result: AppUpdateCheckResult }
  | { kind: 'installing'; progress: UpdateInstallState }
  | { kind: 'installed'; version: string }
  | { kind: 'failed'; message: string }

export function UpdateRow({ version }: { version: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const [state, setState] = useState<State>({ kind: 'idle' })

  const check = async () => {
    setState({ kind: 'checking' })
    try {
      setState({ kind: 'checked', result: await checkForAppUpdate(version) })
    } catch (error) {
      setState({ kind: 'failed', message: describeError(error) })
    }
  }

  const install = async (result: AppUpdateCheckResult) => {
    const pending = result.pendingUpdate
    if (!pending) return
    setState({
      kind: 'installing',
      progress: { phase: 'downloading', version: pending.version },
    })
    const final = await downloadAndInstallAppUpdate(pending, (progress) =>
      setState({ kind: 'installing', progress }),
    )
    setState(
      final.phase === 'installed'
        ? { kind: 'installed', version: pending.version }
        : { kind: 'failed', message: final.message ?? '' },
    )
  }

  const openReleases = (url?: string | null) =>
    void supportClient
      .openExternalUrl(url ?? RELEASES_PAGE_URL)
      .catch((error) =>
        toast.error(t('settingsAbout.update.openFailed'), {
          description: describeError(error, 'open_external_url'),
        }),
      )

  const restart = () =>
    void relaunchAfterUpdate().catch((error) =>
      toast.error(t('settingsAbout.update.restartFailed'), {
        description: describeError(error, 'relaunch_after_update'),
      }),
    )

  let description: React.ReactNode = t('settingsAbout.update.description')
  let control: React.ReactNode = (
    <Button size="sm" variant="outline" onClick={() => void check()}>
      {t('settingsAbout.update.check')}
    </Button>
  )
  let below: React.ReactNode = null

  switch (state.kind) {
    case 'checking':
      control = (
        <Button size="sm" variant="outline" disabled>
          <Spinner />
          {t('settingsAbout.update.checking')}
        </Button>
      )
      break
    case 'checked': {
      const { availability, pendingUpdate } = state.result
      if (!availability.supported) {
        description = t('settingsAbout.update.unsupported')
        control = (
          <Button
            size="sm"
            variant="outline"
            onClick={() => openReleases(availability.downloadUrl)}
          >
            {t('settingsAbout.update.releases')}
          </Button>
        )
      } else if (availability.error) {
        description = (
          <span className="text-destructive">
            {t('settingsAbout.update.failed', { message: availability.error })}
          </span>
        )
      } else if (pendingUpdate) {
        description = t('settingsAbout.update.available', {
          version: pendingUpdate.version,
        })
        control = (
          <Button size="sm" onClick={() => void install(state.result)}>
            {t('settingsAbout.update.install')}
          </Button>
        )
        if (pendingUpdate.notes) {
          below = (
            <pre className="max-h-40 overflow-y-auto rounded-lg bg-muted p-3 font-sans text-xs whitespace-pre-wrap text-muted-foreground">
              {pendingUpdate.notes}
            </pre>
          )
        }
      } else {
        description = t('settingsAbout.update.upToDate')
      }
      break
    }
    case 'installing': {
      const { downloadedBytes, contentLength, phase } = state.progress
      description =
        phase === 'installing'
          ? t('settingsAbout.update.installing')
          : t('settingsAbout.update.downloading')
      control = null
      below = (
        <div role="status" className="flex flex-col gap-1.5">
          <Progress
            aria-label={t('settingsAbout.update.downloading')}
            value={
              contentLength
                ? Math.round(((downloadedBytes ?? 0) / contentLength) * 100)
                : undefined
            }
          />
          {contentLength ? (
            <span className="font-mono text-xs text-muted-foreground">
              {format.bytes(downloadedBytes ?? 0)} /{' '}
              {format.bytes(contentLength)}
            </span>
          ) : null}
        </div>
      )
      break
    }
    case 'installed':
      description = t('settingsAbout.update.installed', {
        version: state.version,
      })
      control = (
        <Button size="sm" onClick={restart}>
          {t('settingsAbout.update.restart')}
        </Button>
      )
      break
    case 'failed':
      description = (
        <span className="text-destructive">
          {t('settingsAbout.update.failed', { message: state.message })}
        </span>
      )
      break
  }

  return (
    <SettingRow
      title={t('settingsAbout.update.title')}
      description={description}
      control={control}
    >
      {below}
    </SettingRow>
  )
}
