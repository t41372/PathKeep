/**
 * The choice on the way back from PathKeep 0.5.0: the archive was prepared
 * for it, its Agent conversations set aside in a package next to the data
 * folder, and this version opens nothing until the person says whether to
 * take them back or start without them (`preview_archive_rollback`, then
 * `resolve_archive_handoff`). Leaving the screen without choosing keeps the
 * archive as 0.5.0 uses it.
 *
 * Two passwords stay apart: the archive's current one (it may have changed
 * in 0.5.0) and the one the conversations had when they were set aside.
 * Most people never changed it, so the second field opens only on request.
 *
 * Right after preparing (`session.prepared`), the screen says what to do
 * next and keeps this choice folded away behind "Changed your mind?".
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { archiveAccessClient } from '@/lib/backend-client/archive-access'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { useSession } from '../../session'
import {
  AfterRepair,
  Panel,
  PasswordField,
  PathLine,
  Unfold,
  Working,
} from './parts'

const NEEDS_CURRENT = 'archive-current-password-required'
const WRONG_PASSWORD = 'archive-password-wrong'
const INCOMPLETE = 'archive-handoff-incomplete'
const OCCUPIED = 'archive-handoff-destination-occupied'

type Choice = 'rejoin' | 'fresh'

export function HandoffChoice() {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const justPrepared = session.prepared !== null
  const [unfolded, setUnfolded] = useState(!justPrepared)
  const preview = useQuery({
    queryKey: ['archive-rollback-preview', 'handoff'],
    queryFn: archiveAccessClient.previewRollback,
    staleTime: 0,
    gcTime: 0,
  })
  const [choice, setChoice] = useState<Choice>('rejoin')
  const [password, setPassword] = useState('')
  const [otherPassword, setOtherPassword] = useState(false)
  const [packagePassword, setPackagePassword] = useState('')
  const [askCurrent, setAskCurrent] = useState(false)
  const encrypted = preview.data?.encrypted ?? false
  const needsPassword = encrypted || askCurrent

  const resolve = useMutation({
    mutationFn: (rejoin: boolean) =>
      archiveAccessClient.resolveHandoff(
        rejoin,
        needsPassword ? password : null,
        rejoin && encrypted
          ? otherPassword
            ? packagePassword
            : password
          : null,
      ),
    onError: (error) => {
      if (commandErrorCode(error) === NEEDS_CURRENT) setAskCurrent(true)
    },
  })
  const finish = useMutation({
    mutationFn: () =>
      archiveAccessClient.prepareRollback(needsPassword ? password : null),
    onSuccess: () => {
      resolve.reset()
      void preview.refetch()
    },
  })
  const code = resolve.error ? commandErrorCode(resolve.error) : null
  const busy = resolve.isPending || finish.isPending
  const ready =
    (!needsPassword || password.length > 0) &&
    (choice === 'fresh' ||
      !encrypted ||
      !otherPassword ||
      packagePassword.length > 0)

  if (resolve.isSuccess) {
    return (
      <Panel title={t('archiveAccess.handoff.choose')} labelledBy="handoff">
        <AfterRepair
          text={t(
            resolve.variables
              ? 'archiveAccess.handoff.doneRejoin'
              : 'archiveAccess.handoff.doneFresh',
          )}
        />
      </Panel>
    )
  }

  const facts = preview.isPending ? (
    <div className="flex flex-col gap-2" aria-busy>
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-8 w-full" />
    </div>
  ) : preview.isError ? (
    <p role="alert" className="text-[13px] text-destructive">
      {t('archiveAccess.handoff.loadFailed', {
        message: describeError(preview.error, 'preview_archive_rollback'),
      })}
    </p>
  ) : (
    <div className="flex flex-col gap-2">
      <PathLine
        label={t('archiveAccess.handoff.folder')}
        path={preview.data.packageFolder}
      />
      <p className="text-[13px] text-muted-foreground">
        {t('archiveAccess.handoff.size', {
          size: format.bytes(preview.data.conversationBytes),
        })}
      </p>
      {preview.data.copyFoldersAndFiles.length > 0 && (
        <p className="text-[13px] text-muted-foreground">
          {t('archiveAccess.handoff.copies', {
            count: preview.data.copyFoldersAndFiles.length,
          })}
        </p>
      )}
    </div>
  )

  const form = (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault()
        if (ready && !busy) resolve.mutate(choice === 'rejoin')
      }}
    >
      <RadioGroup
        value={choice}
        onValueChange={(value) => setChoice(value as Choice)}
        aria-label={t('archiveAccess.handoff.choose')}
        className="gap-1.5"
      >
        {(['rejoin', 'fresh'] as const).map((value) => (
          <Label
            key={value}
            htmlFor={`handoff-${value}`}
            className="flex items-start gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted"
          >
            <RadioGroupItem
              id={`handoff-${value}`}
              value={value}
              className="mt-0.5"
              disabled={busy}
            />
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="font-medium">
                {t(
                  value === 'rejoin'
                    ? 'archiveAccess.handoff.rejoinTitle'
                    : 'archiveAccess.handoff.freshTitle',
                )}
              </span>
              <span className="text-xs text-muted-foreground">
                {t(
                  value === 'rejoin'
                    ? 'archiveAccess.handoff.rejoinBody'
                    : 'archiveAccess.handoff.freshBody',
                )}
              </span>
            </span>
          </Label>
        ))}
      </RadioGroup>

      <Unfold open={needsPassword}>
        <div className="flex flex-col gap-3">
          <PasswordField
            id="handoff-password"
            label={t('archiveAccess.handoff.password')}
            hint={t('archiveAccess.handoff.passwordHint')}
            value={password}
            onChange={setPassword}
            invalid={code === WRONG_PASSWORD}
            disabled={busy}
          />
          {encrypted && choice === 'rejoin' && (
            <div className="flex items-center gap-2">
              <Checkbox
                id="handoff-other-password"
                checked={otherPassword}
                disabled={busy}
                onCheckedChange={(value) => setOtherPassword(value === true)}
              />
              <Label htmlFor="handoff-other-password" className="font-normal">
                {t('archiveAccess.handoff.otherPassword')}
              </Label>
            </div>
          )}
          <Unfold open={encrypted && choice === 'rejoin' && otherPassword}>
            <PasswordField
              id="handoff-package-password"
              label={t('archiveAccess.handoff.packagePassword')}
              hint={t('archiveAccess.handoff.packagePasswordHint')}
              value={packagePassword}
              onChange={setPackagePassword}
              invalid={code === WRONG_PASSWORD}
              disabled={busy}
            />
          </Unfold>
        </div>
      </Unfold>

      {resolve.isPending && (
        <Working
          label={t(
            resolve.variables
              ? 'archiveAccess.handoff.runningRejoin'
              : 'archiveAccess.handoff.runningFresh',
          )}
        />
      )}
      {finish.isPending && (
        <Working label={t('archiveAccess.rollback.running')} />
      )}
      {resolve.error && code !== NEEDS_CURRENT && (
        <div className="flex flex-col gap-2">
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {code === WRONG_PASSWORD
              ? t('archiveAccess.handoff.wrongPassword')
              : code === INCOMPLETE
                ? t('archiveAccess.handoff.incomplete')
                : code === OCCUPIED
                  ? t('archiveAccess.handoff.occupied')
                  : t('archiveAccess.handoff.failed', {
                      message: describeError(
                        resolve.error,
                        'resolve_archive_handoff',
                      ),
                    })}
          </p>
          {code === INCOMPLETE && (
            <Button
              type="button"
              variant="outline"
              className="self-start"
              disabled={busy}
              onClick={() => finish.mutate()}
            >
              {t('archiveAccess.handoff.finish')}
            </Button>
          )}
        </div>
      )}
      {finish.error && (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {t('archiveAccess.rollback.failed', {
            message: describeError(finish.error, 'prepare_archive_rollback'),
          })}
        </p>
      )}

      <Button
        type="submit"
        className="self-start"
        variant={choice === 'fresh' ? 'outline' : 'default'}
        disabled={!ready || busy || !preview.isSuccess}
      >
        {resolve.isPending && <Spinner />}
        {t(
          choice === 'rejoin'
            ? 'archiveAccess.handoff.confirmRejoin'
            : 'archiveAccess.handoff.confirmFresh',
        )}
      </Button>
      <p className="text-[13px] text-muted-foreground">
        {t('archiveAccess.handoff.later')}
      </p>
    </form>
  )

  return (
    <Panel title={t('archiveAccess.handoff.choose')} labelledBy="handoff">
      {facts}
      {justPrepared && (
        <Button
          type="button"
          variant="link"
          className="h-auto self-start p-0 text-[13px]"
          aria-expanded={unfolded}
          onClick={() => setUnfolded((open) => !open)}
        >
          {t('archiveAccess.handoff.changedMind')}
        </Button>
      )}
      <Unfold open={unfolded}>{form}</Unfold>
    </Panel>
  )
}
