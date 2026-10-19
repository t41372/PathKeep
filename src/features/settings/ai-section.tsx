/**
 * Settings → AI: local semantic search, the assistant's AI service, MCP
 * access for outside tools and what they read, rebuilding the search index, and how search by
 * meaning ranks results. Everything here is optional; PathKeep works
 * without any of it.
 */
import { useQuery } from '@tanstack/react-query'
import { Check, Copy } from 'lucide-react'
import { useState } from 'react'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useSemanticIndex } from '@/features/ai-setup/use-semantic-index'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AiSettings } from '@/lib/types'
import { McpLogRow } from './mcp-log-row'
import { ProviderRow } from './provider-row'
import { RebuildDialog } from './rebuild-dialog'
import { SearchTuningRow } from './search-tuning-row'
import { SemanticRow, type SemanticIndex } from './semantic-row'
import { useRebuild } from './use-rebuild'
import { useSaveSetting } from './use-save-setting'

export function AiSection() {
  const { t } = useI18n()
  const index = useSemanticIndex()
  return (
    <SettingsSection title={t('settingsAi.title')}>
      <p className="-mt-2 mb-1 text-[13px] text-muted-foreground">
        {t('settingsAi.optionalNote')}
      </p>
      <SemanticRow index={index} />
      <ProviderRow />
      <McpRow />
      <McpLogRow />
      <RebuildRow index={index} />
      <SearchTuningRow />
    </SettingsSection>
  )
}

/** The master AI switch stays on while any other AI feature still needs it. */
function masterStaysOn(ai: AiSettings) {
  return ai.semanticIndexEnabled || ai.assistantEnabled || ai.skillEnabled
}

function McpRow() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const { save, saving } = useSaveSetting()
  const on = ai.enabled && ai.mcpEnabled

  const toggle = (next: boolean) =>
    void save((config) => {
      config.ai.mcpEnabled = next
      config.ai.enabled = next || masterStaysOn(config.ai)
      return config
    })

  return (
    <SettingRow
      title={t('settingsAi.mcp.title')}
      description={t('settingsAi.mcp.description')}
      htmlFor="settings-mcp"
      control={
        <Switch
          id="settings-mcp"
          checked={on}
          disabled={saving}
          onCheckedChange={toggle}
        />
      }
    >
      {on && <McpDetails />}
    </SettingRow>
  )
}

function McpDetails() {
  const { t } = useI18n()
  const [copied, setCopied] = useState(false)
  const query = useQuery({
    queryKey: ['ai', 'integrations'],
    queryFn: intelligenceClient.previewIntegrations,
  })

  const copy = (text: string) =>
    void navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    })

  return (
    <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
      <p className="text-muted-foreground">{t('settingsAi.mcp.how')}</p>
      <span className="font-medium">{t('settingsAi.mcp.details')}</span>
      {query.isPending ? (
        <Skeleton className="h-9 w-full" />
      ) : query.isError ? (
        <p className="text-destructive">{t('settingsAi.mcp.loadFailed')}</p>
      ) : (
        <div className="flex items-start gap-2 rounded-lg bg-muted p-2.5">
          <code className="min-w-0 flex-1 font-mono text-xs break-all">
            {query.data.mcpCommand}
          </code>
          <Button
            size="icon-xs"
            variant="ghost"
            aria-label={
              copied ? t('settingsAi.mcp.copied') : t('settingsAi.mcp.copy')
            }
            onClick={() => copy(query.data.mcpCommand)}
          >
            {copied ? <Check /> : <Copy />}
          </Button>
        </div>
      )}
    </div>
  )
}

function RebuildRow({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const start = useRebuild(index)
  const busy = index.build !== null || start.isPending

  return (
    <SettingRow
      title={t('settingsAi.rebuild.title')}
      description={
        index.enabled
          ? t('settingsAi.rebuild.description')
          : t('settingsAi.rebuild.needsSemantic')
      }
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={!index.enabled || busy}
          onClick={() => setOpen(true)}
        >
          {t('settingsAi.rebuild.action')}
        </Button>
      }
    >
      <RebuildDialog
        open={open}
        onOpenChange={setOpen}
        onConfirm={() => start.mutate()}
      />
    </SettingRow>
  )
}
