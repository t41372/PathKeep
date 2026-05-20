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
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import {
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import type { AppLockConfig, AppLockStatus } from '../../lib/types'
import { useI18n } from '../../lib/i18n'
import { cn } from '../../lib/cn'
import { Field } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'

const SELECT_CLASS =
  'border-border-default rounded-paper bg-paper text-ink font-sans text-[12.5px] px-2 py-1 focus:border-accent focus:outline-none disabled:opacity-60'
const INPUT_CLASS =
  'border-border-default rounded-paper bg-paper text-ink w-full font-sans text-[12.5px] px-2 py-1.5 focus:border-accent focus:outline-none disabled:opacity-60'
const BUTTON_PRIMARY =
  'border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'
const BUTTON_SECONDARY =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'

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
    <PaperCard testId={navItem.id}>
      <PaperCardHeader
        title={navItem.label}
        right={<PaperCardBadge>{t('settings.optional')}</PaperCardBadge>}
      />
      <PaperCardBody>
        <div className="mb-4">
          <StatusCallout
            tone={currentSettings.enabled ? 'warning' : 'info'}
            title={t('settings.appLockBoundaryTitle')}
            body={t('settings.appLockBoundaryBody')}
          />
        </div>

        <Field label={t('settings.appLockEnabled')}>
          <label className="text-ink-muted flex items-center gap-2 font-sans text-[12px]">
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
        </Field>

        <Field label={t('settings.appLockStatus')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {status?.locked
              ? t('settings.appLockStatusLocked')
              : t('settings.appLockStatusUnlocked')}
          </span>
        </Field>

        <Field label={t('settings.appLockIdleTimeout')}>
          <select
            aria-label={t('settings.appLockIdleTimeout')}
            className={SELECT_CLASS}
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
        </Field>

        <Field
          label={
            usesTouchId
              ? t('settings.appLockTouchId')
              : t('settings.appLockBiometric')
          }
          help={
            !status?.biometricAvailable
              ? usesTouchId
                ? t('settings.appLockTouchIdUnavailable')
                : t('settings.appLockBiometricUnavailable')
              : undefined
          }
        >
          <label className="text-ink-muted flex items-center gap-2 font-sans text-[12px]">
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
        </Field>

        <Field label={t('settings.appLockRecoveryHint')}>
          <input
            aria-label={t('settings.appLockRecoveryHint')}
            className={INPUT_CLASS}
            placeholder={t('settings.appLockRecoveryHintPlaceholder')}
            type="text"
            value={recoveryHint}
            onChange={(event) => {
              onRecoveryHintChange(event.target.value)
            }}
          />
        </Field>

        <Field label={t('settings.appLockPasscode')}>
          <input
            aria-label={t('settings.appLockPasscode')}
            className={INPUT_CLASS}
            placeholder={t('settings.appLockPasscodePlaceholder')}
            type="password"
            value={passcode}
            onChange={(event) => {
              onPasscodeChange(event.target.value)
            }}
          />
        </Field>

        <div className="flex flex-wrap items-center gap-2 pt-3">
          <button
            className={BUTTON_PRIMARY}
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
            className={BUTTON_SECONDARY}
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
            className={BUTTON_SECONDARY}
            type="button"
            disabled={Boolean(action) || !status?.passcodeConfigured}
            onClick={() => {
              void onClearPasscode()
            }}
          >
            {t('settings.appLockClearPasscode')}
          </button>
          <button
            className={BUTTON_SECONDARY}
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
          <div className="mt-4">
            <StatusCallout
              tone="warning"
              title={t('settings.appLockNeedsPasscodeTitle')}
              body={t('settings.appLockNeedsPasscodeBody')}
            />
          </div>
        ) : null}

        {status?.degradationNotes.length ? (
          <div className="border-border-light mt-4 flex flex-col gap-1 border-t pt-3">
            {status.degradationNotes.map((note) => {
              const localizedNote =
                note ===
                'App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest.'
                  ? t('settings.appLockBoundaryBody')
                  : note ===
                      'Touch ID is available on this Mac and can unlock the current PathKeep session.'
                    ? t('settings.appLockTouchIdAvailable')
                    : note

              return (
                <p
                  className="text-ink-faint m-0 font-mono text-[11px]"
                  key={note}
                >
                  {localizedNote}
                </p>
              )
            })}
          </div>
        ) : null}

        <div className={cn('mt-4 flex flex-col gap-2')}>
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
            <Field label={t('settings.appLockConfigPath')}>
              <span className="text-ink-muted font-mono text-[11.5px]">
                {t('common.notAvailable')}
              </span>
            </Field>
          )}
          <Field label={t('settings.appLockLastUnlocked')}>
            <span className="text-ink-muted font-mono text-[11.5px]">
              {status?.lastUnlockedAt ?? t('common.notAvailable')}
            </span>
          </Field>
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}
