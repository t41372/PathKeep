/**
 * Settings → AI: local semantic search, the assistant's AI service, MCP
 * access for outside tools, and rebuilding the search index. Everything here
 * is optional; PathKeep works without any of it.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { Check, Copy } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useSemanticIndex } from '@/features/ai-setup/use-semantic-index'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AiSettings } from '@/lib/types'
import { ProviderRow } from './provider-row'
import { SemanticRow, type SemanticIndex } from './semantic-row'
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
      <RebuildRow index={index} />
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
  const estimate = useQuery({
    queryKey: ['ai', 'reembed-estimate', 'full'],
    queryFn: () => intelligenceClient.estimateReembed('full'),
    enabled: open,
    staleTime: 60_000,
  })
  const start = useMutation({
    mutationFn: () => index.rebuild('full'),
    onSuccess: () => toast.success(t('settingsAi.rebuild.started')),
    onError: (error) =>
      toast.error(
        t('settingsAi.rebuild.failed', { message: describeError(error) }),
      ),
  })

  const busy = index.building || start.isPending
  const minutes = estimate.data
    ? Math.max(1, Math.round(estimate.data.estMinutesCpu))
    : 0

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
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsAi.rebuild.dialogTitle')}
            </AlertDialogTitle>
            {estimate.isPending ? (
              <Skeleton className="h-10 w-full" />
            ) : (
              <AlertDialogDescription>
                {estimate.data && estimate.data.pageCount > 0
                  ? t('settingsAi.rebuild.dialogBody', {
                      count: estimate.data.pageCount,
                      minutes: t('settingsAi.rebuild.minutes', {
                        count: minutes,
                      }),
                    })
                  : t('settingsAi.rebuild.dialogBodyUnknown')}
              </AlertDialogDescription>
            )}
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => start.mutate()}>
              {t('settingsAi.rebuild.confirm')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
