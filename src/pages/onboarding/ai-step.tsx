/**
 * @file ai-step.tsx
 * @description Renders the optional AI onboarding step as a REAL, explicit opt-in: enabling turns on
 *              on-device local semantic search (a small model download + index build run in the
 *              background); skipping keeps AI off. Skip stays the easy, default-friendly path.
 * @module pages/onboarding
 *
 * ## 職責
 * - 誠實說明「啟用」代表什麼：在本機對歷史做語意搜尋，會在背景下載小模型 + 建立索引，沒有資料離開裝置。
 * - 說明 AI 助手（需外部 LLM）不在這裡啟用，之後可在設定中配置。
 * - 提供兩個明確動作（啟用 / 暫時不要），兩者都交回 route owner；不預選、不強迫、不 nag。
 *
 * ## 不負責
 * - 不寫入 config、不啟用 AI，也不直接發起下載或建索引（route owner 在 Finish 後才觸發背景 setup）。
 * - 不在 onboarding 內塞入完整的 provider editor（刻意保持輕量）。
 * - 不管理 onboarding step state。
 *
 * ## 依賴關係
 * - 依賴 PaperCard primitives 與 onboarding i18n copy。
 *
 * ## 性能備注
 * - 純展示，固定成本；無資料查詢或重計算。
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { useI18n } from '../../lib/i18n'

export interface AiStepProps {
  /**
   * Opt IN to local semantic search. The route owner records the choice and, after the archive is
   * initialized + the first backup runs, fires the background model-download + index-build setup.
   */
  onEnable: () => void
  /** Advance past this optional step WITHOUT enabling AI (the easy default path; AI stays off). */
  onSkip: () => void
  onBack: () => void
}

/** One explanatory bullet in the AI opt-in card. */
function AiTrustBullet({ title, body }: { title: string; body: string }) {
  return (
    <div className="flex gap-3">
      {/* The crosshair "+" mark matches the onboarding header's idiom and reads neutrally. */}
      <span aria-hidden="true" className="text-accent mt-[1px] font-mono">
        +
      </span>
      <div className="flex flex-col gap-[2px]">
        <span className="font-serif text-[14px] text-ink">{title}</span>
        <span className="text-ink-muted text-[13px] leading-[1.5]">{body}</span>
      </div>
    </div>
  )
}

export function AiStep({ onEnable, onSkip, onBack }: AiStepProps) {
  const { t } = useI18n('onboarding')

  return (
    <div className="ob-panel-container" data-testid="onboarding-ai-step">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('aiStepTitle')}</h2>
        <p className="ob-desc">{t('aiStepDesc')}</p>
      </div>

      <div className="mt-4">
        <PaperCard testId="onboarding-ai-summary">
          <PaperCardHeader
            title={t('aiStepTitle')}
            right={<PaperCardBadge>{t('stepAi')}</PaperCardBadge>}
          />
          <PaperCardBody className="flex flex-col gap-4">
            <AiTrustBullet
              title={t('aiStepEnableTitle')}
              body={t('aiStepEnableBody')}
            />
            <AiTrustBullet
              title={t('aiStepAssistantTitle')}
              body={t('aiStepAssistantBody')}
            />
            <p className="mono-support" data-testid="onboarding-ai-skip-hint">
              {t('aiStepSkipHint')}
            </p>
          </PaperCardBody>
        </PaperCard>
      </div>

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <div className="flex items-center gap-3">
          <button
            className="btn-secondary"
            type="button"
            data-testid="onboarding-ai-skip"
            onClick={onSkip}
          >
            {t('aiStepSkipAction')}
          </button>
          <button
            className="btn-primary"
            type="button"
            data-testid="onboarding-ai-enable"
            onClick={onEnable}
          >
            {t('aiStepEnableAction')}
          </button>
        </div>
      </div>
    </div>
  )
}
