/** Dialog for adding an AI service from Settings → AI. */
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
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { ProviderForm } from './provider-form'
import {
  draftIsComplete,
  emptyDraft,
  isLocalUrl,
  type ProviderDraft,
} from './providers'

export function AddProviderDialog({
  open,
  onOpenChange,
  onAdd,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onAdd: (draft: ProviderDraft) => Promise<unknown>
}) {
  const { t } = useI18n()
  const [draft, setDraft] = useState<ProviderDraft>(() => emptyDraft())
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    setSaving(true)
    setError(null)
    try {
      await onAdd(draft)
      onOpenChange(false)
      setDraft(emptyDraft())
    } catch (reason) {
      setError(
        t('settingsAi.provider.saveFailed', { message: describeError(reason) }),
      )
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !saving && onOpenChange(next)}>
      <DialogContent className="sm:max-w-md">
        <form
          className="flex flex-col gap-5"
          onSubmit={(event) => {
            event.preventDefault()
            if (draftIsComplete(draft) && !saving) void submit()
          }}
        >
          <DialogHeader>
            <DialogTitle>{t('settingsAi.provider.dialog.title')}</DialogTitle>
            <DialogDescription>
              {t('settingsAi.provider.dialog.description')}
            </DialogDescription>
          </DialogHeader>
          <ProviderForm
            draft={draft}
            onChange={setDraft}
            idPrefix="add-provider"
          />
          <p className="text-xs text-muted-foreground">
            {isLocalUrl(draft.baseUrl)
              ? t('settingsAi.provider.localNote')
              : t('settingsAi.provider.remoteNote')}
          </p>
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
              onClick={() => onOpenChange(false)}
            >
              {t('settingsAi.provider.dialog.cancel')}
            </Button>
            <Button type="submit" disabled={!draftIsComplete(draft) || saving}>
              {saving
                ? t('settingsAi.provider.dialog.saving')
                : t('settingsAi.provider.dialog.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
