/**
 * Rename (`agent-history.md` §4.3): a title only PathKeep shows, kept with
 * the conversation's other marks; the tool's files are not touched. An
 * empty name goes back to the title the tool or the first prompt gave.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { ChatSummary } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import type { ChatMarks } from '@/features/agent-chats/chat-marks'

function RenameForm({
  summary,
  marks,
  onDone,
}: {
  summary: ChatSummary
  marks: ChatMarks
  onDone: () => void
}) {
  const { t } = useI18n()
  const [name, setName] = useState(summary.title ?? '')
  const [saving, setSaving] = useState(false)
  const unchanged = name.trim() === (summary.title ?? '').trim()
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault()
        if (unchanged || saving) return
        setSaving(true)
        void marks.rename(summary.id, name.trim()).then((header) => {
          setSaving(false)
          if (!header) return
          toast.success(t('agentReader.rename.saved'))
          onDone()
        })
      }}
    >
      <div className="flex flex-col gap-2">
        <Label htmlFor="chat-name">{t('agentReader.rename.name')}</Label>
        <Input
          id="chat-name"
          value={name}
          autoFocus
          onChange={(event) => setName(event.target.value)}
        />
        <p className="text-[12.5px] text-muted-foreground">
          {t('agentReader.rename.help')}
        </p>
      </div>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onDone}>
          {t('agentReader.rename.cancel')}
        </Button>
        <Button type="submit" disabled={unchanged || saving}>
          {t('agentReader.rename.save')}
        </Button>
      </DialogFooter>
    </form>
  )
}

export function RenameDialog({
  summary,
  marks,
  open,
  onOpenChange,
}: {
  summary: ChatSummary
  marks: ChatMarks
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[440px]">
        <DialogHeader>
          <DialogTitle>{t('agentReader.rename.title')}</DialogTitle>
          <DialogDescription className="sr-only">
            {t('agentReader.rename.help')}
          </DialogDescription>
        </DialogHeader>
        {open && (
          <RenameForm
            summary={summary}
            marks={marks}
            onDone={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}
