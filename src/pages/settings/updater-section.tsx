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
 * - 不收集或傳送產品使用資料。
 * - 不決定 release source 或 updater backend contract。
 *
 * ## 依賴關係
 * - 依賴 `use-settings-route-state.ts` 提供的 updater state 與 handlers。
 * - 依賴 `useI18n()` 和 `formatBytes()` 呈現 localized progress copy。
 *
 * ## 性能備注
 * - 本模組只渲染 route 已經準備好的 state，不自行發起 background work。
 */

import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type {
  AppBuildInfo,
  UpdateAvailability,
  UpdateInstallState,
} from '../../lib/types'
import type { PendingAppUpdate } from '../../lib/update'
import { Field } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'

const BUTTON_PRIMARY =
  'border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'
const BUTTON_SECONDARY =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'

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
 * Paper aesthetic: PaperCard with mono version chip in the header right slot;
 * Field rows for the version metadata; native progress bar styled with
 * paper tokens; CTAs use paper utility button classes.
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
    <PaperCard testId={navItem.id}>
      <PaperCardHeader
        title={navItem.label}
        right={
          <span className="text-ink-faint font-mono text-[10.5px]">
            {buildInfo?.version ?? t('common.notAvailable')}
          </span>
        }
      />
      <PaperCardBody>
        <div className="mb-4">
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
            body={
              updateInstallState.message ?? t('settings.updateBoundaryBody')
            }
          />
        </div>

        <Field label={t('settings.updateCurrentVersion')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {buildInfo?.version ?? t('common.notAvailable')}
          </span>
        </Field>

        <Field label={t('settings.updateLatestVersion')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {updateAvailability?.version ?? t('common.notAvailable')}
          </span>
        </Field>

        <Field label={t('settings.updatePublishedAt')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {updateAvailability?.publishedAt ?? t('common.notAvailable')}
          </span>
        </Field>

        <Field label={t('settings.updateCheckedAt')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {updateAvailability?.checkedAt ?? t('common.notAvailable')}
          </span>
        </Field>

        {updateInstallState.contentLength ? (
          <div className="mt-3 flex flex-col gap-2">
            <div className="bg-border-light rounded-paper relative h-1.5 w-full overflow-hidden">
              <div
                className="bg-accent absolute inset-y-0 left-0 transition-[width]"
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
            <p className="text-ink-faint m-0 font-mono text-[11px]">
              {t('settings.updateProgress', {
                downloaded: formatBytes(
                  updateInstallState.downloadedBytes ?? 0,
                  language,
                ),
                total: formatBytes(updateInstallState.contentLength, language),
              })}
            </p>
          </div>
        ) : null}

        {updateAvailability?.notes ? (
          <Field label={t('settings.updateReleaseNotes')}>
            <pre className="border-border-light bg-page text-ink-muted rounded-paper m-0 max-h-64 overflow-y-auto border px-3 py-2 font-mono text-[11px] whitespace-pre-wrap">
              {updateAvailability.notes}
            </pre>
          </Field>
        ) : null}

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            className={BUTTON_PRIMARY}
            type="button"
            disabled={updateInstallState.phase === 'checking'}
            onClick={() => {
              void onCheckForUpdates()
            }}
          >
            {t('settings.updateCheckNow')}
          </button>
          <button
            className={BUTTON_PRIMARY}
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
          <button
            className={BUTTON_SECONDARY}
            type="button"
            disabled={updateInstallState.phase !== 'installed'}
            onClick={() => {
              void onRelaunchForUpdate()
            }}
          >
            {t('settings.updateRestartNow')}
          </button>
          <button
            className={BUTTON_SECONDARY}
            type="button"
            onClick={() => {
              void onOpenReleasePage()
            }}
          >
            {t('settings.updateOpenReleasePage')}
          </button>
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}
