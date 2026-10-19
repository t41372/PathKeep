/**
 * "Forgot passcode?" on the app lock screen: the hint saved with the
 * passcode, and the honest way out.
 *
 * Responsibilities: show the recovery hint (or say there is none), say that
 * app lock only guards this window, and point at the one file to edit to turn
 * it off, with a button that shows its folder.
 *
 * Not responsible for: resetting anything. There is deliberately no reset
 * button: one that works on a locked window would let anyone past the lock.
 * Turning it off means editing `config.json` with PathKeep closed, which needs
 * access to this computer account's files, the same access that could read
 * them anyway (see ADR-005 and `TROUBLESHOOTING.md`).
 */
import { FolderOpen } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { isMacOsHost } from '@/lib/runtime'
import type { AppLockStatus } from '@/lib/types'

export function ForgotPasscode({ status }: { status: AppLockStatus }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const hint = status.recoveryHint?.trim()

  const showFolder = () =>
    void supportClient.openPathInFileManager(status.configPath).catch((error) =>
      toast.error(t('shell.lock.showFolderFailed'), {
        description: describeError(error, 'open_path_in_file_manager'),
      }),
    )

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="flex w-full flex-col items-center gap-2"
    >
      <CollapsibleTrigger asChild>
        <Button
          type="button"
          variant="link"
          size="sm"
          className="h-auto p-0 text-muted-foreground"
        >
          {t('shell.lock.forgot')}
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="w-full animate-rise">
        <div className="flex flex-col gap-2 rounded-lg border bg-card p-3 text-left text-[13px]">
          <p>
            {hint ? (
              <>
                {t('shell.lock.hint')}{' '}
                <span className="font-medium [overflow-wrap:anywhere]">
                  {hint}
                </span>
              </>
            ) : (
              t('shell.lock.noHint')
            )}
          </p>
          <p className="text-muted-foreground">{t('shell.lock.onlyWindow')}</p>
          <p className="text-muted-foreground">{t('shell.lock.turnOff')}</p>
          <code className="font-mono text-xs break-all">
            {status.configPath}
          </code>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="self-start"
            onClick={showFolder}
          >
            <FolderOpen />
            {t(
              isMacOsHost()
                ? 'shell.lock.showFolderMac'
                : 'shell.lock.showFolder',
            )}
          </Button>
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}
