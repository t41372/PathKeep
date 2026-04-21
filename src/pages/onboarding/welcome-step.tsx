/**
 * @file welcome-step.tsx
 * @description Renders the onboarding welcome hero and initial CTA.
 * @module pages/onboarding
 *
 * ## 職責
 * - 顯示 PathKeep welcome hero、version line、feature summary 與 trust bullets。
 * - 把「開始設定」行為交回 route owner。
 *
 * ## 不負責
 * - 不管理 onboarding step state。
 * - 不讀寫 config。
 * - 不執行 navigation。
 *
 * ## 依賴關係
 * - 依賴 shared `BrandMark` 和 onboarding i18n copy。
 */

import { BrandMark } from '../../components/brand-mark'
import type { AppBuildInfo } from '../../lib/types'
import { useI18n } from '../../lib/i18n'

export interface WelcomeStepProps {
  buildInfo: AppBuildInfo | null
  buildRevision: string | null
  buildTitle: string | null
  onBegin: () => void
}

export function WelcomeStep({
  buildInfo,
  buildRevision,
  buildTitle,
  onBegin,
}: WelcomeStepProps) {
  const { t } = useI18n('onboarding')

  return (
    <div className="welcome-hero">
      <div className="welcome-logo">
        <BrandMark alt="" />
      </div>
      <h1 className="welcome-title">PATHKEEP</h1>
      <p className="welcome-version mono" title={buildTitle ?? undefined}>
        {t('versionLine', {
          version: buildRevision
            ? `${buildInfo?.version ?? 'preview'} · ${buildRevision}`
            : (buildInfo?.version ?? 'preview'),
        })}
      </p>
      <p className="welcome-tagline">
        {t('welcomeTagline1')}
        <br />
        {t('welcomeTagline2')}
      </p>

      <div className="welcome-features">
        <div className="welcome-feature">
          <div className="feature-icon">↓</div>
          <div className="feature-text">
            <div className="feature-title">{t('featureBackupTitle')}</div>
            <div className="feature-desc">{t('featureBackupDesc')}</div>
          </div>
        </div>
        <div className="welcome-feature">
          <div className="feature-icon">◎</div>
          <div className="feature-text">
            <div className="feature-title">{t('featureSearchTitle')}</div>
            <div className="feature-desc">{t('featureSearchDesc')}</div>
          </div>
        </div>
        <div className="welcome-feature">
          <div className="feature-icon">◈</div>
          <div className="feature-text">
            <div className="feature-title">{t('featureInsightsTitle')}</div>
            <div className="feature-desc">{t('featureInsightsDesc')}</div>
          </div>
        </div>
      </div>

      <div className="welcome-trust">
        <div className="trust-item">
          <span className="trust-icon">⊘</span>
          <span>{t('trustLocalFirst')}</span>
        </div>
        <div className="trust-item">
          <span className="trust-icon">⊞</span>
          <span>{t('trustOpenSource')}</span>
        </div>
        <div className="trust-item">
          <span className="trust-icon">⚙</span>
          <span>{t('trustBuiltWith')}</span>
        </div>
      </div>

      <button className="btn-primary btn-lg" type="button" onClick={onBegin}>
        {t('beginSetup')}
      </button>
    </div>
  )
}
