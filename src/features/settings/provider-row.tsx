/**
 * Which AI service the assistant uses, plus add / test / remove for it.
 * The actions come from `useAiProviders`; the add form is the shared
 * `AddProviderDialog` that onboarding also uses.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
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
import { AddProviderDialog } from '@/features/ai-setup/add-provider-dialog'
import {
  isLocalProvider,
  providerLabel,
  selectedLlmProvider,
} from '@/features/ai-setup/providers'
import { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AiProviderConfig } from '@/lib/types'
import { RowSelect } from './row-select'

const NONE = '__none'
const ADD = '__add'

export function ProviderRow() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const providers = useAiProviders()
  const [adding, setAdding] = useState(false)
  const selected = selectedLlmProvider(ai)

  const choose = (value: string) => {
    if (value === ADD) return setAdding(true)
    void providers.select(value === NONE ? null : value).catch((error) =>
      toast.error(
        t('settingsAi.provider.selectFailed', {
          message: describeError(error),
        }),
      ),
    )
  }

  return (
    <SettingRow
      title={t('settingsAi.provider.title')}
      description={t('settingsAi.provider.description')}
      htmlFor="settings-provider"
      control={
        <RowSelect
          id="settings-provider"
          value={selected?.id ?? NONE}
          disabled={providers.saving}
          onChange={choose}
          options={[
            { value: NONE, label: t('settingsAi.provider.none') },
            ...ai.llmProviders.map((provider) => ({
              value: provider.id,
              label: providerLabel(provider),
            })),
            { value: ADD, label: t('settingsAi.provider.add') },
          ]}
        />
      }
    >
      {selected && (
        <ProviderDetails provider={selected} providers={providers} />
      )}
      <AddProviderDialog
        open={adding}
        onOpenChange={setAdding}
        onAdd={async (draft) => {
          const provider = await providers.add(draft)
          toast.success(t('settingsAi.provider.saved', { name: provider.name }))
        }}
      />
    </SettingRow>
  )
}

function ProviderDetails({
  provider,
  providers,
}: {
  provider: AiProviderConfig
  providers: ReturnType<typeof useAiProviders>
}) {
  const { t } = useI18n()
  const [confirmRemove, setConfirmRemove] = useState(false)
  const local = isLocalProvider(provider)
  const report = providers.reports[provider.id]
  const testing = providers.testing === provider.id

  const result =
    report instanceof Error
      ? {
          ok: false,
          text: t('settingsAi.provider.testFailed', {
            message: describeError(report),
          }),
        }
      : report
        ? report.ok
          ? {
              ok: true,
              text: t('settingsAi.provider.testOk', {
                model: report.model ?? provider.defaultModel,
                ms: report.latencyMs ?? 0,
              }),
            }
          : {
              ok: false,
              text: t('settingsAi.provider.testFailed', {
                message: report.message ?? '',
              }),
            }
        : null

  return (
    <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
      <p className="text-muted-foreground">
        {local
          ? t('settingsAi.provider.localNote')
          : t('settingsAi.provider.remoteNote')}
      </p>
      {!local && !provider.apiKeySaved && (
        <p className="text-destructive">
          {t('settingsAi.provider.keyMissing')}
        </p>
      )}
      {result && (
        <p
          role="status"
          className={result.ok ? 'text-green' : 'text-destructive'}
        >
          {result.text}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => setConfirmRemove(true)}
        >
          {t('settingsAi.provider.remove')}
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={testing}
          onClick={() => void providers.test(provider.id)}
        >
          {testing
            ? t('settingsAi.provider.testing')
            : t('settingsAi.provider.test')}
        </Button>
      </div>
      <AlertDialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsAi.provider.removeTitle', { name: provider.name })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsAi.provider.removeBody')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() =>
                void providers
                  .remove(provider.id)
                  .catch(() =>
                    toast.error(t('settingsAi.provider.removeFailed')),
                  )
              }
            >
              {t('settingsAi.provider.remove')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
