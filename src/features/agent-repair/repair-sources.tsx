/**
 * Where saved conversations come back from, when the file that holds them is
 * damaged: what still checks out in the damaged files, or one of the safety
 * copies. Every listed copy is shown, newest first. The backend says which
 * can be a source (`conversationRepairSource`) from the files each holds:
 * - one that can is chosen, then checked (the check verifies what it holds);
 * - one that can't is there, can't be chosen, and says why;
 * - one made while conversations couldn't be read, or kept when a
 *   conversation file disappeared, says so.
 * A copy whose check asked for its own password gets a field for it. When
 * starting over would leave something behind, a line points to the copies.
 * When the conversation file belongs to another archive, nothing in it is
 * used: the first choice is starting over, and nothing is called damaged.
 *
 * Choosing a row only selects it; the dialog's main button checks it.
 */
import { useQuery } from '@tanstack/react-query'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Skeleton } from '@/components/ui/skeleton'
import { copyLabelKey, HELD_COPY } from '@/features/settings/copy-label'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import type { RecoverySnapshot } from '@/lib/types'
import { LOCAL, type SourceChoice } from './use-agent-repair'

const LOCAL_VALUE = 'local'
const valueOf = (source: SourceChoice) =>
  source.kind === 'local' ? LOCAL_VALUE : `copy:${source.path}`
const sourceOf = (value: string): SourceChoice =>
  value === LOCAL_VALUE ? LOCAL : { kind: 'copy', path: value.slice(5) }

const reasons: Record<string, MessageKey> = {
  missingEvidence: 'agentRepair.sources.missingEvidence',
  unsealed: 'agentRepair.sources.unsealed',
}

/** Why a copy can't be a source; `null` when it can. */
function unusable(copy: RecoverySnapshot): MessageKey | null {
  const source = copy.conversationRepairSource
  if (source === 'checkRequired') return null
  return reasons[source] ?? 'agentRepair.sources.unusable'
}

/** What else a person should know about a copy before checking it. */
function note(copy: RecoverySnapshot): MessageKey | null {
  if (copy.sourceOp === HELD_COPY) return 'agentRepair.sources.held'
  if (copy.conversationsUnreadable) return 'agentRepair.sources.marked'
  return null
}

const newestFirst = (a: RecoverySnapshot, b: RecoverySnapshot) =>
  Date.parse(b.createdAt ?? '') - Date.parse(a.createdAt ?? '') || 0

function CopyRow({ copy }: { copy: RecoverySnapshot }) {
  const { t } = useI18n()
  const format = useFormat()
  const remark = unusable(copy) ?? note(copy)
  const id = `repair-source-${encodeURIComponent(copy.path)}`
  return (
    <Label
      htmlFor={id}
      data-copy-path={copy.path}
      className="flex items-start gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted has-[:disabled]:opacity-70"
    >
      <RadioGroupItem
        id={id}
        value={valueOf({ kind: 'copy', path: copy.path })}
        disabled={unusable(copy) !== null}
        className="mt-0.5"
      />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">
          {t(`settingsStorage.restore.op.${copyLabelKey(copy.sourceOp)}`)}
        </span>
        <span className="text-xs text-muted-foreground tabular">
          {[
            copy.createdAt
              ? format.date(copy.createdAt, {
                  dateStyle: 'medium',
                  timeStyle: 'short',
                })
              : copy.label,
            format.bytes(copy.sizeBytes),
          ].join(' · ')}
        </span>
        {remark && (
          <span className="text-xs text-muted-foreground">{t(remark)}</span>
        )}
      </span>
    </Label>
  )
}

export function RepairSources({
  chosen,
  onChoose,
  disabled,
  suggestCopies,
  foreign,
  asked,
  passwords,
  onPassword,
}: {
  chosen: SourceChoice
  onChoose: (source: SourceChoice) => void
  disabled: boolean
  /** Starting over would leave conversations, or their latest messages, behind, or can't tell. */
  suggestCopies: boolean
  /**
   * The conversation file belongs to another archive: nothing in it is
   * used, so "what is here" is starting over, and nothing is called damaged.
   */
  foreign: boolean
  /** Copies whose check asked for their own password. */
  asked: ReadonlySet<string>
  passwords: Record<string, string>
  onPassword: (path: string, value: string) => void
}) {
  const { t } = useI18n()
  const list = useQuery({
    queryKey: ['recovery-snapshots'],
    queryFn: archiveClient.listRecoverySnapshots,
    staleTime: 0,
  })
  const copies = [...(list.data ?? [])].sort(newestFirst)
  const choosable = copies.filter((copy) => unusable(copy) === null)
  const passwordFor =
    chosen.kind === 'copy' && asked.has(chosen.path) ? chosen.path : null

  return (
    <section
      aria-labelledby="repair-sources-title"
      className="flex flex-col gap-2"
    >
      <h3 id="repair-sources-title" className="text-sm font-medium">
        {t('agentRepair.sources.title')}
      </h3>
      {suggestCopies && choosable.length > 0 && (
        <p data-suggest-copies className="text-[13px] text-muted-foreground">
          {t(
            foreign
              ? 'agentRepair.sources.foreignSuggest'
              : 'agentRepair.sources.suggest',
          )}
        </p>
      )}
      <RadioGroup
        value={valueOf(chosen)}
        onValueChange={(value) => onChoose(sourceOf(value))}
        aria-labelledby="repair-sources-title"
        disabled={disabled}
        className="gap-1"
      >
        <Label
          htmlFor="repair-source-local"
          className="flex items-start gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted"
        >
          <RadioGroupItem
            id="repair-source-local"
            value={LOCAL_VALUE}
            className="mt-0.5"
          />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">
              {t(
                foreign
                  ? 'agentRepair.sources.foreignLocal'
                  : 'agentRepair.sources.local',
              )}
            </span>
            <span className="text-xs text-muted-foreground">
              {t(
                foreign
                  ? 'agentRepair.sources.foreignLocalHint'
                  : 'agentRepair.sources.localHint',
              )}
            </span>
          </span>
        </Label>
        {list.isPending ? (
          <div className="flex flex-col gap-1">
            <span className="sr-only">{t('agentRepair.sources.loading')}</span>
            <Skeleton aria-hidden className="h-[58px] w-full rounded-lg" />
            <Skeleton aria-hidden className="h-[58px] w-full rounded-lg" />
          </div>
        ) : list.isError ? (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('agentRepair.sources.loadFailed', {
              message: describeError(list.error, 'list_recovery_snapshots'),
            })}
          </p>
        ) : (
          copies.map((copy) => <CopyRow key={copy.path} copy={copy} />)
        )}
      </RadioGroup>
      {list.isSuccess && choosable.length === 0 && (
        <p className="text-[13px] text-muted-foreground">
          {t('agentRepair.sources.none')}
        </p>
      )}
      {passwordFor && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="repair-copy-password">
            {t('settingsStorage.restore.password')}
          </Label>
          <Input
            id="repair-copy-password"
            type="password"
            autoComplete="current-password"
            autoFocus
            disabled={disabled}
            value={passwords[passwordFor] ?? ''}
            aria-describedby="repair-copy-password-hint"
            onChange={(event) => onPassword(passwordFor, event.target.value)}
          />
          <p
            id="repair-copy-password-hint"
            className="text-xs text-muted-foreground"
          >
            {t('settingsStorage.restore.passwordHint')}
          </p>
        </div>
      )}
    </section>
  )
}
