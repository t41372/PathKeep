/**
 * @file profile-selection-section.tsx
 * @description Renders the browser-profile selection panel from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 browser profile 清單、已選取狀態與 history detection summary。
 * - 把 profile toggle 交回 route-owned save handler。
 * - 保持 Settings profile review surface 的 icon / path / version honesty。
 *
 * ## 不負責
 * - 不重新掃描瀏覽器。
 * - 不直接修改 archive config。
 * - 不建立新的 profile scope grammar。
 *
 * ## 依賴關係
 * - 依賴 route hook 提供 profiles、selected ids 與 toggle handler。
 * - 依賴 shared `BrowserIcon` 呈現 browser-specific glyph。
 *
 * ## 性能備注
 * - profile list 規模小且固定；本模組只渲染現有 snapshot，不做額外查詢。
 */

import { BrowserIcon } from '../../lib/browser-icons'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { BrowserProfile } from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned profile selection state consumed by the extracted section.
 */
export interface ProfileSelectionSectionState {
  profiles: BrowserProfile[]
  saving: boolean
  selectedIds: Set<string>
  onToggleProfile: (profileId: string) => Promise<void>
}

/**
 * Groups the stable section anchor descriptor with the profile-selection state.
 */
export interface ProfileSelectionSectionProps {
  navItem: SettingsSectionNavItem
  state: ProfileSelectionSectionState
}

/**
 * Renders the browser-profile selection review surface.
 *
 * This keeps the long profile list JSX out of the Settings route shell while
 * leaving save behavior under the hook that already owns config mutations.
 */
export function ProfileSelectionSection({
  navItem,
  state,
}: ProfileSelectionSectionProps) {
  const { t } = useI18n()
  const { profiles, saving, selectedIds, onToggleProfile } = state

  return (
    <div className="panel" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
      </div>
      <div className="panel-body">
        <p className="dashboard-next-action">
          {t('settings.browserProfilesBody')}
        </p>
        <div className="profile-list">
          {profiles.map((profile) => {
            const checked = selectedIds.has(profile.profileId)
            return (
              <button
                className={`profile-item ${checked ? 'checked' : ''}`}
                disabled={saving}
                key={profile.profileId}
                type="button"
                onClick={() => {
                  void onToggleProfile(profile.profileId)
                }}
              >
                <div className="profile-check">
                  <div className={`checkbox ${checked ? 'active' : ''}`}>
                    {checked ? <Glyph icon="check" filled /> : ''}
                  </div>
                </div>
                <div className="profile-icon">
                  <BrowserIcon browserName={profile.browserName} />
                </div>
                <div className="profile-info">
                  <div className="profile-name">
                    {profile.browserName} / {profile.profileName}
                  </div>
                  <div className="profile-path dim mono">
                    {profile.profilePath}
                  </div>
                </div>
                <div className="profile-stats mono dim">
                  {profile.historyExists
                    ? `${t('settings.historyFound')} · ${profile.browserVersion ?? t('common.notAvailable')}`
                    : t('settings.noHistoryDetected')}
                </div>
              </button>
            )
          })}
        </div>
      </div>
    </div>
  )
}
