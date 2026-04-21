/**
 * @file updater-section.tsx
 * @description Renders the Settings updater review panel without owning any update lifecycle state itself.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示桌面更新的 boundary、版本資訊、下載進度與 release notes。
 * - 把 check / install / restart / open release page 這些按鈕接回 route-owned handlers。
 * - 保持 updater surface 和 Settings sticky nav 使用同一組 anchor / icon contract。
 *
 * ## 不負責
 * - 不直接檢查更新，也不管理下載狀態。
 * - 不追蹤 analytics event。
 * - 不決定 release source 或 updater backend contract。
 *
 * ## 依賴關係
 * - 依賴 `use-settings-route-state.ts` 提供的 updater state 與 handlers。
 * - 依賴 `useI18n()` 和 `formatBytes()` 呈現 localized progress copy。
 *
 * ## 性能備注
 * - 本模組只渲染 route 已經準備好的 state，不自行發起 background work。
 */

import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type {
  AppBuildInfo,
  UpdateAvailability,
  UpdateInstallState,
} from '../../lib/types'
import type { PendingAppUpdate } from '../../lib/update'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned updater state consumed by the extracted section.
 *
 * The section stays render-only, so every field here already reflects the
 * canonical updater truth held by the Settings route hook.
 */
export interface UpdaterSectionState {
  buildInfo: AppBuildInfo | null
  pendingUpdate: PendingAppUpdate | null
  updateAvailability: UpdateAvailability | null
  updateInstallState: UpdateInstallState
  onCheckForUpdates: () => Promise<void>
  onDownloadAndInstallUpdate: () => Promise<void>
  onOpenReleasePage: () => Promise<void>
  onRelaunchForUpdate: () => Promise<void>
}

/**
 * Groups the stable section anchor descriptor with the updater review state.
 */
export interface UpdaterSectionProps {
  navItem: SettingsSectionNavItem
  state: UpdaterSectionState
}

/**
 * Renders the updater review surface from route-owned state.
 *
 * This keeps long updater JSX out of the route shell while preserving the same
 * action semantics and boundary callouts that Settings already exposed.
 */
export function UpdaterSection({ navItem, state }: UpdaterSectionProps) {
  const { language, t } = useI18n()
  const {
    buildInfo,
    pendingUpdate,
    updateAvailability,
    updateInstallState,
    onCheckForUpdates,
    onDownloadAndInstallUpdate,
    onOpenReleasePage,
    onRelaunchForUpdate,
  } = state

  return (
    <div className="panel panel--critical" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-action mono">
          {buildInfo?.version ?? t('common.notAvailable')}
        </span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone={
            updateInstallState.phase === 'error'
              ? 'danger'
              : updateInstallState.phase === 'available' ||
                  updateInstallState.phase === 'installed'
                ? 'warning'
                : 'info'
          }
          title={t('settings.updateBoundaryTitle')}
          body={updateInstallState.message ?? t('settings.updateBoundaryBody')}
        />

        <div className="settings-field-grid">
          <div className="config-row">
            <span className="config-label">
              {t('settings.updateCurrentVersion')}
            </span>
            <span className="config-value mono">
              {buildInfo?.version ?? t('common.notAvailable')}
            </span>
          </div>

          <div className="config-row">
            <span className="config-label">
              {t('settings.updateLatestVersion')}
            </span>
            <span className="config-value mono">
              {updateAvailability?.version ?? t('common.notAvailable')}
            </span>
          </div>

          <div className="config-row">
            <span className="config-label">
              {t('settings.updatePublishedAt')}
            </span>
            <span className="config-value mono">
              {updateAvailability?.publishedAt ?? t('common.notAvailable')}
            </span>
          </div>

          <div className="config-row">
            <span className="config-label">
              {t('settings.updateCheckedAt')}
            </span>
            <span className="config-value mono">
              {updateAvailability?.checkedAt ?? t('common.notAvailable')}
            </span>
          </div>

          {updateInstallState.contentLength ? (
            <>
              <div className="update-progress-bar">
                <div
                  className="update-progress-bar__fill"
                  style={{
                    width: `${Math.min(
                      ((updateInstallState.downloadedBytes ?? 0) /
                        updateInstallState.contentLength) *
                        100,
                      100,
                    )}%`,
                  }}
                />
              </div>
              <p className="dashboard-next-action">
                {t('settings.updateProgress', {
                  downloaded: formatBytes(
                    updateInstallState.downloadedBytes ?? 0,
                    language,
                  ),
                  total: formatBytes(
                    updateInstallState.contentLength,
                    language,
                  ),
                })}
              </p>
            </>
          ) : null}

          {updateAvailability?.notes ? (
            <div className="fieldBlock">
              <span className="config-label">
                {t('settings.updateReleaseNotes')}
              </span>
              <pre className="code-block">{updateAvailability.notes}</pre>
            </div>
          ) : null}

          <div className="settings-action-row">
            <button
              className="btn-primary"
              type="button"
              disabled={updateInstallState.phase === 'checking'}
              onClick={() => {
                void onCheckForUpdates()
              }}
            >
              {t('settings.updateCheckNow')}
            </button>
            <button
              className="btn-primary"
              type="button"
              disabled={
                !pendingUpdate ||
                updateInstallState.phase === 'downloading' ||
                updateInstallState.phase === 'installing'
              }
              onClick={() => {
                void onDownloadAndInstallUpdate()
              }}
            >
              {t('settings.updateDownloadAndInstall')}
            </button>
          </div>
          <div className="settings-action-row">
            <button
              className="btn-secondary"
              type="button"
              disabled={updateInstallState.phase !== 'installed'}
              onClick={() => {
                void onRelaunchForUpdate()
              }}
            >
              {t('settings.updateRestartNow')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => {
                void onOpenReleasePage()
              }}
            >
              {t('settings.updateOpenReleasePage')}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
