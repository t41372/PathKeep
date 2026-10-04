/**
 * Copy and "show in folder" for the paths and files Backup shows: schedule
 * files, audit records, manifests, safety copies. Every path PathKeep names
 * can be copied or opened, so nobody has to guess where a file lives.
 */
import { Check, Copy, FolderOpen } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { supportClient } from '@/lib/backend-client/support'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'

/** A real filesystem path, as opposed to a label like `Task Scheduler:<task>`. */
function isFilesystemPath(path: string) {
  return path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path)
}

export function CopyButton({
  text,
  label,
  className,
}: {
  text: string
  label: string
  className?: string
}) {
  const { t } = useI18n()
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), 1500)
    return () => window.clearTimeout(timer)
  }, [copied])

  return (
    <Button
      type="button"
      size="icon-xs"
      variant="ghost"
      aria-label={copied ? t('backupSchedule.change.copied') : label}
      title={label}
      className={cn('text-muted-foreground', className)}
      onClick={() =>
        void navigator.clipboard.writeText(text).then(
          () => setCopied(true),
          (error: unknown) =>
            toast.error(t('backupSchedule.change.copyFailed'), {
              description: describeError(error),
            }),
        )
      }
    >
      {copied ? <Check /> : <Copy />}
    </Button>
  )
}

export function RevealButton({ path }: { path: string }) {
  const { t } = useI18n()
  return (
    <Button
      type="button"
      size="icon-xs"
      variant="ghost"
      aria-label={t('backupSchedule.verify.reveal')}
      title={t('backupSchedule.verify.reveal')}
      className="text-muted-foreground"
      onClick={() =>
        void supportClient.openPathInFileManager(path).catch((error) =>
          toast.error(t('backupSchedule.verify.revealFailed'), {
            description: describeError(error, 'open_path_in_file_manager'),
          }),
        )
      }
    >
      <FolderOpen />
    </Button>
  )
}

/** One path in monospace with copy and, for real paths, show-in-folder. */
export function PathLine({
  path,
  className,
}: {
  path: string
  className?: string
}) {
  const { t } = useI18n()
  return (
    <div className={cn('flex items-start gap-1', className)}>
      <span className="min-w-0 flex-1 pt-0.5 font-mono text-xs leading-5 break-all">
        {path}
      </span>
      <CopyButton text={path} label={t('backupSchedule.change.copyPath')} />
      {isFilesystemPath(path) && <RevealButton path={path} />}
    </div>
  )
}
