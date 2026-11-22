/**
 * What switching the semantic search model does, said before it happens, and
 * the confirmation the model picker asks for. Mirrors
 * src/features/ai-setup/model-switch.tsx, worked out from the fake state.
 */
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'
import { num } from '~/data/format'
import { appBytes, STATIC_MODEL_APPROX_BYTES } from './providers'
import { IN_APP, IN_APP_MODEL, TOTAL_VISITS, useAi } from './state'

export interface SwitchTarget {
  /** Null for a service that is not saved yet. */
  providerId: string | null
  model: string
  /** The embeddings service that runs it; null for the in-app model. */
  service: { name: string; local: boolean } | null
}

export function SwitchConsequences({ target, dataNote = true }: { target: SwitchTarget; dataNote?: boolean }) {
  const ai = useAi()
  const modelOf = (id: string) => (id === IN_APP ? IN_APP_MODEL : (ai.embeddingServices.find((p) => p.id === id)?.defaultModel ?? id))
  // The complete index search reads: the chosen one, or the one kept while another is built.
  const completeId = ai.chosenComplete ? ai.modelId : ai.servingOther
  const isTarget = (id: string | null) => id !== null && id === target.providerId && (id === IN_APP || modelOf(id) === target.model)
  const model = target.model
  const lines: string[] = []

  if (isTarget(completeId)) {
    lines.push(`用 ${model} 建立的索引已經完整，語意搜尋會直接用它。`)
  } else {
    lines.push(
      ai.enabled
        ? `${model} 要有自己的索引：在背景讀取 ${num(TOTAL_VISITS)} 次造訪，等你按「建立索引」才開始，在那之前什麼都不會做。`
        : `${model} 要有自己的索引：開啟語意搜尋後在背景讀取 ${num(TOTAL_VISITS)} 次造訪。`,
    )
    if (completeId) {
      const old = modelOf(completeId)
      lines.push(`完成之前，語意搜尋繼續用 ${old}。`)
      lines.push(`新索引完成後，用 ${old} 建立的索引會被刪除。之後想換回 ${old}，就得重新建它的索引。`)
    } else {
      lines.push('完成之前，依意思搜尋只找得到已經建立索引的造訪。')
    }
  }

  const unfinished = !ai.chosenComplete && ai.done > 0 && !isTarget(ai.modelId) ? ai.done : 0
  if (unfinished > 0) {
    lines.push(`用 ${ai.modelName} 建立、還沒完成的索引（${num(unfinished)} 次造訪）不會保留：下次建立或更新索引時就會刪除。`)
  }

  if (target.service) {
    lines.push(`之後每次語意搜尋都要經過 ${target.service.name}。它沒開或連不上時，語意搜尋就不能用，也沒有別的模型頂替；關鍵字搜尋照常可用。`)
    if (dataNote && !target.service.local) lines.push('線上服務。每個頁面的標題、網站、網址裡的字詞與儲存的網頁內文，以及你的搜尋，都會傳給它。')
  } else {
    lines.push('它是靜態嵌入模型：非常快，但找意思相近的頁面不如一般的上下文嵌入模型。')
    if (!ai.modelDownloaded) lines.push(`建索引前要先下載一次，約 ${appBytes(STATIC_MODEL_APPROX_BYTES)}。`)
  }

  return (
    <ul className="flex list-disc flex-col gap-1.5 pl-5 text-[13px] text-foreground">
      {lines.map((line) => (
        <li key={line}>{line}</li>
      ))}
    </ul>
  )
}

/** The model picker's confirmation: nothing changes until it is accepted. */
export function SwitchModelDialog({ target, name, onCancel, onConfirm }: { target: SwitchTarget | null; name: string; onCancel: () => void; onConfirm: () => void }) {
  return (
    <AlertDialog open={target !== null} onOpenChange={(open) => !open && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>把語意搜尋換成 {name}？</AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div>{target && <SwitchConsequences target={target} />}</div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>取消</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>換模型</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
