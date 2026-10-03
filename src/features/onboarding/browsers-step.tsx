/**
 * Step 2: pick the browser profiles to back up, from the ones the snapshot
 * discovered. Explains Safari's Full Disk Access and lets the user rescan
 * after granting it.
 *
 * Not responsible for saving the selection (the page does, on Continue).
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

function useRecheck() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: appClient.getSnapshot,
    onSuccess: (snapshot) => client.setQueryData(queryKeys.snapshot, snapshot),
  })
}

export function BrowsersStep({
  selected,
  onChange,
  showHint,
}: {
  selected: string[]
  onChange: (selected: string[]) => void
  showHint: boolean
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

  function toggle(profileId: string, on: boolean) {
    onChange(
      on ? [...selected, profileId] : selected.filter((id) => id !== profileId),
    )
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
      {profiles.length > 0 ? (
        <ul className="overflow-hidden rounded-xl border bg-card shadow-card">
          {profiles.map((profile) => (
            <ProfileRow
              key={profile.profileId}
              profile={profile}
              checked={chosen.has(profile.profileId)}
              onCheckedChange={(on) => toggle(profile.profileId, on)}
            />
          ))}
        </ul>
      ) : (
        !needsAccess && (
          <EmptyNotice
            kind={discovery === 'discovery-error' ? 'error' : 'empty'}
            action={recheckButton}
          />
        )
      )}
      {needsAccess && (
        <div className="flex animate-rise items-start gap-3 rounded-xl bg-brand-soft px-4 py-3.5">
          <ShieldAlert className="mt-0.5 size-[18px] shrink-0 text-brand" />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="font-medium">
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
      {showHint && profiles.length > 0 && (
        <p className="text-[13px] text-muted-foreground">
          {t('onboarding.browsers.needOne')}
        </p>
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
        className="flex cursor-pointer items-center gap-3.5 px-4 py-3.5 transition-colors hover:bg-muted/60"
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
    <div className="flex items-start gap-3 rounded-xl border bg-card px-4 py-3.5 shadow-card">
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
