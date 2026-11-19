/**
 * What a finished repair leaves: conversations open again and how many, the
 * safety copy that holds what was replaced (with the way to it), and, when the
 * repair ended this computer's folder approval, that backups wait for the
 * folders to be chosen again.
 */
import { CircleCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Notice } from '@/features/settings/notice'
import { reveal, revealLabelKey } from '@/features/settings/reveal'
import { useI18n } from '@/lib/i18n'
import type { RepairDone } from './use-agent-repair'

export function RepairResult({ done }: { done: RepairDone }) {
  const { t } = useI18n()
  const { preview, result } = done
  return (
    <section
      role="status"
      aria-labelledby="repair-done-title"
      className="flex flex-col gap-3 text-[13px]"
    >
      <div className="flex items-start gap-2">
        <CircleCheck
          aria-hidden
          className="mt-0.5 size-4 shrink-0 text-green"
        />
        <div className="flex flex-col gap-0.5">
          <h3 id="repair-done-title" className="text-sm font-medium">
            {t('agentRepair.done.title')}
          </h3>
          <p className="tabular">
            {t('agentRepair.done.count', {
              count: Number(preview.conversations) || 0,
            })}
          </p>
        </div>
      </div>
      {preview.nextBackupRecapturesToolFiles && (
        <p className="text-muted-foreground">
          {t('agentRepair.actions.startFresh.recapture')}
        </p>
      )}
      {preview.reviewSourcesBeforeBackup && (
        <Notice tone="info">{t('agentRepair.done.reviewSources')}</Notice>
      )}
      <div className="flex flex-col gap-1">
        <p>{t('agentRepair.done.kept')}</p>
        <span
          data-kept-copy
          className="font-mono text-xs break-all text-foreground"
        >
          {result.keptCopyPath}
        </span>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() =>
              void reveal(
                result.keptCopyPath,
                t('settingsStorage.location.failed'),
              )
            }
          >
            {t(revealLabelKey())}
          </Button>
        </div>
        <p className="text-muted-foreground">{t('agentRepair.keptHint')}</p>
      </div>
    </section>
  )
}
