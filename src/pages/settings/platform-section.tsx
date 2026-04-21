/**
 * @file platform-section.tsx
 * @description Renders the Settings platform troubleshooting surface from route-owned support snapshots.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 scheduler / keyring / Safari access 等 platform guidance callouts。
 * - 把對應的修復入口導回 Schedule、Import、Security。
 * - 保持 platform advice 和 support-state snapshot 對齊，而不是在 UI 端猜測 host capability。
 *
 * ## 不負責
 * - 不輪詢 schedule/security support state。
 * - 不執行平台修復。
 * - 不推翻已接受的 platform guidance 文案。
 *
 * ## 依賴關係
 * - 依賴 `platform-guidance` helper 與 route hook 提供的 `supportState`。
 * - 依賴 `Link` 導向對應的修復 surface。
 *
 * ## 性能備注
 * - 只根據現有 support snapshot 派生小量 booleans，沒有額外背景工作。
 */

import { Link } from 'react-router-dom'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import {
  hasSafariAccessIssue,
  keyringNeedsReview,
  normalizePlatform,
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import type { AppSnapshot } from '../../lib/types'
import type { SupportState } from './helpers'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Groups the stable section anchor descriptor with the support snapshot needed
 * for platform troubleshooting callouts.
 */
export interface PlatformSectionProps {
  navItem: SettingsSectionNavItem
  snapshot: AppSnapshot
  supportState: SupportState
}

/**
 * Renders the platform troubleshooting review surface.
 *
 * The section keeps derived booleans local because they are presentational and
 * cheap, while the underlying support snapshot still has one canonical owner
 * in the route hook.
 */
export function PlatformSection({
  navItem,
  snapshot,
  supportState,
}: PlatformSectionProps) {
  const { t } = useI18n()
  const safariNeedsAccess = hasSafariAccessIssue(snapshot.browserProfiles)
  const platform = normalizePlatform(supportState.scheduleStatus?.platform)
  const scheduleNeedsHelp =
    supportState.scheduleStatus?.installState === 'manual-review' ||
    supportState.scheduleStatus?.installState === 'mismatch' ||
    supportState.scheduleStatus?.installState === 'permission-warning' ||
    supportState.scheduleStatus?.installState === 'legacy-install-detected'
  const keyringWarning = keyringNeedsReview(supportState.securityStatus)

  return (
    <div className="panel" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
      </div>
      <div className="panel-body settings-support-grid">
        <p className="dashboard-next-action">
          {t('settings.platformDescription')}
        </p>
        <StatusCallout
          tone={scheduleNeedsHelp ? 'warning' : 'info'}
          title={t(platformLabelKey(platform))}
          body={t(platformSummaryKey(platform))}
          actions={
            <Link className="btn-secondary" to="/schedule">
              {t('settings.reviewSchedule')}
            </Link>
          }
        />
        {safariNeedsAccess ? (
          <StatusCallout
            tone="blocked"
            title={t('platform.safariAccessTitle')}
            body={t('platform.safariAccessBody')}
            actions={
              <Link className="btn-secondary" to="/import">
                {t('settings.reviewImports')}
              </Link>
            }
          />
        ) : null}
        {keyringWarning ? (
          <StatusCallout
            tone="warning"
            title={t('platform.keyringTitle')}
            body={t('platform.keyringBody')}
            actions={
              <Link className="btn-secondary" to="/security">
                {t('settings.reviewSecurity')}
              </Link>
            }
          />
        ) : null}
        {scheduleNeedsHelp ? (
          <StatusCallout
            tone="blocked"
            title={t('platform.schedulerMismatchTitle')}
            body={t('platform.schedulerMismatchBody')}
            actions={
              <Link className="btn-secondary" to="/schedule">
                {t('settings.reviewSchedule')}
              </Link>
            }
          />
        ) : null}
      </div>
    </div>
  )
}
