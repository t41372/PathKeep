/**
 * Settings → Security. Two separate things live here and the copy keeps them
 * apart: archive encryption (the file on disk, `rekey_archive`) and app lock
 * (a passcode in front of the window, `set_app_lock_passcode`).
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Kbd } from '@/components/ui/kbd'
import { Switch } from '@/components/ui/switch'
import { appClient } from '@/lib/backend-client/app'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import { isMacOsHost } from '@/lib/runtime'
import { KeychainDialog } from './keychain-dialog'
import { PasscodeDialog } from './passcode-dialog'
import { RekeyDialog } from './rekey-dialog'
import { RowSelect } from './row-select'
import type { RekeyMode } from './use-rekey'
import { useSaveSetting } from './use-save-setting'

const idleMinutes = [1, 5, 15, 30, 60]

export function SecuritySection() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.security')}>
      <SettingsGroup title={t('settings.security.archiveGroup')} />
      <ArchiveRows />
      <SettingsGroup
        title={t('settings.security.lockGroup')}
        note={t('settings.security.lockNote')}
      />
      <AppLockRows />
    </SettingsSection>
  )
}

function ArchiveRows() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const client = useQueryClient()
  const [rekey, setRekey] = useState<RekeyMode | null>(null)
  const [keychainOpen, setKeychainOpen] = useState(false)
  const encrypted = snapshot.archiveStatus.encrypted
  const keyring = snapshot.keyringStatus

  // Remove from the keychain first; only then say so in the config. A failed
  // removal must not leave the switch claiming the password is gone.
  const forget = useMutation({
    mutationFn: async () => {
      await securityClient.clearDatabaseKey()
      let next = await appClient.getSnapshot()
      if (next.config.rememberDatabaseKeyInKeyring) {
        next = await appClient.saveConfig(
          { ...next.config, rememberDatabaseKeyInKeyring: false },
          next.config,
        )
      }
      client.setQueryData(queryKeys.snapshot, next)
    },
    onSuccess: () => toast.success(t('settings.security.keychain.removed')),
    onError: (error) =>
      toast.error(t('settings.security.keychain.removeFailed'), {
        description: describeError(error, 'keyring_clear_database_key'),
      }),
  })

  return (
    <>
      <SettingRow
        title={t('settings.security.encrypt.title')}
        description={t(
          encrypted
            ? 'settings.security.encrypt.on'
            : 'settings.security.encrypt.off',
        )}
        htmlFor="settings-encrypt"
        control={
          <Switch
            id="settings-encrypt"
            checked={encrypted}
            onCheckedChange={(next) => setRekey(next ? 'encrypt' : 'decrypt')}
          />
        }
      />
      {encrypted && (
        <>
          <SettingRow
            title={t('settings.security.keychain.title')}
            description={t(
              keyring.available
                ? 'settings.security.keychain.description'
                : 'settings.security.keychain.unavailable',
            )}
            htmlFor="settings-keychain"
            control={
              <Switch
                id="settings-keychain"
                checked={keyring.available && keyring.storedSecret}
                disabled={!keyring.available || forget.isPending}
                onCheckedChange={(next) =>
                  next ? setKeychainOpen(true) : forget.mutate()
                }
              />
            }
          />
          <SettingRow
            title={t('settings.security.password.title')}
            description={t('settings.security.password.description')}
            control={
              <Button
                size="sm"
                variant="outline"
                onClick={() => setRekey('change')}
              >
                {t('settings.security.password.action')}
              </Button>
            }
          />
        </>
      )}
      <RekeyDialog mode={rekey} onClose={() => setRekey(null)} />
      <KeychainDialog
        open={keychainOpen}
        onClose={() => setKeychainOpen(false)}
      />
    </>
  )
}

function AppLockRows() {
  const { t } = useI18n()
  const session = useSession()
  const snapshot = useSnapshot()
  const client = useQueryClient()
  const { save, saving } = useSaveSetting()
  const [passcode, setPasscode] = useState<'set' | 'change' | null>(null)
  const [confirmRemove, setConfirmRemove] = useState(false)
  const lock = snapshot.config.appLock
  const hasPasscode = snapshot.appLockStatus.passcodeConfigured

  const toggle = (next: boolean) => {
    if (next && !hasPasscode) return setPasscode('set')
    void save((config) => {
      config.appLock.enabled = next
      config.appLock.passcodeEnabled = true
      return config
    })
  }

  const remove = useMutation({
    mutationFn: async () => {
      await appClient.clearAppLockPasscode()
      client.setQueryData(queryKeys.snapshot, await appClient.getSnapshot())
    },
    onSuccess: () => toast.success(t('settings.security.passcode.removed')),
    onError: (error) =>
      toast.error(t('settings.security.passcode.removeFailed'), {
        description: describeError(error, 'clear_app_lock_passcode'),
      }),
  })

  const lockNow = () =>
    void session.lock().then((locked) => {
      if (!locked) toast(t('shell.lock.needsPasscode'))
    })

  const shortcut = isMacOsHost() ? '⌘L' : 'Ctrl+L'

  return (
    <>
      <SettingRow
        title={t('settings.security.appLock.title')}
        description={t('settings.security.appLock.description')}
        htmlFor="settings-app-lock"
        control={
          <Switch
            id="settings-app-lock"
            checked={lock.enabled}
            disabled={saving}
            onCheckedChange={toggle}
          />
        }
      />
      {hasPasscode && (
        <SettingRow
          title={t('settings.security.passcode.title')}
          description={t('settings.security.passcode.description')}
          control={
            <>
              <Button
                size="sm"
                variant="ghost"
                disabled={remove.isPending}
                onClick={() => setConfirmRemove(true)}
              >
                {t('settings.security.passcode.remove')}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPasscode('change')}
              >
                {t('settings.security.passcode.change')}
              </Button>
            </>
          }
        />
      )}
      {lock.enabled && (
        <>
          <SettingRow
            title={t('settings.security.autoLock.title')}
            description={t('settings.security.autoLock.description')}
            htmlFor="settings-auto-lock"
            control={
              <RowSelect
                id="settings-auto-lock"
                value={String(lock.idleTimeoutMinutes)}
                disabled={saving}
                onChange={(value) =>
                  void save((config) => {
                    config.appLock.idleTimeoutMinutes = Number(value)
                    return config
                  })
                }
                options={idleMinutes.map((minutes) => ({
                  value: String(minutes),
                  label:
                    minutes === 60
                      ? t('settings.security.autoLock.hour')
                      : t('settings.security.autoLock.minutes', {
                          count: minutes,
                        }),
                }))}
              />
            }
          />
          <SettingRow
            title={t('settings.security.lockNow.title')}
            description={
              <>
                {t('settings.security.lockNow.description')}{' '}
                <Kbd>{shortcut}</Kbd>
              </>
            }
            control={
              <Button size="sm" variant="outline" onClick={lockNow}>
                {t('settings.security.lockNow.action')}
              </Button>
            }
          />
        </>
      )}
      <PasscodeDialog mode={passcode} onClose={() => setPasscode(null)} />
      <AlertDialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settings.security.passcode.removeTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settings.security.passcode.removeBody')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => remove.mutate()}>
              {t('settings.security.passcode.remove')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
