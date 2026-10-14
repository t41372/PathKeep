/**
 * Sources: every browser profile PathKeep discovered, with a switch to
 * include it in backups. Also where a missing Safari permission or a failed
 * scan is explained.
 *
 * Profiles are grouped by browser (the user's choice, 2026-10-12): a browser
 * with one profile is one row; one with several has a row of its own (how
 * many, their visits, how many are backed up, the worst status) over its
 * profiles, closed when there are more than three and none has a problem.
 * Profiles with no history file are gathered at the end, closed.
 *
 * Not responsible for running backups (see `app/backup-runner`).
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  ChevronRight,
  ScanSearch,
  ShieldAlert,
  TriangleAlert,
} from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useSourceStats } from '@/features/home/queries'
import { appClient } from '@/lib/backend-client/app'
import type { SourceStats } from '@/lib/backend-client/sources'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  browserDiscoveryState,
  hasBrowserProfileAccessIssue,
} from '@/lib/platform-guidance'
import { queryKeys } from '@/lib/query'
import { useSaveConfig, useSnapshot } from '@/lib/queries/app'
import type { BrowserProfile } from '@/lib/types'
import { openFullDiskAccess } from './full-disk-access'

type SourceStatus = 'ok' | 'needsPermission' | 'paused' | 'notFound'

function sourceStatus(
  profile: BrowserProfile,
  selected: boolean,
): SourceStatus {
  if (!profile.historyExists) return 'notFound'
  if (hasBrowserProfileAccessIssue(profile)) return 'needsPermission'
  return selected ? 'ok' : 'paused'
}

/** Which status a group shows: its worst. */
const statusRank: Record<SourceStatus, number> = {
  needsPermission: 3,
  paused: 2,
  notFound: 1,
  ok: 0,
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

function SourceRow({
  profile,
  stat,
  statsState,
  selected,
  busy,
  onToggle,
  nested = false,
}: {
  profile: BrowserProfile
  stat: SourceStats | undefined
  statsState: 'loading' | 'ready' | 'failed'
  selected: boolean
  busy: boolean
  onToggle: (next: boolean) => void
  /** Under its browser's row: the profile's name leads and the icon is left out. */
  nested?: boolean
}) {
  const { t } = useI18n()
  const format = useFormat()
  const status = sourceStatus(profile, selected)
  const rescan = useRescan()
  const canGrant =
    status === 'needsPermission' && profile.browserFamily === 'safari'
  const name = `${profile.browserName} ${profile.profileName}`

  return (
    <li
      className={cn(
        'flex items-center gap-3.5 border-t py-3 pr-5 first:border-t-0',
        nested ? 'pl-[68px]' : 'pl-5',
      )}
    >
      {!nested && (
        <BrowserIcon
          browserName={profile.browserName}
          className="size-7"
          decorative
        />
      )}
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">
          {nested ? profile.profileName : profile.browserName}
        </span>
        {!nested && (
          <span className="truncate text-xs text-muted-foreground">
            {profile.profileName}
          </span>
        )}
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

interface RowProps {
  statById: Map<string, SourceStats>
  statsState: 'loading' | 'ready' | 'failed'
  selected: Set<string>
  busy: boolean
  onToggle: (profileId: string, next: boolean) => void
}

/**
 * One browser's profiles under a row that sums them up; `missing` gathers the
 * profiles with no history file instead.
 */
function ProfileGroup({
  label,
  browserName,
  profiles,
  missing = false,
  hint,
  rows,
}: {
  label: string
  /** For the icon; `null` for the profiles without history. */
  browserName: string | null
  profiles: BrowserProfile[]
  missing?: boolean
  hint?: string
  rows: RowProps
}) {
  const { t } = useI18n()
  const format = useFormat()
  const statuses = profiles.map((profile) =>
    sourceStatus(profile, rows.selected.has(profile.profileId)),
  )
  const worst = statuses.reduce((a, b) =>
    statusRank[b] > statusRank[a] ? b : a,
  )
  const [open, setOpen] = useState(
    !missing && (profiles.length <= 3 || worst === 'needsPermission'),
  )
  const on = profiles.filter(
    (profile) => profile.historyExists && rows.selected.has(profile.profileId),
  ).length
  const visits = profiles.reduce(
    (sum, profile) =>
      sum + (rows.statById.get(profile.profileId)?.visitCount ?? 0),
    0,
  )
  const listId = `sources-${label.replace(/\W+/g, '-')}`
  return (
    <li className="border-t first:border-t-0">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={t(
          open ? 'backup.sources.group.hide' : 'backup.sources.group.show',
          {
            browser: label,
          },
        )}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-3.5 px-5 py-3 text-left transition-colors hover:bg-muted/50"
      >
        <ChevronRight
          className={cn(
            'size-4 shrink-0 text-muted-foreground transition-transform duration-150',
            open && 'rotate-90',
          )}
          aria-hidden
        />
        {browserName && (
          <BrowserIcon
            browserName={browserName}
            className="size-7"
            decorative
          />
        )}
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{label}</span>
          <span className="truncate text-xs text-muted-foreground">
            {missing
              ? hint
              : t('backup.sources.group.profiles', { count: profiles.length })}
          </span>
        </div>
        {!missing && (
          <>
            <span className="hidden min-w-24 text-right font-mono text-[13px] text-muted-foreground tabular sm:block">
              {rows.statsState === 'ready' ? format.number(visits) : '—'}
            </span>
            <span className="text-xs text-muted-foreground">
              {t('backup.sources.group.on', { on, total: profiles.length })}
            </span>
            <Badge
              variant="secondary"
              className={cn(
                'min-w-[76px] justify-center rounded-md border-0 font-normal',
                statusStyle[worst],
              )}
            >
              {worst === 'needsPermission' && <ShieldAlert />}
              {t(`backup.sources.status.${worst}`)}
            </Badge>
          </>
        )}
      </button>
      {open && (
        <ul id={listId} className="border-t">
          {profiles.map((profile) => (
            <SourceRow
              key={profile.profileId}
              profile={profile}
              stat={rows.statById.get(profile.profileId)}
              statsState={rows.statsState}
              selected={rows.selected.has(profile.profileId)}
              busy={rows.busy}
              onToggle={(next) => rows.onToggle(profile.profileId, next)}
              nested={!missing}
            />
          ))}
        </ul>
      )}
    </li>
  )
}

/** Found profiles by browser, in the order they were found; then those without history. */
function groupProfiles(profiles: BrowserProfile[]) {
  const byBrowser = new Map<string, BrowserProfile[]>()
  const missing: BrowserProfile[] = []
  for (const profile of profiles) {
    if (!profile.historyExists) {
      missing.push(profile)
      continue
    }
    const group = byBrowser.get(profile.browserName) ?? []
    group.push(profile)
    byBrowser.set(profile.browserName, group)
  }
  return { browsers: [...byBrowser], missing }
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

  const { browsers, missing } = groupProfiles(snapshot.browserProfiles)
  const rows: RowProps = {
    statById,
    statsState,
    selected,
    busy: save.isPending,
    onToggle: (profileId, next) => toggle(profileId, next),
  }

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
        {browsers.map(([browserName, profiles]) =>
          profiles.length === 1 ? (
            <SourceRow
              key={profiles[0].profileId}
              profile={profiles[0]}
              stat={statById.get(profiles[0].profileId)}
              statsState={statsState}
              selected={selected.has(profiles[0].profileId)}
              busy={save.isPending}
              onToggle={(next) => toggle(profiles[0].profileId, next)}
            />
          ) : (
            <ProfileGroup
              key={browserName}
              label={browserName}
              browserName={browserName}
              profiles={profiles}
              rows={rows}
            />
          ),
        )}
        {missing.length > 0 && (
          <ProfileGroup
            label={t('backup.sources.group.notFound', {
              count: missing.length,
            })}
            browserName={null}
            profiles={missing}
            missing
            hint={t('backup.sources.group.notFoundHint')}
            rows={rows}
          />
        )}
      </ul>
    </section>
  )
}
