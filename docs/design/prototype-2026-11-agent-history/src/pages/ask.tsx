/**
 * Ask, shared by every data type. It starts scoped to the space you came
 * from; the other types are one tap away. Agent conversations stay locked
 * until allowed in Settings › Agent 對話 › AI 存取, because they often hold
 * code, keys and private paths.
 */
import { ArrowUp, Check, Lock } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/cn'
import { kindById, kinds, type Kind, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'

const examples: Partial<Record<KindId, string[]>> = {
  browsing: ['上個月我在看哪些跟 SQLCipher 有關的網頁？', '我上週末都在逛什麼？', '幫我找回那篇講台南老街的影片'],
  agent: ['上次 Safari 匯入的時區問題是怎麼修的？', '這週哪個專案花最多時間？', '我叫 Agent 做過哪些資料庫 migration？'],
}

/** Whether Ask may read this type. Agent is off until the person allows it in Settings. */
const readable = (k: Kind) => k.real && k.id !== 'agent'

function Scope({ initial }: { initial: KindId[] }) {
  const { enabledKinds } = useProto()
  const shown = kinds.filter((k) => enabledKinds.includes(k.id))
  const [picked, setPicked] = useState<Set<KindId>>(() => new Set(initial))
  const active = useMemo(() => shown.filter((k) => picked.has(k.id) && readable(k)), [shown, picked])
  const [text, setText] = useState('')
  const lockedHere = shown.filter((k) => picked.has(k.id) && !readable(k))
  const toggle = (id: KindId) =>
    setPicked((p) => {
      const n = new Set(p)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })
  const hints = active.flatMap((k) => examples[k.id] ?? []).slice(0, 3)

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
                disabled={!k.real}
                onClick={() => toggle(k.id)}
                className={cn(
                  'flex items-center gap-1 rounded-full border px-2.5 py-0.5 transition-colors',
                  on && can && 'bg-card shadow-card',
                  on && !can && 'border-dashed bg-card/60',
                  !on && 'border-dashed text-muted-foreground hover:text-foreground',
                  !k.real && 'cursor-default opacity-60',
                )}
              >
                {on ? can ? <Check className="size-3.5 text-green" /> : <Lock className="size-3" /> : <k.icon className="size-3.5" />}
                {k.label}
                {!k.real && <span className="ml-0.5">（規劃中）</span>}
              </button>
              {k.id === 'agent' && on && (
                <Link to="/settings/agent" className="text-[12px] text-brand hover:underline">
                  在設定裡允許
                </Link>
              )}
            </span>
          )
        })}
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
            <h1 className="text-[22px] font-semibold tracking-[-0.01em]">想找什麼？</h1>
            <p className="max-w-[460px] text-center text-[13px] leading-relaxed text-muted-foreground">
              只會讀{active.map((k) => `「${k.label}」`).join('和')}。助理用你在「設定 › AI」裡選的模型，回答時會標出引用了哪幾筆紀錄。
            </p>
            <div className="flex flex-wrap justify-center gap-2">
              {hints.map((e) => (
                <button key={e} type="button" onClick={() => setText(e)} className="rounded-full border bg-card px-3 py-1.5 text-[13px] text-muted-foreground shadow-card transition-colors hover:text-foreground">
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
  const { nav, space, enabledKinds } = useProto()
  // Spaces: start with the space you came from. Round-2 shell: every type the assistant may read.
  const initial = nav === 'spaces' ? [space] : enabledKinds.filter((id) => readable(kindById[id]))
  return <Scope key={`${nav}-${space}`} initial={initial} />
}
