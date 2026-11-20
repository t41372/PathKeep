/**
 * Browsing history's pane in the sources step: the browser profiles the
 * snapshot discovered, each with its history file's size. Explains Safari's
 * Full Disk Access and lets the user check again after granting it; a
 * profile that becomes readable then is left for the user to tick.
 *
 * Not responsible for saving the selection (the page does, on Continue) or
 * the pane's header and list (`source-pane.tsx`).
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { ScanSearch, ShieldAlert, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { appClient } from '@/lib/backend-client/app'
import { supportClient } from '@/lib/backend-client/support'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  browserDiscoveryState,
  hasSafariAccessIssue,
  isBrowserProfileReadable,
  macosFullDiskAccessSettingsUrl,
} from '@/lib/platform-guidance'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import type { BrowserProfile } from '@/lib/types'
import { listedProfiles } from './draft'
import { InlineError } from './inline-error'
import { PaneHeader, SourceList } from './source-pane'
import { GROUP_AFTER, summaryText, type KindCounts } from './sources-model'

function useRecheck() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: appClient.getSnapshot,
    onSuccess: (snapshot) => client.setQueryData(queryKeys.snapshot, snapshot),
  })
}

export function BrowserSources({
  headingId,
  title,
  counts,
  selected,
  onChange,
  filter,
  onFilter,
}: {
  headingId: string
  title: string
  counts: KindCounts
  selected: string[]
  onChange: (selected: string[]) => void
  filter: string
  onFilter: (filter: string) => void
}) {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const recheck = useRecheck()
  const [openFailed, setOpenFailed] = useState<string | null>(null)
  const profiles = listedProfiles(snapshot.browserProfiles)
  const discovery = browserDiscoveryState(
    snapshot.browserDiscoveryIssue,
    profiles.length,
  )
  const needsAccess =
    discovery === 'full-disk-access' ||
    hasSafariAccessIssue(snapshot.browserProfiles)
  const chosen = new Set(selected)
  const items = profiles.map((profile) => ({
    id: profile.profileId,
    group: profile.browserName,
    text: `${profile.browserName} ${profile.profileName}`,
    profile,
  }))

  function set(ids: string[], on: boolean) {
    const next = new Set(selected)
    for (const id of ids) {
      if (on) next.add(id)
      else next.delete(id)
    }
    onChange([...next])
  }

  async function openSettings() {
    setOpenFailed(null)
    try {
      await supportClient.openExternalUrl(macosFullDiskAccessSettingsUrl)
    } catch (error) {
      setOpenFailed(describeError(error, 'open_external_url'))
    }
  }

  const recheckButton = (
    <Button
      size="sm"
      variant={needsAccess ? 'ghost' : 'outline'}
      disabled={recheck.isPending}
      onClick={() => recheck.mutate()}
    >
      <ScanSearch className={cn(recheck.isPending && 'animate-pulse')} />
      {t('onboarding.browsers.fullDiskAccess.recheck')}
    </Button>
  )

  return (
    <>
      <PaneHeader
        headingId={headingId}
        title={title}
        summary={summaryText(t, counts)}
        alert={counts.readableChosen === 0}
        filter={filter}
        onFilter={onFilter}
        showFilter={items.length > GROUP_AFTER}
        ids={items.map((item) => item.id)}
        chosen={chosen}
        onSet={set}
      />
      {items.length > 0 ? (
        <SourceList
          label={t('onboarding.sources.browsersLabel')}
          items={items}
          chosen={chosen}
          filter={filter}
          onSet={set}
          row={(item) => (
            <ProfileRow
              key={item.id}
              profile={item.profile}
              checked={chosen.has(item.id)}
              onCheckedChange={(on) => set([item.id], on)}
            />
          )}
        />
      ) : needsAccess ? (
        <span className="flex-1" />
      ) : (
        <EmptyNotice
          kind={discovery === 'discovery-error' ? 'error' : 'empty'}
          action={recheckButton}
        />
      )}
      {needsAccess && (
        <div className="flex animate-rise items-start gap-3 border-t bg-brand-soft px-5 py-3.5">
          <ShieldAlert className="mt-0.5 size-[18px] shrink-0 text-brand" />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="text-sm font-medium">
              {t('onboarding.browsers.fullDiskAccess.title')}
            </span>
            <span className="text-[13px] leading-normal text-muted-foreground">
              {t('onboarding.browsers.fullDiskAccess.body')}
            </span>
            {openFailed && (
              <InlineError
                title={t('onboarding.browsers.fullDiskAccess.openFailed')}
                detail={openFailed}
              />
            )}
            {recheck.isError && (
              <InlineError
                title={t('onboarding.browsers.fullDiskAccess.recheckFailed')}
                detail={describeError(recheck.error, 'app_snapshot')}
              />
            )}
          </div>
          <div className="flex shrink-0 flex-col items-end gap-1">
            <Button size="sm" onClick={() => void openSettings()}>
              {t('onboarding.browsers.fullDiskAccess.open')}
            </Button>
            {recheckButton}
          </div>
        </div>
      )}
    </>
  )
}

function ProfileRow({
  profile,
  checked,
  onCheckedChange,
}: {
  profile: BrowserProfile
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const id = `profile-${profile.profileId}`
  const blocked = !isBrowserProfileReadable(profile)
  return (
    <li className="border-b last:border-b-0">
      <label
        htmlFor={id}
        className="flex cursor-pointer items-center gap-3.5 px-5 py-3 transition-colors hover:bg-muted/60"
      >
        <Checkbox
          id={id}
          checked={checked}
          onCheckedChange={(value) => onCheckedChange(value === true)}
          className="size-[18px] rounded-[5px]"
        />
        <BrowserIcon
          browserName={profile.browserName}
          className="size-7"
          decorative
        />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate font-medium">{profile.browserName}</span>
          <span className="truncate text-xs text-muted-foreground">
            {profile.profileName}
            {blocked && ` · ${t('onboarding.browsers.needsPermission')}`}
          </span>
        </span>
        {!blocked && (
          <span
            className="font-mono text-xs text-muted-foreground tabular"
            title={t('onboarding.browsers.historySize', {
              size: format.bytes(profile.historyBytes),
            })}
          >
            {format.bytes(profile.historyBytes)}
          </span>
        )}
      </label>
    </li>
  )
}

function EmptyNotice({
  kind,
  action,
}: {
  kind: 'empty' | 'error'
  action: React.ReactNode
}) {
  const { t } = useI18n()
  const Icon = kind === 'empty' ? ScanSearch : TriangleAlert
  return (
    <div className="m-5 flex animate-rise items-start gap-3 rounded-xl border px-4 py-3.5">
      <Icon
        className={cn(
          'mt-0.5 size-4 shrink-0',
          kind === 'empty' ? 'text-muted-foreground' : 'text-brand',
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">
          {t(`backup.sources.notice.${kind}.title`)}
        </span>
        <span className="text-[13px] text-muted-foreground">
          {t(`backup.sources.notice.${kind}.body`)}
        </span>
      </div>
      {action}
    </div>
  )
}
