/**
 * Adding an AI service, or changing one, as src/features/ai-setup/provider-dialog.tsx:
 * the assistant's (`llm`) or semantic search's embeddings service (`embedding`).
 */
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { SwitchConsequences } from './model-switch'
import { ProviderForm } from './provider-form'
import { draftFromProvider, draftIsComplete, emptyDraft, isLocalUrl, type ProviderConfig, type ProviderDraft, type Purpose } from './providers'
import { useAi } from './state'

export function ProviderDialog({
  open,
  onOpenChange,
  onSave,
  purpose = 'llm',
  editing = null,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSave: (draft: ProviderDraft) => void
  purpose?: Purpose
  editing?: ProviderConfig | null
}) {
  const [saving, setSaving] = useState(false)
  return (
    <Dialog open={open} onOpenChange={(next) => !saving && onOpenChange(next)}>
      <DialogContent className="sm:max-w-md">
        <Body purpose={purpose} editing={editing} saving={saving} setSaving={setSaving} onSave={onSave} onDone={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  )
}

function Body({
  purpose,
  editing,
  saving,
  setSaving,
  onSave,
  onDone,
}: {
  purpose: Purpose
  editing: ProviderConfig | null
  saving: boolean
  setSaving: (saving: boolean) => void
  onSave: (draft: ProviderDraft) => void
  onDone: () => void
}) {
  const { listCase } = useAi()
  const [draft, setDraft] = useState<ProviderDraft>(() => (editing ? draftFromProvider(editing) : emptyDraft(undefined, purpose)))
  const keySaved = Boolean(editing?.apiKeySaved)
  const embedding = purpose === 'embedding'
  const title = editing ? `編輯 ${editing.name}` : embedding ? '新增嵌入服務' : '新增 AI 服務'
  const description = editing
    ? '變更 PathKeep 連線到這個服務的方式。'
    : embedding
      ? '把文字轉成嵌入向量的服務，需相容 OpenAI。為了建立索引，語意搜尋會把每個頁面的標題、網站、網址裡的字詞，以及儲存的網頁內文（如果有）傳給它，也會傳送你搜尋過的每一組關鍵字；搜尋時也會把你的搜尋傳給它。'
      : '選擇助理要從哪裡取得回答。'
  const local = isLocalUrl(draft.baseUrl)
  const note = embedding
    ? local
      ? '在這台電腦上執行，頁面與搜尋都留在這裡。'
      : '線上服務。每個頁面的標題、網站、網址裡的字詞與儲存的網頁內文，以及你的搜尋，都會傳給它。'
    : local
      ? '在這台電腦上執行，不會向任何地方傳送內容。'
      : '這是線上服務。你提問時，助理找到的頁面會傳送給它。'
  const complete = draftIsComplete(draft, { purpose, keySaved })
  const model = draft.model.trim()
  const switches = embedding && model !== '' && (!editing || model !== editing.defaultModel)

  const submit = () => {
    setSaving(true)
    window.setTimeout(() => {
      onSave(draft)
      setSaving(false)
      onDone()
    }, 500)
  }

  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={(event) => {
        event.preventDefault()
        if (complete && !saving) submit()
      }}
    >
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{description}</DialogDescription>
      </DialogHeader>
      <ProviderForm draft={draft} onChange={setDraft} idPrefix={editing ? 'edit-provider' : 'add-provider'} purpose={purpose} keySaved={keySaved} listCase={listCase} />
      <p className="text-xs text-muted-foreground">{note}</p>
      {switches && (
        <section role="note" className="flex flex-col gap-1.5">
          <p className="text-[13px] font-medium">儲存後，語意搜尋就換成這個模型：</p>
          <SwitchConsequences target={{ providerId: editing?.id ?? null, model, service: { name: draft.name.trim() || model, local } }} dataNote={false} />
        </section>
      )}
      <DialogFooter>
        <Button type="button" variant="ghost" disabled={saving} onClick={onDone}>
          取消
        </Button>
        <Button type="submit" disabled={!complete || saving}>
          {saving ? '正在儲存…' : editing ? '儲存' : '新增服務'}
        </Button>
      </DialogFooter>
    </form>
  )
}
