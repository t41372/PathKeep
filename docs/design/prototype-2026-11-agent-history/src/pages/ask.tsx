/**
 * Ask, shared by every data type. Only the scope row at the top and the
 * states it brings are proposed here; the rest of today's Ask page (chat
 * list, streaming answers, citations, model choice, and so on) is not redrawn
 * and stays as it is. A new chat starts scoped to the space you came from.
 * Agent conversations stay locked until allowed in Settings › Agent 對話 ›
 * AI 存取, because they often hold code, keys and private paths.
 */
import { ArrowUp, Check, Lock } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { NotRedrawn } from '~/components/not-redrawn'
import { kinds, type Kind, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'

/** Today's four examples; the four for Agent 對話 are written when that screen is built. */
const browsingExamples = ['這週我花最多時間在什麼上？', '找回上個月我讀過的那篇 Rust 文章', '最近我反覆打開了哪些網站？', '上個月底我在研究什麼？']
const agentExamples = ['（Agent 對話的範例一，做這個畫面時寫）', '（範例二）', '（範例三）', '（範例四）']

function Scope({ initial }: { initial: KindId[] }) {
  const { enabledKinds } = useProto()
  // Prototype-only: as if 設定 › Agent 對話 › AI 存取 › 助理 were on.
  const [agentAllowed, setAgentAllowed] = useState(false)
  /** Whether Ask may read this type. Agent is off until the person allows it in Settings. */
  const readable = (k: Kind) => k.id !== 'agent' || agentAllowed
  // Types that are not built yet appear nowhere, here included.
  const shown = kinds.filter((k) => k.real && enabledKinds.includes(k.id))
  const [picked, setPicked] = useState<Set<KindId>>(() => new Set(initial))
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const active = useMemo(() => shown.filter((k) => picked.has(k.id) && readable(k)), [shown, picked, agentAllowed])
  const [text, setText] = useState('')
  const lockedHere = shown.filter((k) => picked.has(k.id) && !readable(k))
  const toggle = (id: KindId) =>
    setPicked((p) => {
      const n = new Set(p)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })
  const hints = active.some((k) => k.id === 'browsing') ? browsingExamples : agentExamples

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b px-6 py-3 text-[13px]">
        <span className="text-muted-foreground">問的範圍：</span>
        {shown.map((k) => {
          const can = readable(k)
          const on = picked.has(k.id)
          return (
            <span key={k.id} className="flex items-center gap-1">
              <button
                type="button"
                aria-pressed={on}
                onClick={() => toggle(k.id)}
                className={cn(
                  'flex items-center gap-1 rounded-full border px-2.5 py-0.5 transition-colors',
                  on && can && 'bg-card shadow-card',
                  on && !can && 'border-dashed bg-card/60',
                  !on && 'border-dashed text-muted-foreground hover:text-foreground',
                )}
              >
                {on ? can ? <Check className="size-3.5 text-green" /> : <Lock className="size-3" /> : <k.icon className="size-3.5" />}
                {k.label}
              </button>
              {k.id === 'agent' && on && !can && (
                <Link to="/settings/agent" className="text-[12px] text-brand hover:underline">
                  在設定裡允許
                </Link>
              )}
            </span>
          )
        })}
        <span className="flex-1" />
        <label className="flex items-center gap-2 rounded-lg border border-dashed px-2.5 py-1 text-xs text-muted-foreground">
          原型：設定裡已允許助理讀 Agent 對話
          <Switch checked={agentAllowed} onCheckedChange={setAgentAllowed} aria-label="原型：設定裡已允許助理讀 Agent 對話" />
        </label>
      </div>
      <div className="px-6 pt-4">
        <NotRedrawn page="助理" />
      </div>
      <div className="flex flex-1 flex-col items-center justify-center gap-5 px-6">
        {active.length === 0 ? (
          <div className="flex max-w-[460px] flex-col items-center gap-3 text-center">
            <Lock className="size-6 text-muted-foreground" />
            <h1 className="text-[18px] font-semibold">{lockedHere.length ? `助理還不能讀「${lockedHere.map((k) => k.label).join('、')}」` : '選一個類型來問'}</h1>
            <p className="text-[13px] leading-relaxed text-muted-foreground">
              Agent 對話裡常有程式碼、金鑰和私人路徑，所以預設不給助理讀。到「設定 › Agent 對話 › AI 存取」打開後才會用；或先在上面選別的類型來問。
            </p>
            <Button variant="outline" asChild>
              <Link to="/settings/agent">到設定允許</Link>
            </Button>
          </div>
        ) : (
          <>
            <h1 className="text-[22px] font-semibold tracking-[-0.01em]">問問你走過的路</h1>
            <p className="max-w-[460px] text-center text-[13px] leading-relaxed text-muted-foreground">
              只會讀{active.map((k) => `「${k.label}」`).join('和')}。（後面接現在空對話的說明：用的是本機模型還是線上服務，線上服務時會把需要的片段送出去。）
            </p>
            <span className="text-xs text-muted-foreground">可以這樣問</span>
            <div className="flex max-w-[620px] flex-wrap justify-center gap-2">
              {hints.map((e) => (
                <button key={e} type="button" onClick={() => toast(`原型：這裡會直接送出「${e}」，跟現在一樣`)} className="rounded-full border bg-card px-3 py-1.5 text-[13px] text-muted-foreground shadow-card transition-colors hover:text-foreground">
                  {e}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
      <div className="mx-auto w-full max-w-[760px] px-6 pb-6">
        <div className="flex items-end gap-2 rounded-2xl border bg-card p-2 shadow-card">
          <Textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={active.length ? `問問你的${active.map((k) => k.label).join('和')}…` : '先選一個助理能讀的類型'}
            disabled={active.length === 0}
            rows={1}
            className="min-h-10 resize-none border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent"
          />
          <Button size="icon" className="shrink-0 rounded-xl" disabled={!text.trim() || active.length === 0} aria-label="送出">
            <ArrowUp />
          </Button>
        </div>
      </div>
    </div>
  )
}

export default function AskPage() {
  const { space } = useProto()
  // A new chat starts with the space you came from.
  return <Scope key={space} initial={[space]} />
}
