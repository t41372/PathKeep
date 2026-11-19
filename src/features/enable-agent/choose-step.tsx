/**
 * The second step, "Choose": a tick per tool found (a tool whose folder
 * cannot be read cannot be ticked, and says so), the switch for noting each
 * project's git details, and three notes: the shared schedule as it is set
 * now, where the space will appear, and where these choices live later.
 *
 * Responsibilities: drawing the choices and the notes. Not responsible
 * for: keeping the choices (`use-agent-choices.ts`).
 */
import { CircleAlert, GitBranch } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { isStoreDamage } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { scheduleInstalled, useScheduleStatus } from '@/lib/queries/schedule'
import { spaceKeyLabel, spaces, spacesOn } from '@/lib/spaces'
import { formatInterval } from '@/features/backup/interval'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import type { ToolChoice } from './preview-model'
import type { useAgentChoices } from './use-agent-choices'

function ScheduleNote() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const status = useScheduleStatus()
  if (status.isPending) return <Skeleton className="h-4 w-72" />
  const schedule = scheduleInstalled(status.data)
    ? t('enableAgent.choose.scheduleEvery', {
        interval: formatInterval(snapshot.config.dueAfterHours, t),
      })
    : t('enableAgent.choose.scheduleOff')
  return <>{t('enableAgent.choose.schedule', { schedule })}</>
}

/** The switcher's shortcut the Agent space will have once it is on, if it gets one. */
function agentShortcut() {
  const on = new Set(spacesOn().map((space) => space.id))
  const order = spaces
    .filter((space) => on.has(space.id) || space.id === 'agent')
    .map((space) => space.id)
  const position = order.indexOf('agent') + 1
  return position >= 1 && position <= 9 ? spaceKeyLabel(position) : null
}

export function ChooseStep({
  tools,
  choices,
}: {
  tools: readonly ToolChoice[]
  choices: ReturnType<typeof useAgentChoices>
}) {
  const { t } = useI18n()
  const format = useFormat()
  const gitTitle = useId()
  const gitBody = useId()
  const listId = useId()
  const name = t('spaces.agent.name')
  const keys = agentShortcut()

  return (
    <div className="flex flex-col gap-4">
      <span id={listId} className="sr-only">
        {t('enableAgent.choose.toolsLabel')}
      </span>
      <ul
        aria-labelledby={listId}
        className="flex flex-col divide-y rounded-lg border"
      >
        {tools.map((tool) => {
          const nameId = `${listId}-${tool.tool}`
          return (
            <li key={tool.tool}>
              <label className="flex cursor-pointer items-center gap-3 px-3.5 py-3 text-[13px] has-disabled:cursor-default">
                <Checkbox
                  aria-labelledby={nameId}
                  disabled={!tool.readable || choices.loading}
                  checked={tool.readable && choices.ticked.has(tool.tool)}
                  onCheckedChange={(on) =>
                    choices.setTicked(tool.tool, on === true)
                  }
                />
                <span id={nameId}>
                  <ToolBadge id={tool.tool} />
                </span>
                <span className="ml-auto text-right text-xs text-muted-foreground tabular">
                  {tool.readable ? (
                    t('enableAgent.choose.toolSummary', {
                      count: tool.files,
                      size: format.bytes(tool.bytes),
                    })
                  ) : (
                    <span className="flex items-center gap-1.5">
                      <CircleAlert
                        aria-hidden
                        className="size-3.5 text-brand"
                      />
                      {t('enableAgent.choose.toolUnreadable')}
                    </span>
                  )}
                </span>
              </label>
            </li>
          )
        })}
      </ul>
      {choices.saveError !== null && (
        <div
          role="alert"
          className="flex items-start justify-between gap-3 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
        >
          <span className="flex flex-col gap-1">
            <span className="font-medium">
              {t('enableAgent.choose.saveFailed')}
            </span>
            <span className="text-xs [overflow-wrap:anywhere] text-muted-foreground">
              {isStoreDamage(choices.saveError)
                ? t('enableAgent.preview.reasons.unavailable')
                : describeError(choices.saveError)}
            </span>
          </span>
          <Button variant="outline" size="sm" onClick={choices.retrySave}>
            {t('common.retry')}
          </Button>
        </div>
      )}
      <div className="flex items-start justify-between gap-4 rounded-lg border bg-card px-3.5 py-3 text-[13px]">
        <span className="flex flex-col gap-1">
          <span id={gitTitle} className="flex items-center gap-1.5 font-medium">
            <GitBranch aria-hidden className="size-4 text-muted-foreground" />
            {t('enableAgent.choose.git.title')}
          </span>
          <span id={gitBody} className="leading-relaxed text-muted-foreground">
            {t('enableAgent.choose.git.body')}
          </span>
        </span>
        <Switch
          aria-labelledby={gitTitle}
          aria-describedby={gitBody}
          disabled={choices.loading}
          checked={choices.readGit}
          onCheckedChange={choices.setReadGit}
          className="mt-0.5"
        />
      </div>
      <ul className="flex flex-col gap-1.5 text-[13px] text-muted-foreground">
        <li>
          <ScheduleNote />
        </li>
        <li>
          {keys
            ? t('enableAgent.choose.switcher', { name, keys })
            : t('enableAgent.choose.switcherNoKeys', { name })}
        </li>
        <li>{t('enableAgent.choose.later', { name })}</li>
      </ul>
    </div>
  )
}
