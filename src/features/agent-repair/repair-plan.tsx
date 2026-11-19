/**
 * What a prepared repair will do, in the words the person decides on: which
 * repair, the counts the backend measured, what happens to backups after it,
 * and where what it replaces is kept.
 *
 * Only "start over" compares what comes back with what the damaged files
 * held: how many conversations come back complete, how many only as an older
 * version (their latest messages stay in the kept copy), and how many don't
 * come back. A count the backend could not establish is said to be unknown,
 * never shown as 0. Details that were left out (stars, titles, where a file
 * came from) are counted for any repair.
 *
 * Why the repair is needed changes the words, never the counts: a rebuild for
 * a few conversations that can't be read (`parked`) counts only those; one
 * for lists that don't match the conversations (`diverged`) says so rather
 * than "damaged"; starting over because the conversation file belongs to
 * another archive (`foreign`) uses nothing in it, so there is nothing to
 * compare and nothing is called damaged.
 */
import { Notice } from '@/features/settings/notice'
import { copyLabelKey } from '@/features/settings/copy-label'
import type { RepairPreview } from '@/lib/backend-client/agent-repair'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { count, maybeCount } from './repair-counts'
import type { RepairCause } from './use-agent-repair'

/** Of what comes back: complete, or only as an older version. Nothing when nothing comes back. */
function Completeness({ preview }: { preview: RepairPreview }) {
  const { t } = useI18n()
  const whole = maybeCount(preview.conversationsRecoveredWhole)
  const partial = maybeCount(preview.conversationsRecoveredPartially)
  if (count(preview.conversations) === 0) return null
  if (whole === null || partial === null)
    return <li>{t('agentRepair.actions.startFresh.latestUnknown')}</li>
  if (partial === 0)
    return <li>{t('agentRepair.actions.startFresh.allComplete')}</li>
  return (
    <>
      <li>{t('agentRepair.actions.startFresh.whole', { count: whole })}</li>
      <li data-partial>
        {t('agentRepair.actions.startFresh.partial', { count: partial })}
      </li>
    </>
  )
}

function LeftBehind({ preview }: { preview: RepairPreview }) {
  const { t } = useI18n()
  const left = maybeCount(preview.conversationsOnlyInCopy)
  if (left === null)
    return <li>{t('agentRepair.actions.startFresh.uncounted')}</li>
  if (left === 0) return <li>{t('agentRepair.actions.startFresh.allBack')}</li>
  return (
    <li>{t('agentRepair.actions.startFresh.leftBehind', { count: left })}</li>
  )
}

/** Details that didn't come through, for any repair. */
function LeftOut({ preview }: { preview: RepairPreview }) {
  const { t } = useI18n()
  const records = count(preview.skippedOptionalRecords)
  const sections = count(preview.unreadableOptionalSections)
  return (
    <>
      {records > 0 && (
        <li>{t('agentRepair.details.skipped', { count: records })}</li>
      )}
      {sections > 0 && (
        <li>{t('agentRepair.details.unreadable', { count: sections })}</li>
      )}
    </>
  )
}

/** The plan's title, body and count line for this action and cause. */
function wordsFor(
  action: RepairPreview['action'],
  cause: RepairCause,
): { title: MessageKey; body: MessageKey; count: MessageKey } {
  const words = {
    title: `agentRepair.actions.${action}.title`,
    body: `agentRepair.actions.${action}.body`,
    count: `agentRepair.actions.${action}.count`,
  } as const
  if (action === 'rebuild' && cause === 'parked')
    return {
      ...words,
      body: 'agentRepair.actions.rebuild.parkedBody',
      count: 'agentRepair.actions.rebuild.parkedCount',
    }
  if (action === 'rebuild' && cause === 'diverged')
    return { ...words, body: 'agentRepair.actions.rebuild.divergedBody' }
  if (action === 'startFresh' && cause === 'foreign')
    return {
      ...words,
      title: 'agentRepair.actions.startFresh.foreignTitle',
      body: 'agentRepair.actions.startFresh.foreignBody',
    }
  return words
}

export function RepairPlan({
  preview,
  cause,
}: {
  preview: RepairPreview
  cause: RepairCause
}) {
  const { t } = useI18n()
  const format = useFormat()
  const conversations = count(preview.conversations)
  const action = preview.action
  const words = wordsFor(action, cause)
  // Nothing in another archive's file is used, so nothing is compared with it.
  const compares = action === 'startFresh' && cause !== 'foreign'

  return (
    <section
      aria-labelledby="repair-plan-title"
      data-repair-action={action}
      className="flex flex-col gap-3 text-[13px]"
    >
      <div className="flex flex-col gap-1">
        <h3 id="repair-plan-title" className="text-sm font-medium">
          {t(words.title)}
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
        <p className="leading-relaxed text-muted-foreground">{t(words.body)}</p>
      </div>

      <ul className="flex list-disc flex-col gap-1 pl-5">
        <li className="font-medium tabular">
          {t(words.count, { count: conversations })}
        </li>
        {compares && (
          <>
            <Completeness preview={preview} />
            <LeftBehind preview={preview} />
          </>
        )}
        <LeftOut preview={preview} />
        {preview.nextBackupRecapturesToolFiles && (
          <li>{t('agentRepair.actions.startFresh.recapture')}</li>
        )}
      </ul>

      {cause !== 'foreign' && preview.archiveOnlyConversationsUnreadable && (
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
