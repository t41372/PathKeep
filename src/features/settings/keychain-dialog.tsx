/**
 * Asks for the archive password so it can go into the system keychain.
 * PathKeep never keeps the password in memory on the frontend, so turning the
 * keychain on later needs the user to type it once more.
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { appClient } from '@/lib/backend-client/app'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'

export function KeychainDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <KeychainForm onClose={onClose} />}
    </Dialog>
  )
}

function KeychainForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const client = useQueryClient()
  const [password, setPassword] = useState('')

  const store = useMutation({
    mutationFn: async () => {
      await securityClient.storeDatabaseKey(password)
      let snapshot = await appClient.getSnapshot()
      if (!snapshot.config.rememberDatabaseKeyInKeyring) {
        snapshot = await appClient.saveConfig(
          { ...snapshot.config, rememberDatabaseKeyInKeyring: true },
          snapshot.config,
        )
      }
      client.setQueryData(queryKeys.snapshot, snapshot)
    },
    onSuccess: () => {
      toast.success(t('settings.security.keychain.saved'))
      onClose()
    },
  })

  return (
    <DialogContent className="sm:max-w-sm">
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (password && !store.isPending) store.mutate()
        }}
      >
        <DialogHeader>
          <DialogTitle>
            {t('settings.security.keychain.dialogTitle')}
          </DialogTitle>
          <DialogDescription>
            {t('settings.security.keychain.dialogBody')}
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="keychain-password">
            {t('settings.security.keychain.password')}
          </Label>
          <Input
            id="keychain-password"
            type="password"
            autoComplete="current-password"
            autoFocus
            value={password}
            disabled={store.isPending}
            onChange={(event) => setPassword(event.target.value)}
          />
        </div>
        {store.error && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('settings.security.keychain.saveFailed', {
              message: describeError(store.error, 'keyring_store_database_key'),
            })}
          </p>
        )}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={store.isPending}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!password || store.isPending}>
            {store.isPending && <Spinner />}
            {t('common.save')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
