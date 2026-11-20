/**
 * Settings › Agent conversations (`#/settings/agent`, `agent-history.md`
 * §7): the preferences that have nothing to do with backing up. Which
 * tools, which folders and reading `.git` are on the space's backup page,
 * and the page opens by saying so.
 *
 * Responsibilities: the five groups in the spec's order: grouping into
 * projects (scratch folders, the folders waiting to be confirmed),
 * the reader, usage and cost (the same two switches as Insights' cost
 * card), AI access (three switches, all off at first, the index's state
 * under its switch: `semantic-status.tsx`) and what Continue copies. Each switch shows the saved value, changes at once, and goes
 * back with a notice when the backend refuses it. A kind that is off has
 * no page here; an old link says so.
 * Not responsible for: what the switches change elsewhere (the reader,
 * Insights, Projects read them themselves).
 */
import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import { CardError } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useSavePreferences } from '@/features/agent-insights/use-preferences'
import type { AgentViewerPatch } from '@/lib/backend-client/agent-viewer'
import { describeError } from '@/lib/errors'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useInsightsPreferences } from '@/lib/queries/agent-insights'
import {
  useAgentProjectsSync,
  useProjectList,
} from '@/lib/queries/agent-projects'
import {
  useSaveViewerPreferences,
  useViewerPreferences,
} from '@/lib/queries/agent-viewer'
import { spaceBackupPath, spacePath } from '@/lib/spaces'
import { ScratchRow } from './scratch-row'
import { SemanticIndexStatus } from './semantic-status'

type ViewerSwitch =
  | 'readerCollapseToolCalls'
  | 'readerShowThinking'
  | 'assistantMayRead'
  | 'buildSemanticIndex'
  | 'mcpMayRead'

const SWITCH_SKELETON = (
  <Skeleton aria-hidden className="h-5 w-9 rounded-full" />
)

function ViewerSwitchRow({
  field,
  title,
  hint,
  under,
}: {
  field: ViewerSwitch
  title: MessageKey
  hint: MessageKey
  /**
   * What the value has made, under the row once it is read; `saving` while
   * a change is on its way (the value shown is already the new one).
   */
  under?: (on: boolean, saving: boolean) => ReactNode
}) {
  const { t } = useI18n()
  const prefs = useViewerPreferences()
  const save = useSaveViewerPreferences()
  const [saving, setSaving] = useState(false)
  const id = `settings-agent-${field}`
  const toggle = (on: boolean) => {
    setSaving(true)
    void save({ [field]: on } satisfies AgentViewerPatch)
      .catch((error: unknown) =>
        toast.error(t('settingsAgent.saveFailed'), {
          description: describeError(error),
        }),
      )
      .finally(() => setSaving(false))
  }
  return (
    <SettingRow
      title={t(title)}
      description={t(hint)}
      htmlFor={id}
      control={
        prefs.data ? (
          <Switch
            id={id}
            checked={prefs.data[field]}
            onCheckedChange={toggle}
          />
        ) : (
          SWITCH_SKELETON
        )
      }
    >
      {prefs.data && under?.(prefs.data[field], saving)}
    </SettingRow>
  )
}

const semanticStatus = (on: boolean, saving: boolean) => (
  <SemanticIndexStatus on={on} saving={saving} />
)

function SuggestionsRow({
  projects,
}: {
  projects: ReturnType<typeof useProjectList>
}) {
  const { t } = useI18n()
  const count = projects.data?.pendingSuggestions
  return (
    <SettingRow
      title={t('settingsAgent.suggestions.title')}
      description={
        projects.isPending && projects.fetchStatus !== 'idle' ? (
          <Skeleton aria-hidden className="mt-1 h-3.5 w-72" />
        ) : count === undefined ? (
          t('settingsAgent.suggestions.unknown')
        ) : count === 0 ? (
          t('settingsAgent.suggestions.none')
        ) : (
          t('settingsAgent.suggestions.description', { count })
        )
      }
      control={
        count !== undefined &&
        count > 0 && (
          <Button variant="outline" size="sm" asChild>
            <Link to={spacePath('agent', 'projects', '?review=1')}>
              {t('settingsAgent.suggestions.review')}
            </Link>
          </Button>
        )
      }
    />
  )
}

