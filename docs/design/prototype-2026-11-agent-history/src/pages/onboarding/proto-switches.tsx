/**
 * Prototype-only switchers for onboarding, behind one dashed button so the
 * step list keeps its room: the states the real steps switch on that a
 * reviewer cannot reach by clicking (no keychain, a schedule that cannot be
 * installed, a stage of the final run failing, ...). Not part of the design.
 */
import type { ReactNode } from 'react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Switch } from '@/components/ui/switch'
import type { ModelListCase } from '../settings/ai/model-field'
import { RowSelect } from '../settings/ai/row-select'
import type { ResultCase } from './done-step'
import type { PreviewCase } from './schedule-step'
import type { Stage } from './use-finish'

export interface OnboardingCases {
  keychainAvailable: boolean
  mac: boolean
  preview: PreviewCase
  failAt: Stage | null
  result: ResultCase
  listCase: ModelListCase
  /** Whether this computer has any agent tool's folder. */
  agentTools: boolean
}

export const defaultCases: OnboardingCases = { keychainAvailable: true, mac: true, preview: 'ready', failAt: null, result: 'all', listCase: 'found', agentTools: true }

export function OnboardingSwitches({ cases, onChange, locked }: { cases: OnboardingCases; onChange: (patch: Partial<OnboardingCases>) => void; locked: boolean }) {
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button type="button" className="mb-1 rounded-lg border border-dashed px-2.5 py-2 text-left text-xs text-muted-foreground hover:bg-muted">
          原型：切換狀態…
        </button>
      </PopoverTrigger>
      <PopoverContent side="right" align="end" className="flex w-[340px] flex-col gap-3 text-xs text-muted-foreground">
        <Row label="原型：這台電腦有 Agent 工具">
          <Switch checked={cases.agentTools} onCheckedChange={(agentTools) => onChange({ agentTools })} aria-label="原型：這台電腦有 Agent 工具" />
        </Row>
        <Row label="原型：系統鑰匙圈可用">
          <Switch checked={cases.keychainAvailable} onCheckedChange={(keychainAvailable) => onChange({ keychainAvailable })} aria-label="原型：系統鑰匙圈可用" />
        </Row>
        <Row label="原型：在 Mac 上（關掉是 Windows）">
          <Switch checked={cases.mac} onCheckedChange={(mac) => onChange({ mac })} aria-label="原型：在 Mac 上" />
        </Row>
        <Row label="原型：排程預覽">
          <RowSelect
            label="原型：排程預覽"
            value={cases.preview}
            onChange={(v) => onChange({ preview: v as PreviewCase })}
            options={[
              { value: 'ready', label: '可以安裝' },
              { value: 'unsupported', label: '無法自動安裝' },
              { value: 'failed', label: '準備失敗' },
            ]}
          />
        </Row>
        <Row label="原型：服務上的模型清單">
          <RowSelect
            label="原型：服務上的模型清單"
            value={cases.listCase}
            onChange={(v) => onChange({ listCase: v as ModelListCase })}
            options={[
              { value: 'found', label: '有合用的' },
              { value: 'otherKind', label: '只有另一種' },
              { value: 'none', label: '一個都沒有' },
              { value: 'failed', label: '讀不到' },
            ]}
          />
        </Row>
        <Row label="原型：完成時哪一步失敗一次">
          <RowSelect
            label="原型：完成時哪一步失敗一次"
            value={cases.failAt ?? 'none'}
            disabled={locked}
            onChange={(v) => onChange({ failAt: v === 'none' ? null : (v as Stage) })}
            options={[
              { value: 'none', label: '都不失敗' },
              { value: 'archive', label: '建立存檔' },
              { value: 'keychain', label: '存到鑰匙圈' },
              { value: 'schedule', label: '安裝排程' },
              { value: 'backup', label: '第一次備份' },
              { value: 'ai', label: '設定 AI' },
            ]}
          />
        </Row>
        <Row label="原型：第一次備份的結果">
          <RowSelect
            label="原型：第一次備份的結果"
            value={cases.result}
            onChange={(v) => onChange({ result: v as ResultCase })}
            options={[
              { value: 'all', label: '全部存好' },
              { value: 'safari', label: 'Safari 沒有權限' },
              { value: 'partial', label: '部分瀏覽器失敗' },
            ]}
          />
        </Row>
      </PopoverContent>
    </Popover>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span>{label}</span>
      {children}
    </div>
  )
}
