/**
 * Prototype-only switchers for Settings › AI: jump to each state the real
 * rows switch on (semantic search, the services, what a test or a check
 * answers, the MCP details and requests). Not part of the design.
 */
import type { ReactNode } from 'react'
import { RowSelect } from './row-select'
import { IN_APP, useAi, type AiState, type CheckCase, type McpCase, type SemanticPhase, type TestCase } from './state'
import type { ModelListCase } from './model-field'

const phases: { value: SemanticPhase; label: string }[] = [
  { value: 'off', label: '關閉' },
  { value: 'downloading', label: '下載模型中' },
  { value: 'downloadFailed', label: '下載失敗' },
  { value: 'modelMissing', label: '模型還沒下載' },
  { value: 'setupRunning', label: '開啟中，等其他工作' },
  { value: 'needsBuild', label: '還沒有索引' },
  { value: 'unfinished', label: '索引還沒完成' },
  { value: 'unfinishedOlderText', label: '還沒完成，用舊版文字' },
  { value: 'running', label: '建立索引中' },
  { value: 'queued', label: '排隊等其他工作' },
  { value: 'pausedUser', label: '你暫停了' },
  { value: 'pausedService', label: '服務出錯而暫停' },
  { value: 'pausedQueue', label: '所有背景工作暫停' },
  { value: 'ready', label: '完成' },
  { value: 'readyEmpty', label: '完成，但沒有造訪' },
  { value: 'olderText', label: '完成，用舊版文字建立' },
  { value: 'failed', label: '索引失敗' },
  { value: 'degraded', label: '索引不完整' },
]

const PARTIAL = 214_530

function phasePatch(phase: SemanticPhase): Partial<AiState> {
  const download = phase === 'downloading' || phase === 'downloadFailed' || phase === 'modelMissing'
  const complete = phase === 'off' || phase === 'ready' || phase === 'readyEmpty' || phase === 'olderText' || phase === 'failed' || phase === 'degraded'
  const partial = phase === 'unfinished' || phase === 'unfinishedOlderText' || phase === 'running' || phase.startsWith('paused')
  return {
    phase,
    downloaded: 0,
    ...(download ? { modelId: IN_APP, modelDownloaded: false } : { modelDownloaded: true }),
    // Only an embeddings service can refuse to run its model.
    ...(phase === 'pausedService' ? { modelId: 'embed-lm-studio' } : {}),
    chosenComplete: complete,
    servingOther: null,
    done: partial ? PARTIAL : 0,
  }
}

export function AiProtoSwitches() {
  const ai = useAi()
  const assistant = ai.llmSelected === null ? 'none' : ai.llmSelected === 'llm-ollama' ? 'local' : ai.llmSelected === 'llm-openai' ? 'online' : ai.llmSelected === 'llm-anthropic' ? 'noKey' : 'other'
  const embedding = ai.modelId === IN_APP ? 'inApp' : ai.modelId === 'embed-lm-studio' ? 'local' : ai.modelId === 'embed-openai' ? 'online' : 'other'
  return (
    <div className="mb-1 grid grid-cols-2 gap-x-5 gap-y-2 rounded-xl border border-dashed px-4 py-3 text-xs text-muted-foreground max-[900px]:grid-cols-1">
      <Pick label="原型：語意搜尋">
        <RowSelect label="原型：語意搜尋" value={ai.phase} onChange={(v) => ai.set(phasePatch(v as SemanticPhase))} options={phases} />
      </Pick>
      <Pick label="原型：語意搜尋用的模型">
        <RowSelect
          label="原型：語意搜尋用的模型"
          value={embedding}
          onChange={(v) => ai.set({ modelId: v === 'local' ? 'embed-lm-studio' : v === 'online' ? 'embed-openai' : IN_APP, chosenComplete: true, servingOther: null, phase: ai.phase === 'off' ? 'off' : 'ready' })}
          options={[
            { value: 'inApp', label: 'App 內建' },
            { value: 'local', label: '本機的嵌入服務' },
            { value: 'online', label: '線上的嵌入服務' },
            ...(embedding === 'other' ? [{ value: 'other', label: '你新增的服務' }] : []),
          ]}
        />
      </Pick>
      <Pick label="原型：助理的 AI 服務">
        <RowSelect
          label="原型：助理的 AI 服務"
          value={assistant}
          onChange={(v) => ai.set({ llmSelected: v === 'none' ? null : v === 'local' ? 'llm-ollama' : v === 'online' ? 'llm-openai' : 'llm-anthropic' })}
          options={[
            { value: 'none', label: '沒有' },
            { value: 'local', label: '本機的服務' },
            { value: 'online', label: '線上的服務' },
            { value: 'noKey', label: '線上，還沒存金鑰' },
            ...(assistant === 'other' ? [{ value: 'other', label: '你新增的服務' }] : []),
          ]}
        />
      </Pick>
      <Pick label="原型：測試連線的結果">
        <RowSelect
          label="原型：測試連線的結果"
          value={ai.testCase}
          onChange={(v) => ai.set({ testCase: v as TestCase })}
          options={[
            { value: 'ok', label: '成功' },
            { value: 'secret-missing', label: '金鑰不對' },
            { value: 'network-error', label: '連不上' },
            { value: 'rate-limited', label: '限制請求' },
            { value: 'bad-model', label: '沒有這個模型' },
            { value: 'bad-endpoint', label: '位址不是 API' },
            { value: 'unsupported-capability', label: '不能用在這裡' },
            { value: 'provider-disabled', label: '服務已關閉' },
            { value: 'other', label: '其他錯誤' },
            { value: 'thrown', label: '請求本身失敗' },
          ]}
        />
      </Pick>
      <Pick label="原型：儲存服務後的檢查">
        <RowSelect
          label="原型：儲存服務後的檢查"
          value={ai.checkCase}
          onChange={(v) => ai.set({ checkCase: v as CheckCase })}
          options={[
            { value: 'found', label: '有這個模型' },
            { value: 'missing', label: '沒有這個模型' },
            { value: 'unreachable', label: '讀不到模型' },
          ]}
        />
      </Pick>
      <Pick label="原型：服務上的模型清單">
        <RowSelect
          label="原型：服務上的模型清單"
          value={ai.listCase}
          onChange={(v) => ai.set({ listCase: v as ModelListCase })}
          options={[
            { value: 'found', label: '有合用的' },
            { value: 'otherKind', label: '只有另一種' },
            { value: 'none', label: '一個都沒有' },
            { value: 'failed', label: '讀不到' },
          ]}
        />
      </Pick>
      <Pick label="原型：MCP 連線資訊與請求">
        <RowSelect
          label="原型：MCP 連線資訊與請求"
          value={ai.mcpCase}
          onChange={(v) => ai.set({ mcpCase: v as McpCase })}
          options={[
            { value: 'requests', label: '有請求' },
            { value: 'empty', label: '還沒有請求' },
            { value: 'loading', label: '載入中' },
            { value: 'failed', label: '載入失敗' },
          ]}
        />
      </Pick>
    </div>
  )
}

function Pick({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span>{label}</span>
      {children}
    </div>
  )
}