/** The two cost switches: archive choices, shared with Insights' cost card. */
function CostRows({ version }: { version: string | undefined }) {
  const { t } = useI18n()
  const { readable } = useAgentKind()
  const insights = useInsightsPreferences()
  const save = useSavePreferences()
  const prefs = insights.data
  const unread = !readable || insights.isError
  const control = (
    id: string,
    checked: boolean | undefined,
    onChange: (on: boolean) => void,
    disabled = false,
  ) =>
    checked === undefined ? (
      unread ? (
        <Switch id={id} checked={false} disabled />
      ) : (
        SWITCH_SKELETON
      )
    ) : (
      <Switch
        id={id}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    )
  return (
    <>
      <SettingRow
        title={t('settingsAgent.cost.show')}
        description={
          unread && !prefs
            ? t('settingsAgent.cost.unavailable')
            : t('settingsAgent.cost.showHint')
        }
        htmlFor="settings-agent-show-cost"
        control={control(
          'settings-agent-show-cost',
          prefs?.showCost,
          (on) => void save({ showCost: on }),
        )}
      >
        <div className="flex items-center justify-between gap-5 border-t pt-3 pl-5">
          <div className="flex min-w-0 flex-col gap-[3px]">
            <label
              htmlFor="settings-agent-estimate-cost"
              className="font-medium transition-opacity data-[off=true]:opacity-60"
              data-off={prefs?.showCost === false}
            >
              {t('settingsAgent.cost.estimate')}
            </label>
            <div
              id="settings-agent-estimate-cost-hint"
              className="text-[13px] leading-[1.45] [overflow-wrap:anywhere] text-muted-foreground"
            >
              {t('settingsAgent.cost.estimateHint', {
                version: version ?? '…',
              })}
              {prefs?.showCost === false && (
                <> {t('settingsAgent.cost.estimateOff')}</>
              )}
            </div>
          </div>
          <div className="flex shrink-0 items-center">
            {control(
              'settings-agent-estimate-cost',
              prefs?.estimateCost,
              (on) => void save({ estimateCost: on }),
              prefs?.showCost === false,
            )}
          </div>
        </div>
      </SettingRow>
    </>
  )
}

function AiNote() {
  const { t } = useI18n()
  const prefs = useViewerPreferences().data
  if (
    !prefs ||
    prefs.assistantMayRead ||
    prefs.buildSemanticIndex ||
    prefs.mcpMayRead
  )
    return null
  return (
    <p className="animate-rise px-1 text-xs text-muted-foreground">
      {t('settingsAgent.ai.allOff')}
    </p>
  )
}

function OffNote() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.agent')}>
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        {t('settingsAgent.off')}
      </p>
      <Button variant="outline" size="sm" className="self-start" asChild>
        <Link to="/settings/kinds">{t('settings.nav.kinds')}</Link>
      </Button>
    </SettingsSection>
  )
}

export function AgentSection() {
  const { t } = useI18n()
  const { row, readable } = useAgentKind()
  const viewer = useViewerPreferences()
  const projects = useProjectList({ limit: 1 })
  useAgentProjectsSync()
  if (row?.state === 'off') return <OffNote />
  const home = readable ? (projects.data?.homeFolder ?? null) : null

  return (
    <SettingsSection title={t('settings.nav.agent')}>
      <div className="-mt-2 mb-1 flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[13px] leading-relaxed text-muted-foreground">
        <p>{t('settingsAgent.backupNote')}</p>
        <Link
          to={spaceBackupPath('agent')}
          className="font-medium text-brand hover:underline"
        >
          {t('settingsAgent.openBackup')}
        </Link>
      </div>
      {viewer.isError && (
        <div className="rounded-xl border bg-card px-[18px] py-4 shadow-card">
          <CardError
            error={viewer.error}
            onRetry={() => void viewer.refetch()}
          />
        </div>
      )}

      <SettingsGroup
        title={t('settingsAgent.grouping.title')}
        note={t('settingsAgent.grouping.intro')}
      />
      <ScratchRow home={home} />
      <SuggestionsRow projects={projects} />

      <SettingsGroup title={t('settingsAgent.reader.title')} />
      <ViewerSwitchRow
        field="readerCollapseToolCalls"
        title="settingsAgent.reader.collapse"
        hint="settingsAgent.reader.collapseHint"
      />
      <ViewerSwitchRow
        field="readerShowThinking"
        title="settingsAgent.reader.thinking"
        hint="settingsAgent.reader.thinkingHint"
      />

      <SettingsGroup
        title={t('settingsAgent.cost.title')}
        note={t('settingsAgent.cost.intro')}
      />
      <CostRows version={viewer.data?.priceTableVersion} />

      <SettingsGroup
        title={t('settingsAgent.ai.title')}
        note={t('settingsAgent.ai.intro')}
      />
      <ViewerSwitchRow
        field="assistantMayRead"
        title="settingsAgent.ai.assistant"
        hint="settingsAgent.ai.assistantHint"
      />
      <ViewerSwitchRow
        field="buildSemanticIndex"
        title="settingsAgent.ai.semantic"
        hint="settingsAgent.ai.semanticHint"
        under={semanticStatus}
      />
      <ViewerSwitchRow
        field="mcpMayRead"
        title="settingsAgent.ai.mcp"
        hint="settingsAgent.ai.mcpHint"
      />
      <AiNote />

      <SettingsGroup title={t('settingsAgent.resume.title')} />
      <SettingRow
        title={t('settingsAgent.resume.row')}
        description={t('settingsAgent.resume.rowHint')}
      />
      <p className="px-1 text-xs leading-relaxed text-muted-foreground">
        {t('settingsAgent.resume.notYet')}
      </p>
    </SettingsSection>
  )
}
