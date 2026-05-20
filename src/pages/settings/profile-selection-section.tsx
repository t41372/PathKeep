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

import {
  PaperCard,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { BrowserIcon } from '../../lib/browser-icons'
import { useI18n } from '../../lib/i18n'
import { cn } from '../../lib/cn'
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
 * Paper aesthetic: PaperCard wrapper + per-profile button row using paper
 * tokens (border-border-light, hover:bg-hover, accent fill when selected).
 * BrowserIcon SVG kept inline.
 */
export function ProfileSelectionSection({
  navItem,
  state,
}: ProfileSelectionSectionProps) {
  const { t } = useI18n()
  const { profiles, saving, selectedIds, onToggleProfile } = state

  return (
    <PaperCard testId={navItem.id}>
      <span id={navItem.id} aria-hidden />
      <PaperCardHeader title={navItem.label} />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.browserProfilesBody')}
        </p>
        <div className="flex flex-col gap-1.5">
          {profiles.map((profile) => {
            const checked = selectedIds.has(profile.profileId)
            const historyFileLabel =
              profile.historyFileName ||
              profile.historyPath?.split(/[\\/]/).pop() ||
              profile.profileName
            return (
              <button
                className={cn(
                  'rounded-paper flex w-full items-center gap-3 border px-3 py-2 text-left transition-colors',
                  checked
                    ? 'border-accent bg-accent-soft'
                    : 'border-border-default hover:border-ink-muted hover:bg-hover',
                  saving ? 'cursor-not-allowed opacity-60' : '',
                )}
                disabled={saving}
                key={profile.profileId}
                type="button"
                onClick={() => {
                  void onToggleProfile(profile.profileId)
                }}
              >
                <span
                  className={cn(
                    'rounded-paper inline-grid h-5 w-5 place-items-center border font-mono text-[10px]',
                    checked
                      ? 'border-accent bg-accent text-white'
                      : 'border-border-default text-ink-faint',
                  )}
                >
                  {checked ? '✓' : ''}
                </span>
                <span className="h-5 w-5 shrink-0">
                  <BrowserIcon browserName={profile.browserName} />
                </span>
                <span className="flex flex-1 flex-col">
                  <span className="text-ink font-sans text-[12.5px] font-medium">
                    {profile.browserName} / {profile.profileName}
                  </span>
                  <span className="text-ink-faint font-mono text-[10.5px]">
                    {historyFileLabel}
                  </span>
                </span>
                <span className="text-ink-faint font-mono text-[10.5px]">
                  {profile.historyExists
                    ? `${t('settings.historyFound')} · ${profile.browserVersion ?? t('common.notAvailable')}`
                    : t('settings.noHistoryDetected')}
                </span>
              </button>
            )
          })}
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}
