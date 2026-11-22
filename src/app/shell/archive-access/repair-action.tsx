/**
 * Rebuilding what a damaged archive lacks, on the person's explicit choice
 * (`repair_archive_control`): a settings file that is missing or damaged, or
 * a store PathKeep 0.5 had and the folder no longer does (Ask chats, or site
 * categories and saved page text records). Nothing here deletes data: the
 * settings rebuild checks the archive's files under its key before writing,
 * and a store is made only where none is.
 *
 * The archive's own password is asked for when the backend says it needs it
 * (`archive-current-password-required`): an encrypted archive's files are
 * checked under its key.
 */
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import {
  archiveAccessClient,
  type ArchiveControlRepair,
} from '@/lib/backend-client/archive-access'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { AfterRepair, Panel, PasswordField, Unfold, Working } from './parts'

const NEEDS_PASSWORD = 'archive-current-password-required'
const WRONG_PASSWORD = 'archive-password-wrong'
const UNAVAILABLE = [
  'archive-control-recovery-unavailable',
  'archive-store-recovery-unavailable',
]

export function RepairAction({ repair }: { repair: ArchiveControlRepair }) {
  const { t } = useI18n()
  const [password, setPassword] = useState('')
  const [askPassword, setAskPassword] = useState(false)
  const run = useMutation({
    mutationFn: () =>
      archiveAccessClient.repairControl(repair, askPassword ? password : null),
    onError: (error) => {
      if (commandErrorCode(error) === NEEDS_PASSWORD) setAskPassword(true)
    },
  })
  const code = run.error ? commandErrorCode(run.error) : null
  const words =
    repair === 'settings'
      ? {
          title: t('archiveAccess.rebuild.title'),
          body: t('archiveAccess.rebuild.body'),
          confirm: t('archiveAccess.rebuild.confirm'),
          running: t('archiveAccess.rebuild.running'),
          done: t('archiveAccess.rebuild.done'),
          unavailable: t('archiveAccess.rebuild.unavailable'),
          failed: (message: string) =>
            t('archiveAccess.rebuild.failed', { message }),
        }
      : {
          title: t(
            repair === 'ask'
              ? 'archiveAccess.addStore.askTitle'
              : 'archiveAccess.addStore.intelligenceTitle',
          ),
          body: t(
            repair === 'ask'
              ? 'archiveAccess.addStore.askBody'
              : 'archiveAccess.addStore.intelligenceBody',
          ),
          confirm: t('archiveAccess.addStore.confirm'),
          running: t('archiveAccess.addStore.running'),
          done: t('archiveAccess.addStore.done'),
          unavailable: t('archiveAccess.addStore.unavailable'),
          failed: (message: string) =>
            t('archiveAccess.addStore.failed', { message }),
        }

  if (run.isSuccess) {
    return (
      <Panel title={words.title} labelledBy={`repair-${repair}`}>
        <AfterRepair text={words.done} />
      </Panel>
    )
  }

  return (
    <Panel title={words.title} labelledBy={`repair-${repair}`}>
      <p className="text-[13px] text-muted-foreground">{words.body}</p>
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault()
          if (!run.isPending && (!askPassword || password)) run.mutate()
        }}
      >
        <Unfold open={askPassword}>
          <PasswordField
            id={`repair-${repair}-password`}
            label={t('archiveAccess.currentPassword')}
            hint={t('archiveAccess.currentPasswordHint')}
            value={password}
            onChange={setPassword}
            invalid={code === WRONG_PASSWORD}
            disabled={run.isPending}
          />
        </Unfold>
        {run.isPending && <Working label={words.running} />}
        {run.error && code !== NEEDS_PASSWORD && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {code === WRONG_PASSWORD
              ? t('archiveAccess.wrongCurrentPassword')
              : code && UNAVAILABLE.includes(code)
                ? words.unavailable
                : words.failed(describeError(run.error))}
          </p>
        )}
        <Button
          type="submit"
          className="self-start"
          disabled={run.isPending || (askPassword && !password)}
        >
          {run.isPending && <Spinner />}
          {words.confirm}
        </Button>
      </form>
    </Panel>
  )
}
