/**
 * Search ranking: the hybrid-search knobs (RRF `k`, the two list weights, the
 * starred boost), each with its default, its range and a plain explanation.
 * Edits stay a draft until 儲存. Mirrors src/features/settings/search-tuning-row.tsx.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useAi } from './state'

type Field = 'k' | 'lexical' | 'semantic' | 'starred'

interface FieldSpec {
  field: Field
  label: string
  help: string
  fallback: number
  min: number
  max: number
  step: number
  integer?: boolean
}

const fields: FieldSpec[] = [
  { field: 'k', label: '排名平滑', help: '每份清單前幾名比其餘結果重要多少。數值越小，最前面的結果越占優勢；越大，兩份清單混合得越平均。', fallback: 60, min: 1, max: 1000, step: 1, integer: true },
  { field: 'lexical', label: '文字相符', help: '包含你輸入文字的網頁占多少分量。0 表示不計入。', fallback: 1, min: 0, max: 100, step: 0.1 },
  { field: 'semantic', label: '意思相近', help: '主題相同但不一定含有你輸入文字的網頁占多少分量。0 表示不計入。', fallback: 1, min: 0, max: 100, step: 0.1 },
  { field: 'starred', label: '加星網頁加分', help: '加星的網頁往前排多少。數值保持得很小，加星的網頁不會壓過明顯更相關的結果。', fallback: 0.15, min: 0, max: 0.5, step: 0.05 },
]

type Values = Record<Field, number>
const defaults = Object.fromEntries(fields.map((spec) => [spec.field, spec.fallback])) as Values

function parse(spec: FieldSpec, text: string): number | null {
  if (text.trim() === '') return null
  const value = Number(text)
  if (!Number.isFinite(value) || value < spec.min || value > spec.max) return null
  if (spec.integer && !Number.isInteger(value)) return null
  return value
}

const asText = (values: Values) => Object.fromEntries(fields.map((spec) => [spec.field, String(values[spec.field])])) as Record<Field, string>
const same = (a: Values, b: Values) => fields.every((spec) => a[spec.field] === b[spec.field])

export function SearchTuningRow() {
  const ai = useAi()
  const [saved, setSaved] = useState<Values>(defaults)
  const [draft, setDraft] = useState(() => asText(defaults))
  const parsed = Object.fromEntries(fields.map((spec) => [spec.field, parse(spec, draft[spec.field])])) as Record<Field, number | null>
  const valid = fields.every((spec) => parsed[spec.field] !== null)
  const dirty = valid && !same(parsed as Values, saved)
  const atDefaults = same(saved, defaults)
  const draftIsSaved = fields.every((spec) => draft[spec.field] === String(saved[spec.field]))

  const write = (values: Values) => {
    setSaved(values)
    toast.success('搜尋排序已儲存。')
  }

  return (
    <SettingRow
      title="搜尋排序"
      description={
        <span className="flex flex-col">
          <span>在「歷史」中依意思搜尋時，結果如何排序。它會合併兩份清單：文字相符的網頁和意思相近的網頁，再把加星的網頁稍微往前排。</span>
          {!ai.enabled && <span>只在開啟本機語意搜尋時使用。現在也可以先設定。</span>}
        </span>
      }
    >
      <form
        className="flex flex-col gap-3 border-t pt-3"
        onSubmit={(event) => {
          event.preventDefault()
          if (dirty) write(parsed as Values)
        }}
      >
        {fields.map((spec) => {
          const id = `settings-tuning-${spec.field}`
          const invalid = parsed[spec.field] === null
          return (
            <div key={spec.field} className="flex items-start gap-4">
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <label htmlFor={id} className="text-[13px] font-medium">
                  {spec.label}
                </label>
                <span className="text-xs text-muted-foreground">{spec.help}</span>
                <span id={`${id}-hint`} className={invalid ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}>
                  {invalid ? `請輸入 ${spec.min} 到 ${spec.max} 之間的數字。` : `預設 ${spec.fallback} · 範圍 ${spec.min} 到 ${spec.max}`}
                </span>
              </div>
              <Input
                id={id}
                type="number"
                inputMode="decimal"
                min={spec.min}
                max={spec.max}
                step={spec.step}
                value={draft[spec.field]}
                aria-invalid={invalid}
                aria-describedby={`${id}-hint`}
                className="h-8 w-24 shrink-0 text-right tabular-nums"
                onChange={(event) => setDraft((current) => ({ ...current, [spec.field]: event.target.value }))}
              />
            </div>
          )
        })}
        <div className="flex justify-end gap-2">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={atDefaults && draftIsSaved}
            onClick={() => {
              setDraft(asText(defaults))
              if (!atDefaults) write(defaults)
            }}
          >
            恢復預設值
          </Button>
          <Button type="submit" size="sm" disabled={!dirty}>
            儲存
          </Button>
        </div>
      </form>
    </SettingRow>
  )
}
