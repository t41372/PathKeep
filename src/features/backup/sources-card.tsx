/**
 * Sources: every browser profile PathKeep discovered, with a switch to
 * include it in backups. Also where a missing Safari permission or a failed
 * scan is explained.
 *
 * Not responsible for running backups (see `app/backup-runner`).
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { ScanSearch, ShieldAlert, TriangleAlert } from 'lucide-react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useSourceStats } from '@/features/home/queries'
import { appClient } from '@/lib/backend-client/app'
import { supportClient } from '@/lib/backend-client/support'
import type { SourceStats } from '@/lib/backend-client/sources'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  browserDiscoveryState,
  hasBrowserProfileAccessIssue,
  macosFullDiskAccessSettingsUrl,
} from '@/lib/platform-guidance'
import { queryKeys } from '@/lib/query'
import { useSaveConfig, useSnapshot } from '@/lib/queries/app'
import type { BrowserProfile } from '@/lib/types'

type SourceStatus = 'ok' | 'needsPermission' | 'paused' | 'notFound'

function sourceStatus(
  profile: BrowserProfile,
  selected: boolean,
): SourceStatus {
  if (!profile.historyExists) return 'notFound'
  if (hasBrowserProfileAccessIssue(profile)) return 'needsPermission'
  return selected ? 'ok' : 'paused'
}

const statusStyle: Record<SourceStatus, string> = {
  ok: 'bg-green/12 text-green',
  needsPermission: 'bg-brand-soft text-brand',
  paused: 'bg-muted text-muted-foreground',
  notFound: 'bg-muted text-muted-foreground',
}

function useRescan() {
  const client = useQueryClient()
  const { t } = useI18n()
  return useMutation({
    mutationFn: appClient.getSnapshot,
    onSuccess: (snapshot) => {
      client.setQueryData(queryKeys.snapshot, snapshot)
      toast.success(
        t('backup.sources.rescanDone', {
          count: snapshot.browserProfiles.length,
        }),
      )
    },
    onError: (error) =>
      toast.error(t('backup.sources.rescanFailed'), {
        description: describeError(error, 'app_snapshot'),
      }),
  })
}

function openFullDiskAccess(failed: string) {
  return supportClient
    .openExternalUrl(macosFullDiskAccessSettingsUrl)
    .catch((error) => {
      toast.error(failed, {
        description: describeError(error, 'open_external_url'),
      })
    })
}

function SourceRow({
  profile,
  stat,
  statsState,
  selected,
  busy,
  onToggle,
}: {
  profile: BrowserProfile
  stat: SourceStats | undefined
  statsState: 'loading' | 'ready' | 'failed'
  selected: boolean
  busy: boolean
  onToggle: (next: boolean) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const status = sourceStatus(profile, selected)
  const rescan = useRescan()
  const canGrant =
    status === 'needsPermission' && profile.browserFamily === 'safari'
  const name = `${profile.browserName} ${profile.profileName}`

  return (
    <li className="flex items-center gap-3.5 border-t px-5 py-3 first:border-t-0">
      <BrowserIcon
        browserName={profile.browserName}
        className="size-7"
        decorative
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">
          {profile.browserName}
        </span>
        <span className="truncate text-xs text-muted-foreground">
          {profile.profileName}
        </span>
      </div>
      <div className="hidden min-w-24 flex-col items-end text-right sm:flex">
        {statsState === 'loading' ? (
          <Skeleton className="h-3.5 w-16" />
        ) : stat ? (
          <>
            <span className="font-mono text-[13px] text-muted-foreground tabular">
              {format.number(stat.visitCount)}
            </span>
            <span className="text-xs text-muted-foreground">
              {stat.lastVisitAt
                ? t('backup.sources.lastVisit', {
                    ago: format.relative(stat.lastVisitAt),
                  })
                : t('backup.sources.noVisits')}
            </span>
          </>
        ) : (
          <span
            className="text-muted-foreground"
            title={
              statsState === 'failed'
                ? t('backup.sources.statsFailed')
                : undefined
            }
          >
            —
          </span>
        )}
      </div>
      {canGrant && (
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            void openFullDiskAccess(t('backup.sources.grantFailed'))
          }
        >
          {t('backup.sources.grant')}
        </Button>
      )}
      {canGrant && (
        <Button
          size="sm"
          variant="ghost"
          disabled={rescan.isPending}
          onClick={() => rescan.mutate()}
        >
          {t('backup.sources.checkAgain')}
        </Button>
      )}
      <Badge
        variant="secondary"
        className={cn(
          'min-w-[76px] justify-center rounded-md border-0 font-normal',
          statusStyle[status],
        )}
        title={
          status === 'needsPermission'
            ? (profile.accessIssue ?? undefined)
            : undefined
        }
      >
        {status === 'needsPermission' && <ShieldAlert />}
        {t(`backup.sources.status.${status}`)}
      </Badge>
      <Switch
        checked={selected && status !== 'notFound'}
        disabled={busy || status === 'notFound'}
        onCheckedChange={onToggle}
        aria-label={t('backup.sources.toggle', { name })}
      />
    </li>
  )
}

function DiscoveryNotice({
  kind,
}: {
  kind: 'full-disk-access' | 'discovery-error' | 'empty'
}) {
  const { t } = useI18n()
  const rescan = useRescan()
  const key =
    kind === 'full-disk-access'
      ? 'fullDiskAccess'
      : kind === 'empty'
        ? 'empty'
        : 'error'
  const Icon =
    kind === 'empty'
      ? ScanSearch
      : kind === 'full-disk-access'
        ? ShieldAlert
        : TriangleAlert
  return (
    <div className="flex items-start gap-3 border-t px-5 py-4 first:border-t-0">
      <Icon
        className={cn(
          'mt-0.5 size-4 shrink-0',
          kind === 'empty' ? 'text-muted-foreground' : 'text-brand',
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-sm font-medium">
          {t(`backup.sources.notice.${key}.title`)}
        </span>
        <span className="text-[13px] text-muted-foreground">
          {t(`backup.sources.notice.${key}.body`)}
        </span>
      </div>
      {kind === 'full-disk-access' && (
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            void openFullDiskAccess(t('backup.sources.grantFailed'))
          }
        >
          {t('backup.sources.grant')}
        </Button>
      )}
      <Button
        size="sm"
        variant="ghost"
        disabled={rescan.isPending}
        onClick={() => rescan.mutate()}
      >
        {kind === 'full-disk-access'
          ? t('backup.sources.checkAgain')
          : t('backup.sources.rescan')}
      </Button>
    </div>
  )
}

export function SourcesCard() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const stats = useSourceStats()
  const save = useSaveConfig()
  const rescan = useRescan()
  const selected = new Set(snapshot.config.selectedProfileIds)
  const statById = new Map(
    (stats.data ?? []).map((stat) => [stat.profileId, stat]),
  )
  const statsState = stats.isPending
    ? 'loading'
    : stats.isError
      ? 'failed'
      : 'ready'
  const discovery = browserDiscoveryState(
    snapshot.browserDiscoveryIssue,
    snapshot.browserProfiles.length,
  )

  function toggle(profileId: string, next: boolean) {
    save.mutate(
      (config) => ({
        ...config,
        selectedProfileIds: next
          ? [...new Set([...config.selectedProfileIds, profileId])]
          : config.selectedProfileIds.filter((id) => id !== profileId),
      }),
      {
        onError: (error) =>
          toast.error(t('backup.sources.saveFailed'), {
            description: describeError(error, 'save_config'),
          }),
      },
    )
  }

  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <header className="flex items-center justify-between px-5 py-3.5">
        <h2 className="text-sm font-semibold">{t('backup.sources.title')}</h2>
        <Button
          size="sm"
          variant="outline"
          disabled={rescan.isPending}
          onClick={() => rescan.mutate()}
        >
          <ScanSearch className={cn(rescan.isPending && 'animate-pulse')} />
          {t('backup.sources.rescan')}
        </Button>
      </header>
      <ul className="border-t">
        {discovery === 'full-disk-access' || discovery === 'discovery-error' ? (
          <DiscoveryNotice kind={discovery} />
        ) : null}
        {discovery === 'empty' && <DiscoveryNotice kind="empty" />}
        {snapshot.browserProfiles.map((profile) => (
          <SourceRow
            key={profile.profileId}
            profile={profile}
            stat={statById.get(profile.profileId)}
            statsState={statsState}
            selected={selected.has(profile.profileId)}
            busy={save.isPending}
            onToggle={(next) => toggle(profile.profileId, next)}
          />
        ))}
      </ul>
    </section>
  )
}
