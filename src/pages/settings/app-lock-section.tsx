/**
 * @file app-lock-section.tsx
 * @description Renders the App Lock control panel from route-owned draft state and handlers.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 App Lock 的 enable/disable、idle timeout、biometric、passcode 與 recovery review surface。
 * - 把 save config、set/clear passcode、lock now 交回 route-owned handlers。
 * - 保持 config path / last unlocked / degradation notes 的誠實呈現。
 *
 * ## 不負責
 * - 不直接讀寫 App Lock config 或 passcode。
 * - 不決定 biometric capability 或 session lock contract。
 * - 不管理 shell-level `/lock` navigation。
 *
 * ## 依賴關係
 * - 依賴 `use-settings-route-state.ts` 提供 draft state、copy/open path actions 與 mutations。
 * - 依賴 `ReviewPathActionRow` 保持 Settings review surface 的一致 copy/open grammar。
 *
 * ## 性能備注
 * - 本模組只讀取 route hook 已經準備好的 App Lock state，不自行觸發背景查詢。
 */

import {
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import type { AppLockConfig, AppLockStatus } from '../../lib/types'
import { useI18n } from '../../lib/i18n'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned App Lock state consumed by the extracted section.
 */
export interface AppLockSectionState {
  action: string | null
  canEnable: boolean
  configDirty: boolean
  copyFeedback: ReviewCopyFeedback | null
  currentSettings: AppLockConfig | null
  passcode: string
  recoveryHint: string
  status: AppLockStatus | null
  usesTouchId: boolean
  onBiometricChange: (enabled: boolean) => void
  onClearPasscode: () => Promise<void>
  onCopyPath: (key: string, value: string) => Promise<void>
  onEnabledChange: (enabled: boolean) => void
  onIdleTimeoutChange: (minutes: number) => void
  onLockNow: () => Promise<void>
  onOpenPath: (path: string) => void
  onPasscodeChange: (value: string) => void
  onRecoveryHintChange: (value: string) => void
  onSaveConfig: () => Promise<void>
  onSetPasscode: () => Promise<void>
}

/**
 * Groups the stable section anchor descriptor with the App Lock view-model.
 */
export interface AppLockSectionProps {
  navItem: SettingsSectionNavItem
  state: AppLockSectionState
}

/**
 * Renders the App Lock control panel while leaving every mutation under route
 * ownership.
 *
 * The component exits early when the route has not hydrated a draft yet so it
 * never invents fallback security settings on its own.
 */
export function AppLockSection({ navItem, state }: AppLockSectionProps) {
  const { t } = useI18n()
  const {
    action,
    canEnable,
    configDirty,
    copyFeedback,
    currentSettings,
    passcode,
    recoveryHint,
    status,
    usesTouchId,
    onBiometricChange,
    onClearPasscode,
    onCopyPath,
    onEnabledChange,
    onIdleTimeoutChange,
    onLockNow,
    onOpenPath,
    onPasscodeChange,
    onRecoveryHintChange,
    onSaveConfig,
    onSetPasscode,
  } = state

  if (!currentSettings) {
    return null
  }

  return (
    <div className="panel panel--security" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-badge">{t('settings.optional')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone={currentSettings.enabled ? 'warning' : 'info'}
          title={t('settings.appLockBoundaryTitle')}
          body={t('settings.appLockBoundaryBody')}
        />

        <div className="settings-field-grid">
          <label className="checkbox-row">
            <input
              aria-label={t('settings.appLockEnabled')}
              checked={currentSettings.enabled}
              type="checkbox"
              onChange={(event) => {
                onEnabledChange(event.target.checked)
              }}
            />
            <span>{t('settings.appLockEnabled')}</span>
          </label>

          <div className="config-row">
            <span className="config-label">{t('settings.appLockStatus')}</span>
            <span className="config-value mono">
              {status?.locked
                ? t('settings.appLockStatusLocked')
                : t('settings.appLockStatusUnlocked')}
            </span>
          </div>

          <div className="config-row">
            <span className="config-label">
              {t('settings.appLockIdleTimeout')}
            </span>
            <select
              aria-label={t('settings.appLockIdleTimeout')}
              className="settings-select"
              value={currentSettings.idleTimeoutMinutes}
              onChange={(event) => {
                onIdleTimeoutChange(Number(event.target.value))
              }}
            >
              {[1, 5, 10, 15, 30, 60].map((minutes) => (
                <option key={minutes} value={minutes}>
                  {t('settings.appLockMinutes', { count: minutes })}
                </option>
              ))}
            </select>
          </div>

          <label className="checkbox-row">
            <input
              aria-label={
                usesTouchId
                  ? t('settings.appLockTouchId')
                  : t('settings.appLockBiometric')
              }
              checked={currentSettings.biometricEnabled}
              disabled={!status?.biometricAvailable}
              type="checkbox"
              onChange={(event) => {
                onBiometricChange(event.target.checked)
              }}
            />
            <span>
              {usesTouchId
                ? t('settings.appLockTouchId')
                : t('settings.appLockBiometric')}
            </span>
          </label>

          {!status?.biometricAvailable ? (
            <p className="dashboard-next-action">
              {usesTouchId
                ? t('settings.appLockTouchIdUnavailable')
                : t('settings.appLockBiometricUnavailable')}
            </p>
          ) : null}

          <label className="fieldBlock">
            <span className="config-label">
              {t('settings.appLockRecoveryHint')}
            </span>
            <input
              aria-label={t('settings.appLockRecoveryHint')}
              className="settings-input"
              placeholder={t('settings.appLockRecoveryHintPlaceholder')}
              type="text"
              value={recoveryHint}
              onChange={(event) => {
                onRecoveryHintChange(event.target.value)
              }}
            />
          </label>

          <label className="fieldBlock">
            <span className="config-label">
              {t('settings.appLockPasscode')}
            </span>
            <input
              aria-label={t('settings.appLockPasscode')}
              className="settings-input"
              placeholder={t('settings.appLockPasscodePlaceholder')}
              type="password"
              value={passcode}
              onChange={(event) => {
                onPasscodeChange(event.target.value)
              }}
            />
          </label>

          <div className="settings-action-row">
            <button
              className="btn-primary"
              type="button"
              disabled={
                Boolean(action) ||
                !configDirty ||
                (currentSettings.enabled && !canEnable)
              }
              onClick={() => {
                void onSaveConfig()
              }}
            >
              {action ?? t('settings.appLockSave')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(action) || passcode.trim().length < 4}
              onClick={() => {
                void onSetPasscode()
              }}
            >
              {status?.passcodeConfigured
                ? t('settings.appLockUpdatePasscode')
                : t('settings.appLockSetPasscode')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(action) || !status?.passcodeConfigured}
              onClick={() => {
                void onClearPasscode()
              }}
            >
              {t('settings.appLockClearPasscode')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(action) || !status?.enabled}
              onClick={() => {
                void onLockNow()
              }}
            >
              {t('settings.appLockLockNow')}
            </button>
          </div>

          {!canEnable ? (
            <StatusCallout
              tone="warning"
              title={t('settings.appLockNeedsPasscodeTitle')}
              body={t('settings.appLockNeedsPasscodeBody')}
            />
          ) : null}

          {status?.degradationNotes.map((note) => {
            const localizedNote =
              note ===
              'App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest.'
                ? t('settings.appLockBoundaryBody')
                : note ===
                    'Touch ID is available on this Mac and can unlock the current PathKeep session.'
                  ? t('settings.appLockTouchIdAvailable')
                  : note

            return (
              <p className="dashboard-next-action" key={note}>
                {localizedNote}
              </p>
            )
          })}

          {status?.configPath ? (
            <ReviewPathActionRow
              copyFeedback={copyFeedback}
              copyKey="settings:app-lock-config"
              copyLabel={t('common.copyAction')}
              errorMessage={t('audit.copyFailed')}
              label={t('settings.appLockConfigPath')}
              onCopy={(key, value) => {
                void onCopyPath(key, value)
              }}
              onOpenPath={onOpenPath}
              openPathLabel={t('settings.openDirectory')}
              successMessage={t('common.copiedNotice')}
              value={status.configPath}
            />
          ) : (
            <div className="config-row">
              <span className="config-label">
                {t('settings.appLockConfigPath')}
              </span>
              <span className="config-value mono">
                {t('common.notAvailable')}
              </span>
            </div>
          )}
          <div className="config-row">
            <span className="config-label">
              {t('settings.appLockLastUnlocked')}
            </span>
            <span className="config-value mono">
              {status?.lastUnlockedAt ?? t('common.notAvailable')}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
