import { Outlet, useNavigate } from 'react-router-dom'
import { BusyOverlay } from '../components/primitives/busy-overlay'
import { BrandMark } from '../components/brand-mark'
import { useShellData } from './shell-data-context'
import { useI18n } from '../lib/i18n'

export function OnboardingShell() {
  const navigate = useNavigate()
  const { busyAction, busyOverlay } = useShellData()
  const { t } = useI18n('shell')

  return (
    <div className="app-frame onboarding-frame" data-testid="onboarding-shell">
      <div aria-hidden className="shell-dot-grid" />
      <span aria-hidden className="corner-mark corner-mark--tl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--tr">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--bl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--br">
        +
      </span>
      <header className="onboarding-header">
        <div className="onboarding-brand">
          <div className="logo-lockup">
            <div aria-hidden className="logo-mark">
              <BrandMark alt="" />
            </div>
            <div className="logo-text">
              <span className="logo-name">PATHKEEP</span>
              <span className="logo-version">{t('onboardingVersion')}</span>
            </div>
          </div>
          <p>{t('onboardingLeaveHint')}</p>
        </div>
        <div className="onboarding-actions">
          <button
            className="btn-secondary"
            type="button"
            onClick={() => void navigate('/')}
          >
            {t('exitSetup')}
          </button>
        </div>
      </header>
      <div
        className="onboarding-content"
        style={{ position: 'relative', zIndex: 2 }}
      >
        <Outlet />
      </div>
      {busyAction ? (
        <BusyOverlay
          label={busyOverlay?.label ?? busyAction}
          detail={busyOverlay?.detail}
          progressLabel={busyOverlay?.progressLabel}
          progressValue={busyOverlay?.progressValue}
          steps={busyOverlay?.steps}
          activeStep={busyOverlay?.activeStep}
        />
      ) : null}
    </div>
  )
}
