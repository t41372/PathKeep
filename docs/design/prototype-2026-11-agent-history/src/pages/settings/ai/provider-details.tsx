/**
 * The panel under a chosen AI service, for the assistant's or semantic
 * search's: what is sent, a missing key, the check made when it was saved,
 * its connection test, and edit and remove. Mirrors
 * src/features/settings/provider-details.tsx.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { ProviderDialog } from './provider-dialog'
import { isLocalProvider, type ProviderConfig, type ProviderDraft, type Purpose } from './providers'
import { useAi, type TestCase } from './state'

/** settingsAi.provider.testErrors, by the reason the backend classified. */
function testFailure(code: TestCase, provider: ProviderConfig) {
  const address = provider.baseUrl ?? ''
  switch (code) {
    case 'secret-missing':
      return '服務不接受這個 API 金鑰。檢查金鑰後再測試一次。'
    case 'network-error':
      return `連不上 ${address}。檢查位址，以及服務是否在執行。`
    case 'rate-limited':
      return '服務暫時限制了請求，稍後再試。'
    case 'bad-model':
      return `服務上沒有叫 ${provider.defaultModel} 的模型。檢查模型名稱。`
    case 'bad-endpoint':
      return `${address} 有服務在執行，但不是它的 API 位址。LM Studio 和 Ollama 的位址通常以 /v1 結尾，例如 http://localhost:1234/v1。`
    case 'unsupported-capability':
      return '這個服務不能用在這裡。'
    case 'provider-disabled':
      return '這個服務已關閉。'
    default:
      return '服務回傳了錯誤：HTTP 500 Internal Server Error'
  }
}

export function ProviderDetails({
  provider,
  purpose,
  onUpdate,
  onRemove,
  hasIndex = false,
}: {
  provider: ProviderConfig
  purpose: Purpose
  onUpdate: (draft: ProviderDraft) => void
  onRemove: () => void
  /** Semantic search has an index made with this service, which removing it gives up. */
  hasIndex?: boolean
}) {
  const ai = useAi()
  const [confirmRemove, setConfirmRemove] = useState(false)
  const [editing, setEditing] = useState(false)
  const local = isLocalProvider(provider)
  const embedding = purpose === 'embedding'
  const report = ai.reports[provider.id]
  const testing = ai.testing === provider.id

  const result = report
    ? report.ok
      ? { ok: true, text: `已連線到 ${provider.defaultModel}，耗時 ${report.ms} 毫秒。` }
      : report.code === 'thrown'
        ? { ok: false, text: '無法連線。error sending request for url (http://localhost:11434/v1/chat/completions)' }
        : { ok: false, text: `無法連線。${testFailure(report.code, provider)}` }
    : null
  const checked = result ? null : ai.checks[provider.id]
  const checkLine = checked && {
    ok: checked.kind === 'found',
    busy: checked.kind === 'checking',
    text:
      checked.kind === 'checking'
        ? `正在檢查 ${provider.name}…`
        : checked.kind === 'found'
          ? `${provider.name} 有回應，也有 ${provider.defaultModel}。「測試連線」會實際執行一次模型。`
          : checked.kind === 'missing'
            ? `${provider.name} 有回應，但沒有叫 ${provider.defaultModel} 的模型。請檢查模型名稱。`
            : `沒能讀取 ${provider.name} 的模型：error sending request for url (${provider.baseUrl ?? ''}/models)`,
  }

  return (
    <div className="flex flex-col gap-2 border-t pt-3 text-[13px]">
      <p className="text-muted-foreground">
        {embedding
          ? local
            ? '在這台電腦上執行，頁面與搜尋都留在這裡。'
            : '線上服務。每個頁面的標題、網站、網址裡的字詞與儲存的網頁內文，以及你的搜尋，都會傳給它。'
          : local
            ? '在這台電腦上執行，不會向任何地方傳送內容。'
            : '這是線上服務。你提問時，助理找到的頁面會傳送給它。'}
      </p>
      {!local && !provider.apiKeySaved && <p className="text-destructive">還沒有為這個服務儲存 API 金鑰。</p>}
      {result && (
        <p role="status" className={result.ok ? 'text-green' : 'text-destructive'}>
          {result.text}
        </p>
      )}
      {checkLine && (
        <p role="status" className={checkLine.busy ? 'text-muted-foreground' : checkLine.ok ? 'text-green' : 'text-destructive'}>
          {checkLine.text}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={() => setConfirmRemove(true)}>
          移除
        </Button>
        <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
          編輯
        </Button>
        <Button size="sm" variant="outline" disabled={testing} onClick={() => ai.test(provider.id)}>
          {testing ? '測試中…' : '測試連線'}
        </Button>
      </div>
      <AlertDialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>要移除 {provider.name} 嗎？</AlertDialogTitle>
            <AlertDialogDescription>
              {embedding
                ? hasIndex
                  ? '這個服務與它儲存的 API 金鑰會從這台電腦上移除。用它建立的索引少了它就不能用，會被刪除。語意搜尋改回使用 App 內建靜態模型，要先建好它自己的索引。'
                  : '這個服務與它儲存的 API 金鑰會從這台電腦上移除，語意搜尋改回使用 App 內建靜態模型。'
                : '這個服務和已儲存的 API 金鑰會從這台電腦移除。你的對話會保留。'}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={onRemove}>移除</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <ProviderDialog
        open={editing}
        onOpenChange={setEditing}
        purpose={purpose}
        editing={provider}
        onSave={(draft) => {
          onUpdate(draft)
          toast.success(`${draft.name.trim() || provider.name} 已儲存。`)
        }}
      />
    </div>
  )
}
