/**
 * The panel under a chosen AI service, for the assistant's or semantic
 * search's: where its requests go, its connection test, and edit and remove.
 * The actions come from `useAiProviders` for that purpose.
 */
import { useState } from 'react'
import { toast } from 'sonner'
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
import { ProviderDialog } from '@/features/ai-setup/provider-dialog'
import { isLocalProvider } from '@/features/ai-setup/providers'
import type { useAiProviders } from '@/features/ai-setup/use-ai-providers'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type {
  AiProviderConfig,
  AiProviderConnectionTestReport,
} from '@/lib/types'

/** The reasons `classify_provider_error` gives, each said in the user's language. */
const TEST_ERRORS = [
  'secret-missing',
  'network-error',
  'rate-limited',
  'bad-model',
  'bad-endpoint',
  'unsupported-capability',
  'provider-disabled',
] as const

/**
 * Why a connection test failed, from the reason the backend classified (its
 * own message is English and technical); the message itself only when the
 * reason is unknown.
 */
function testFailure(
  t: ReturnType<typeof useI18n>['t'],
  report: AiProviderConnectionTestReport,
  provider: AiProviderConfig,
) {
  const code = TEST_ERRORS.find((known) => known === report.errorCode)
  if (!code)
    return t('settingsAi.provider.testErrors.other', {
      message: report.message,
    })
  return t(`settingsAi.provider.testErrors.${code}`, {
    address: provider.baseUrl ?? '',
    model: report.model || provider.defaultModel,
  })
}

export function ProviderDetails({
  provider,
  providers,
  onModelChanged,
  indexOn = false,
}: {
  provider: AiProviderConfig
  providers: ReturnType<typeof useAiProviders>
  /** After the model was changed and saved: semantic search builds its index again. */
  onModelChanged?: () => void
  /** Semantic search is on, so a new model is indexed as soon as it is saved. */
  indexOn?: boolean
}) {
  const { t } = useI18n()
  const [confirmRemove, setConfirmRemove] = useState(false)
  const [editing, setEditing] = useState(false)
  const local = isLocalProvider(provider)
  const embedding = providers.purpose === 'embedding'
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
                message: testFailure(t, report, provider),
              }),
            }
        : null

  return (
    <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
      <p className="text-muted-foreground">
        {embedding
          ? t(
              local
                ? 'settingsAi.semantic.serviceLocalNote'
                : 'settingsAi.semantic.serviceRemoteNote',
            )
          : t(
              local
                ? 'settingsAi.provider.localNote'
                : 'settingsAi.provider.remoteNote',
            )}
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
        <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
          {t('settingsAi.provider.edit')}
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
              {t(
                embedding
                  ? 'settingsAi.semantic.serviceRemoveBody'
                  : 'settingsAi.provider.removeBody',
              )}
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
      <ProviderDialog
        open={editing}
        onOpenChange={setEditing}
        purpose={providers.purpose}
        editing={provider}
        indexOn={indexOn}
        onSave={async (draft) => {
          const updated = await providers.update(provider.id, draft)
          toast.success(
            t('settingsAi.provider.updated', { name: updated.name }),
          )
          if (updated.defaultModel !== provider.defaultModel) onModelChanged?.()
        }}
      />
    </div>
  )
}
