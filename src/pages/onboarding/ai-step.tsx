/**
 * @file ai-step.tsx
 * @description Renders the optional AI-setup onboarding step — a calm, skip-first card that explains
 *              AI is optional, off by default, and local-first, then offers a deep-link to Settings.
 * @module pages/onboarding
 *
 * ## 職責
 * - 用平靜的語氣說明 AI 為選用、預設關閉、本地優先（PathKeep 沒有 AI 也完整可用）。
 * - 提供「在設定中配置 AI」的 deep-link 行為，以及顯眼的「暫時跳過」。
 * - 把「跳過 / 繼續到下一步」都交回 route owner，兩者都前進。
 *
 * ## 不負責
 * - 不在 onboarding 內塞入完整的 provider editor（刻意保持輕量）。
 * - 不寫入 config，不啟用 AI，不發起任何網路或持久化（AI 維持關閉）。
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
   * Record the in-flow "open AI settings after finishing setup" intent and advance to the final
   * review (M-10). It does NOT navigate away here — the route owner deep-links to
   * `/settings#settings-ai` only after the archive is initialized, so the onboarding step + the
   * confirmed master-password draft are never discarded mid-flow.
   */
  onSetUpAi: () => void
  /** Advance past this optional step without enabling AI. */
  onSkip: () => void
  onBack: () => void
}

/** One reassurance bullet in the AI-setup card. */
function AiTrustBullet({ title, body }: { title: string; body: string }) {
  return (
    <div className="flex gap-3">
      {/* The crosshair "+" mark matches the onboarding header's idiom and reads neutrally — the
          earlier "⊘" (circled slash / no-entry) leaned negative against these positive bullets. */}
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

export function AiStep({ onSetUpAi, onSkip, onBack }: AiStepProps) {
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
              title={t('aiStepOffByDefaultTitle')}
              body={t('aiStepOffByDefaultBody')}
            />
            <AiTrustBullet
              title={t('aiStepLocalFirstTitle')}
              body={t('aiStepLocalFirstBody')}
            />
            <AiTrustBullet
              title={t('aiStepCitationsTitle')}
              body={t('aiStepCitationsBody')}
            />
            <p className="mono-support" data-testid="onboarding-ai-skip-hint">
              {t('aiStepSkipHint')}
            </p>
            <div className="flex flex-wrap items-center gap-3">
              <button
                className="btn-secondary"
                type="button"
                data-testid="onboarding-ai-setup"
                onClick={onSetUpAi}
              >
                {t('aiStepSetUpAction')}
              </button>
            </div>
          </PaperCardBody>
        </PaperCard>
      </div>

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button
          className="btn-primary btn-lg"
          type="button"
          data-testid="onboarding-ai-skip"
          onClick={onSkip}
        >
          {t('aiStepSkipAction')}
        </button>
      </div>
    </div>
  )
}
