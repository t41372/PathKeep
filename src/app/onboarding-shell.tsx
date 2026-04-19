/**
 * This module renders the onboarding-specific shell that deliberately omits the normal sidebar chrome.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `OnboardingShell`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { Outlet, useNavigate } from 'react-router-dom'
import { BusyOverlay } from '../components/primitives/busy-overlay'
import { BrandMark } from '../components/brand-mark'
import { useShellData } from './shell-data-context'
import { useI18n } from '../lib/i18n'

/**
 * Renders the onboarding shell wrapper.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
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
          logLines={busyOverlay?.logLines}
        />
      ) : null}
    </div>
  )
}
