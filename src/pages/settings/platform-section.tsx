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
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
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

const LINK_BUTTON_CLASS =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors'

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
 * Paper aesthetic: PaperCard wrapper with intro line + a stack of
 * StatusCallout entries (preserved as-is — Callout's tone tokens are
 * already paper-aware).
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
    <PaperCard testId={navItem.id}>
      <PaperCardHeader title={navItem.label} />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.platformDescription')}
        </p>
        <div className="flex flex-col gap-3">
          <StatusCallout
            tone={scheduleNeedsHelp ? 'warning' : 'info'}
            title={t(platformLabelKey(platform))}
            body={t(platformSummaryKey(platform))}
            actions={
              <Link className={LINK_BUTTON_CLASS} to="/schedule">
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
                <Link className={LINK_BUTTON_CLASS} to="/import">
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
                <Link className={LINK_BUTTON_CLASS} to="/security">
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
                <Link className={LINK_BUTTON_CLASS} to="/schedule">
                  {t('settings.reviewSchedule')}
                </Link>
              }
            />
          ) : null}
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}
