/**
 * AI: today's step, unchanged (src/features/onboarding/ai-step.tsx). Not now,
 * local semantic search, or an AI service with the same form as Settings › AI.
 */
import { CircleOff, Cpu, Plug } from 'lucide-react'
import type { ModelListCase } from '../settings/ai/model-field'
import { ProviderForm } from '../settings/ai/provider-form'
import { appBytes, isLocalUrl, STATIC_MODEL_APPROX_BYTES } from '../settings/ai/providers'
import { ChoiceCard, ChoiceGroup } from './choice'
import type { AiChoice, Draft } from './draft'
import { StepHeading } from './steps'

const icons = { off: CircleOff, local: Cpu, provider: Plug }

export const aiTitles: Record<AiChoice, string> = { off: '先不用', local: '本機語意搜尋', provider: '連接 AI 服務' }

export function AiStep({ draft, where, onChange, listCase }: { draft: Draft; where: string; onChange: (patch: Partial<Draft>) => void; listCase: ModelListCase }) {
  const bodies: Record<AiChoice, string> = {
    off: '只用全文和正則搜尋，什麼都不下載。',
    local: `下載一次靜態嵌入模型，然後${where}建立索引。非常快，但找意思相近的頁面不如一般的上下文嵌入模型。搜尋全程離線。`,
    provider: 'Ollama、OpenAI、Anthropic 等。只有你提問時才會傳送相關片段。',
  }
  return (
    <>
      <StepHeading title="要開啟 AI 功能嗎？" body="預設關閉。開啟後可以用自然語言搜尋，也可以問助理。" />
      <ChoiceGroup value={draft.ai} onValueChange={(ai: AiChoice) => onChange({ ai })} label="AI">
        {(['off', 'local', 'provider'] as const).map((choice) => {
          const Icon = icons[choice]
          return (
            <ChoiceCard
              key={choice}
              value={choice}
              leading={<Icon className="mt-px size-[18px] shrink-0" strokeWidth={1.75} />}
              title={aiTitles[choice]}
              body={bodies[choice]}
              trailing={choice === 'local' && <span className="font-mono text-xs whitespace-nowrap text-muted-foreground">≈ {appBytes(STATIC_MODEL_APPROX_BYTES)}</span>}
            />
          )
        })}
      </ChoiceGroup>

      {draft.ai === 'provider' && (
        <section className="flex animate-rise flex-col gap-3 rounded-xl border bg-card p-4 shadow-card">
          <ProviderForm draft={draft.provider} onChange={(provider) => onChange({ provider })} idPrefix="onboarding-provider" listCase={listCase} />
          <p className="text-xs text-muted-foreground">{isLocalUrl(draft.provider.baseUrl) ? '在這台電腦上執行，不會向任何地方傳送內容。' : '這是線上服務。你提問時，助理找到的頁面會傳送給它。'}</p>
        </section>
      )}

      <p className="text-[13px] text-muted-foreground">不用 AI，PathKeep 照樣完整可用。之後可以在「設定 → AI」更改。</p>
    </>
  )
}
