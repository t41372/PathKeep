/**
 * Open at login and the menu bar (tray) icon. Both live in the OS, not in
 * PathKeep's config, so each switch shows what the OS reported last and only
 * moves once the backend confirms the change.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import {
  desktopClient,
  type DesktopIntegration,
} from '@/lib/backend-client/desktop'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { isMacOsHost } from '@/lib/runtime'

const queryKey = ['desktop-integration'] as const

type Setting = 'launchAtLogin' | 'menuBarIcon'

export function DesktopRows() {
  const { t } = useI18n()
  const query = useQuery({ queryKey, queryFn: desktopClient.get })

  if (query.isPending) {
    return (
      <>
        <Skeleton className="h-[76px] rounded-xl" />
        <Skeleton className="h-[76px] rounded-xl" />
      </>
    )
  }

  if (query.isError) {
    return (
      <SettingRow
        title={t(
          isMacOsHost()
            ? 'settings.general.desktop.title'
            : 'settings.general.desktop.trayTitle',
        )}
        description={
          <span className="text-destructive">
            {t('settings.general.desktop.loadFailed', {
              message: describeError(query.error, 'get_desktop_integration'),
            })}
          </span>
        }
        control={
          <Button
            size="sm"
            variant="outline"
            disabled={query.isFetching}
            onClick={() => void query.refetch()}
          >
            {t('common.retry')}
          </Button>
        }
      />
    )
  }

  const state = query.data
  return (
    <>
      {state.launchAtLoginSupported && (
        <DesktopSwitch setting="launchAtLogin" state={state} />
      )}
      {state.menuBarIconSupported && (
        <DesktopSwitch setting="menuBarIcon" state={state} />
      )}
    </>
  )
}

function DesktopSwitch({
  setting,
  state,
}: {
  setting: Setting
  state: DesktopIntegration
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const [error, setError] = useState<string | null>(null)
  const change = useMutation({
    mutationFn: (enabled: boolean) =>
      setting === 'launchAtLogin'
        ? desktopClient.setLaunchAtLogin(enabled)
        : desktopClient.setMenuBarIcon(enabled),
    onMutate: () => setError(null),
    onSuccess: (next) => client.setQueryData(queryKey, next),
    onError: (reason) => setError(describeError(reason)),
  })

  const id = `settings-${setting}`
  const copy =
    setting === 'launchAtLogin'
      ? {
          title: t('settings.general.desktop.login.title'),
          description: t('settings.general.desktop.login.description'),
        }
      : {
          title: t(
            isMacOsHost()
              ? 'settings.general.desktop.menuBar.title'
              : 'settings.general.desktop.menuBar.trayTitle',
          ),
          description: t('settings.general.desktop.menuBar.description'),
        }

  return (
    <SettingRow
      title={copy.title}
      description={copy.description}
      htmlFor={id}
      control={
        <Switch
          id={id}
          checked={state[setting]}
          disabled={change.isPending}
          onCheckedChange={(next) => change.mutate(next)}
        />
      }
    >
      {error && (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {t('settings.general.desktop.changeFailed', { message: error })}
        </p>
      )}
    </SettingRow>
  )
}
