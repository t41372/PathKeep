/**
 * Dialog for adding an AI service, or changing one, from Settings → AI: the
 * assistant's (`llm`) or semantic search's embeddings service (`embedding`).
 * Saving is the caller's (`useAiProviders`); this only collects the fields.
 */
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { AiProviderConfig, AiProviderPurpose } from '@/lib/types'
import { ProviderForm } from './provider-form'
import {
  draftFromProvider,
  draftIsComplete,
  emptyDraft,
  isLocalUrl,
  type ProviderDraft,
} from './providers'

export function ProviderDialog({
  open,
  onOpenChange,
  onSave,
  purpose = 'llm',
  editing = null,
  indexOn = false,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSave: (draft: ProviderDraft) => Promise<unknown>
  purpose?: AiProviderPurpose
  /** The service to change; adds a new one when absent. */
  editing?: AiProviderConfig | null
  /** Semantic search is on, so a new embeddings model's index can be built right away. */
  indexOn?: boolean
}) {
  const [saving, setSaving] = useState(false)
  return (
    <Dialog open={open} onOpenChange={(next) => !saving && onOpenChange(next)}>
      <DialogContent className="sm:max-w-md">
        {/* The content unmounts when closed, so each opening starts afresh. */}
        <ProviderDialogBody
          purpose={purpose}
          editing={editing}
          indexOn={indexOn}
          saving={saving}
          setSaving={setSaving}
          onSave={onSave}
          onDone={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  )
}

function ProviderDialogBody({
  purpose,
  editing,
  indexOn,
  saving,
  setSaving,
  onSave,
  onDone,
}: {
  purpose: AiProviderPurpose
  editing: AiProviderConfig | null
  indexOn: boolean
  saving: boolean
  setSaving: (saving: boolean) => void
  onSave: (draft: ProviderDraft) => Promise<unknown>
  onDone: () => void
}) {
  const { t } = useI18n()
  const [draft, setDraft] = useState<ProviderDraft>(() =>
    editing ? draftFromProvider(editing) : emptyDraft(undefined, purpose),
  )
  const [error, setError] = useState<string | null>(null)
  const keySaved = Boolean(editing?.apiKeySaved)

  const submit = async () => {
    setSaving(true)
    setError(null)
    try {
      await onSave(draft)
      onDone()
    } catch (reason) {
      setError(
        t('settingsAi.provider.saveFailed', { message: describeError(reason) }),
      )
    } finally {
      setSaving(false)
    }
  }

  const embedding = purpose === 'embedding'
  const title = editing
    ? t('settingsAi.provider.editDialog.title', { name: editing.name })
    : embedding
      ? t('settingsAi.semantic.serviceDialog.title')
      : t('settingsAi.provider.dialog.title')
  const description = editing
    ? t('settingsAi.provider.editDialog.description')
    : embedding
      ? t('settingsAi.semantic.serviceDialog.description')
      : t('settingsAi.provider.dialog.description')
  const local = isLocalUrl(draft.baseUrl)
  const note = embedding
    ? t(
        local
          ? 'settingsAi.semantic.serviceLocalNote'
          : 'settingsAi.semantic.serviceRemoteNote',
      )
    : t(
        local
          ? 'settingsAi.provider.localNote'
          : 'settingsAi.provider.remoteNote',
      )
  const complete = draftIsComplete(draft, { purpose, keySaved })
  // A semantic index belongs to one model: another one means building it again.
  const modelChange =
    embedding && editing && draft.model.trim() !== editing.defaultModel
      ? editing.defaultModel
      : null

  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault()
        if (complete && !saving) void submit()
      }}
    >
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{description}</DialogDescription>
      </DialogHeader>
      <ProviderForm
        draft={draft}
        onChange={setDraft}
        idPrefix={editing ? 'edit-provider' : 'add-provider'}
        purpose={purpose}
        keySaved={keySaved}
        providerId={editing?.id ?? null}
      />
      <p className="text-xs text-muted-foreground">{note}</p>
      {modelChange !== null && draft.model.trim() !== '' && (
        <ModelChangeNote old={modelChange} now={indexOn} />
      )}
      {error && (
        <p role="alert" className="text-[13px] text-destructive">
          {error}
        </p>
      )}
      <DialogFooter>
        <Button
          type="button"
          variant="ghost"
          disabled={saving}
          onClick={onDone}
        >
          {t('settingsAi.provider.dialog.cancel')}
        </Button>
        <Button type="submit" disabled={!complete || saving}>
          {saving
            ? t('settingsAi.provider.dialog.saving')
            : editing
              ? t('settingsAi.provider.editDialog.submit')
              : t('settingsAi.provider.dialog.submit')}
        </Button>
      </DialogFooter>
    </form>
  )
}

/**
 * What changing the model means, before it is saved: the new model needs its
 * own index (how many visits), built when the user starts it; until then
 * search keeps the old one. `now`: semantic search is on.
 */
function ModelChangeNote({ old, now }: { old: string; now: boolean }) {
  const { t } = useI18n()
  const format = useFormat()
  const estimate = useQuery({
    queryKey: ['reembed-estimate', 'full'],
    queryFn: () => intelligenceClient.estimateReembed('full'),
    staleTime: 30_000,
  })
  return (
    <p role="note" className="text-[13px] text-foreground">
      {estimate.data
        ? t(
            now
              ? 'settingsAi.semantic.modelChange'
              : 'settingsAi.semantic.modelChangeLater',
            { count: format.number(estimate.data.pageCount), old },
          )
        : t(
            now
              ? 'settingsAi.semantic.modelChangeCounting'
              : 'settingsAi.semantic.modelChangeLaterCounting',
            { old },
          )}
    </p>
  )
}
