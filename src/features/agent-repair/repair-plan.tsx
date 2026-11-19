/**
 * What a prepared repair will do, in the words the person decides on: which
 * repair, the counts the backend measured, what happens to backups after it,
 * and where what it replaces is kept.
 *
 * A count of conversations that can't be brought back is said only for
 * "start over", the one repair that can leave some behind, and when the
 * backend could not count them the plan says so instead of showing 0.
 */
import { Notice } from '@/features/settings/notice'
import { copyLabelKey } from '@/features/settings/copy-label'
import type { RepairPreview } from '@/lib/backend-client/agent-repair'
import { useFormat, useI18n } from '@/lib/i18n'

/** A decimal string from the wire; conversation counts stay far below 2^53. */
const count = (value: string) => Number(value) || 0

function LeftBehind({ preview }: { preview: RepairPreview }) {
  const { t } = useI18n()
  const left = preview.conversationsOnlyInCopy
  if (left === null)
    return <li>{t('agentRepair.actions.startFresh.uncounted')}</li>
  if (count(left) === 0)
    return <li>{t('agentRepair.actions.startFresh.allBack')}</li>
  return (
    <li>
      {t('agentRepair.actions.startFresh.leftBehind', { count: count(left) })}
    </li>
  )
}

export function RepairPlan({ preview }: { preview: RepairPreview }) {
  const { t } = useI18n()
  const format = useFormat()
  const conversations = count(preview.conversations)
  const action = preview.action

  return (
    <section
      aria-labelledby="repair-plan-title"
      data-repair-action={action}
      className="flex flex-col gap-3 text-[13px]"
    >
      <div className="flex flex-col gap-1">
        <h3 id="repair-plan-title" className="text-sm font-medium">
          {t(`agentRepair.actions.${action}.title`)}
        </h3>
        {action === 'fromCopy' && preview.sourceOp && (
          <p className="text-muted-foreground">
            {t('agentRepair.actions.fromCopy.made', {
              label: t(
                `settingsStorage.restore.op.${copyLabelKey(preview.sourceOp)}`,
              ),
              date: preview.createdAt
                ? format.date(preview.createdAt, {
                    dateStyle: 'medium',
                    timeStyle: 'short',
                  })
                : '—',
            })}
          </p>
        )}
        <p className="leading-relaxed text-muted-foreground">
          {t(`agentRepair.actions.${action}.body`)}
        </p>
      </div>

      <ul className="flex list-disc flex-col gap-1 pl-5">
        <li className="font-medium tabular">
          {t(`agentRepair.actions.${action}.count`, { count: conversations })}
        </li>
        {action === 'startFresh' && <LeftBehind preview={preview} />}
        {preview.nextBackupRecapturesToolFiles && (
          <li>{t('agentRepair.actions.startFresh.recapture')}</li>
        )}
      </ul>

      {preview.archiveOnlyConversationsUnreadable && (
        <Notice tone="warning">
          {t('agentRepair.actions.startFresh.toolsDeleted')}
        </Notice>
      )}
      {preview.reviewSourcesBeforeBackup && (
        <Notice tone="info">{t('agentRepair.reviewSources')}</Notice>
      )}

      <div className="flex flex-col gap-1">
        <p>{t('agentRepair.kept')}</p>
        <span
          data-kept-as
          className="font-mono text-xs break-all text-foreground"
        >
          {preview.keptAs}
        </span>
        <p className="text-muted-foreground">{t('agentRepair.keptHint')}</p>
      </div>
      <p className="text-xs text-muted-foreground">{t('agentRepair.held')}</p>
    </section>
  )
}
