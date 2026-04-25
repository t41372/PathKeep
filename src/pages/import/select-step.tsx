/**
 * Import wizard selection step — redesigned with proper icons and cleaner layout.
 *
 * ## 職責
 * - 渲染 takeout / browser method picker 與 source selection UI。
 * - 顯示 detected browser profiles、manual path toggle、與 selected source summary。
 * - 把 select-step 的互動轉回 route owner 提供的 callbacks。
 *
 * ## 設計原則
 * - 使用 Glyph 圖標替代 emoji
 * - 清晰的視覺層級與適當的留白
 * - 瀏覽器列表使用專屬圖標與狀態標記
 *
 * ## 依賴關係
 * - 依賴 `src/components/primitives/status-callout.tsx` 呈現 browser discovery fallback。
 * - 依賴 `src/components/ui.tsx` 的 `Glyph` 圖標。
 * - 依賴 `src/lib/browser-icons.tsx` 的 `BrowserIcon` 組件。
 * - 依賴 `./shared.ts` 的 `ImportMethod` 型別。
 */

import { Glyph } from '../../components/ui'
import { StatusCallout } from '../../components/primitives/status-callout'
import { BrowserIcon } from '../../lib/browser-icons'
import { useI18n } from '../../lib/i18n'
import { isBrowserProfileReadable } from '../../lib/platform-guidance'
import type { BrowserProfile } from '../../lib/types'
import type { ImportMethod } from './shared'

/**
 * Props for the extracted Import selection step.
 */
export interface ImportSelectStepProps {
  detectedBrowserProfiles: BrowserProfile[]
  language: string
  manualPathExpanded: boolean
  method: ImportMethod
  selectedBrowserProfile: BrowserProfile | null
  selectedBrowserProfileId: string | null
  sourcePath: string
  onBrowseSource: (options: { directory: boolean }) => void | Promise<void>
  onManualPathExpandedChange: (expanded: boolean) => void
  onMethodChange: (method: ImportMethod) => void
  onOpenFullDiskAccessSettings: () => void | Promise<void>
  onScan: () => void | Promise<void>
  onSelectBrowserProfile: (profile: BrowserProfile) => void
  onSourcePathChange: (path: string) => void
}

/**
 * Renders the Import wizard's source-selection step.
 */
export function ImportSelectStep({
  detectedBrowserProfiles,
  language,
  manualPathExpanded,
  method,
  selectedBrowserProfile,
  selectedBrowserProfileId,
  sourcePath,
  onBrowseSource,
  onManualPathExpandedChange,
  onMethodChange,
  onOpenFullDiskAccessSettings,
  onScan,
  onSelectBrowserProfile,
  onSourcePathChange,
}: ImportSelectStepProps) {
  const { t } = useI18n()
  const readyBrowserProfileCount = detectedBrowserProfiles.filter(
    (profile) => isBrowserProfileReadable(profile) && profile.historyPath,
  ).length

  return (
    <>
      <div className="import-methods">
        <button
          className={`import-method-card ${method === 'takeout' ? 'active' : ''}`}
          type="button"
          aria-pressed={method === 'takeout'}
          onClick={() => onMethodChange('takeout')}
        >
          <div className="import-method-card__icon">
            <Glyph icon="download" />
          </div>
          <div className="import-method-card__content">
            <div className="import-method-card__title">
              {t('import.takeoutMethodTitle')}
            </div>
            <div className="import-method-card__desc">
              {t('import.takeoutMethodBody')}
            </div>
          </div>
          {method === 'takeout' && (
            <div className="import-method-card__indicator">
              <Glyph icon="check" />
            </div>
          )}
        </button>
        <button
          className={`import-method-card ${method === 'browser' ? 'active' : ''}`}
          type="button"
          aria-pressed={method === 'browser'}
          onClick={() => onMethodChange('browser')}
        >
          <div className="import-method-card__icon">
            <Glyph icon="database" />
          </div>
          <div className="import-method-card__content">
            <div className="import-method-card__title">
              {t('import.browserMethodTitle')}
            </div>
            <div className="import-method-card__desc">
              {t('import.browserMethodBody')}
            </div>
          </div>
          {method === 'browser' && (
            <div className="import-method-card__indicator">
              <Glyph icon="check" />
            </div>
          )}
        </button>
      </div>
      {method === 'takeout' ? (
        <div className="import-info-panel">
          <div className="import-info-section">
            <div className="import-info-section__header">
              <Glyph icon="check" />
              <span>{t('import.takeoutScopeTitle')}</span>
            </div>
            <p className="import-info-section__body">
              {t('import.takeoutScopeBody')}
            </p>
            <ul className="import-info-list">
              <li>{t('import.takeoutScopeImportable')}</li>
              <li>{t('import.takeoutGuideSupportedExample')}</li>
            </ul>
          </div>
          <div className="import-info-section import-info-section--muted">
            <div className="import-info-section__header">
              <Glyph icon="warning" />
              <span>{t('import.takeoutUnsupportedTitle')}</span>
            </div>
            <ul className="import-info-list">
              <li>{t('import.takeoutScopeIgnored')}</li>
              <li>{t('import.takeoutScopeReview')}</li>
              <li>{t('import.takeoutGuideUnsupportedExample')}</li>
            </ul>
          </div>
        </div>
      ) : (
        <div className="import-info-panel import-info-panel--compact">
          <p className="import-hint">{t('import.browserPreparationHint')}</p>
        </div>
      )}

      <div className="import-section-title">{t('import.selectTitle')}</div>
      <div className="import-section-desc">
        {method === 'takeout'
          ? t('import.takeoutSelectBody')
          : t('import.browserSelectBody')}
      </div>

      {method === 'browser' ? (
        <div className="import-browser-section">
          <div className="import-browser-header">
            <span className="import-browser-header__label">
              {t('import.detectedBrowserProfiles')}
            </span>
            <span className="import-browser-header__count">
              {t('import.detectedBrowserProfilesCount', {
                count: readyBrowserProfileCount.toLocaleString(language),
              })}
            </span>
          </div>
          {detectedBrowserProfiles.length > 0 ? (
            <div className="import-browser-list">
              {detectedBrowserProfiles.map((profile) => {
                const ready = Boolean(
                  isBrowserProfileReadable(profile) && profile.historyPath,
                )
                const selected = selectedBrowserProfileId === profile.profileId
                return (
                  <BrowserProfileCard
                    key={profile.profileId}
                    profile={profile}
                    ready={ready}
                    selected={selected}
                    t={t}
                    onSelect={() => onSelectBrowserProfile(profile)}
                    onOpenFullDiskAccessSettings={onOpenFullDiskAccessSettings}
                  />
                )
              })}
            </div>
          ) : (
            <StatusCallout
              tone="warning"
              title={t('import.noDetectedBrowserProfilesTitle')}
              body={t('import.noDetectedBrowserProfilesBody')}
            />
          )}
        </div>
      ) : null}

      <div className="import-source-actions">
        <button
          className="btn-secondary import-source-btn"
          type="button"
          onClick={() => {
            void onBrowseSource({ directory: false })
          }}
        >
          <Glyph icon={method === 'takeout' ? 'download' : 'database'} />
          <span>
            {method === 'takeout'
              ? t('import.chooseTakeoutFile')
              : t('import.chooseHistoryFile')}
          </span>
        </button>
        {method === 'takeout' ? (
          <button
            className="btn-secondary import-source-btn"
            type="button"
            onClick={() => {
              void onBrowseSource({ directory: true })
            }}
          >
            <Glyph icon="folder_open" />
            <span>{t('import.chooseTakeoutFolder')}</span>
          </button>
        ) : (
          <button
            aria-expanded={manualPathExpanded}
            className="btn-ghost import-source-btn"
            type="button"
            onClick={() => onManualPathExpandedChange(!manualPathExpanded)}
          >
            <Glyph icon="build" />
            <span>
              {manualPathExpanded
                ? t('import.hideManualPath')
                : t('import.showManualPath')}
            </span>
          </button>
        )}
      </div>

      {sourcePath.trim() ? (
        <div className="import-source-summary">
          <div className="import-source-summary__label">
            <Glyph icon="check" />
            <span>{t('import.selectedSource')}</span>
          </div>
          <div className="import-source-summary__path">{sourcePath}</div>
          {method === 'browser' && selectedBrowserProfile ? (
            <div className="import-source-summary__profile">
              <BrowserIcon
                browserName={selectedBrowserProfile.browserName}
                className="import-source-summary__icon"
                decorative
              />
              <span>
                {selectedBrowserProfile.browserName} ·{' '}
                {selectedBrowserProfile.profileName}
              </span>
            </div>
          ) : null}
        </div>
      ) : null}

      {(method === 'takeout' ||
        manualPathExpanded ||
        detectedBrowserProfiles.length === 0) && (
        <label className="import-path-field">
          <span className="import-path-field__label">
            {t('import.sourcePath')}
          </span>
          <input
            type="text"
            value={sourcePath}
            onChange={(event) => {
              onSourcePathChange(event.target.value)
              if (method === 'browser') {
                onManualPathExpandedChange(true)
              }
            }}
            placeholder={
              method === 'takeout'
                ? t('import.takeoutPathPlaceholder')
                : t('import.browserPathPlaceholder')
            }
          />
        </label>
      )}

      <div className="import-actions">
        <button
          className="btn-primary import-scan-btn"
          type="button"
          onClick={() => {
            void onScan()
          }}
          disabled={!sourcePath.trim()}
          aria-disabled={!sourcePath.trim()}
        >
          <span>{t('import.scanSource')}</span>
          <Glyph icon="arrow_forward" />
        </button>
      </div>
    </>
  )
}

/**
 * Browser profile card component with icon and status.
 */
function BrowserProfileCard({
  profile,
  ready,
  selected,
  t,
  onSelect,
  onOpenFullDiskAccessSettings,
}: {
  profile: BrowserProfile
  ready: boolean
  selected: boolean
  t: (key: string, params?: Record<string, string | number>) => string
  onSelect: () => void
  onOpenFullDiskAccessSettings: () => void | Promise<void>
}) {
  const profileLabel = `${profile.browserName} · ${profile.profileName}`
  const cardContent = (
    <>
      <div className="browser-profile-card__header">
        <div className="browser-profile-card__identity">
          <BrowserIcon
            browserName={profile.browserName}
            className="browser-profile-card__icon"
            decorative
          />
          <div className="browser-profile-card__names">
            <span className="browser-profile-card__browser">
              {profile.browserName}
            </span>
            <span className="browser-profile-card__profile">
              {profile.profileName}
            </span>
          </div>
        </div>
        <span
          className={`browser-profile-card__status ${
            ready ? 'ready' : 'blocked'
          }`}
        >
          {ready
            ? t('import.browserProfileReady')
            : t('import.browserProfileNeedsAccess')}
        </span>
      </div>
      <div className="browser-profile-card__path">
        {profile.historyPath || profile.profilePath}
      </div>
      {!ready && (
        <div className="browser-profile-card__help">
          <p className="browser-profile-card__hint">
            {profile.browserFamily === 'safari'
              ? t('import.safariFullDiskAccessHint')
              : t('import.browserProfileUnreadable')}
          </p>
          {profile.browserFamily === 'safari' && (
            <button
              className="btn-secondary btn-sm"
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                void onOpenFullDiskAccessSettings()
              }}
            >
              {t('import.openFullDiskAccessSettings')}
            </button>
          )}
        </div>
      )}
    </>
  )

  if (!ready) {
    return (
      <div
        aria-disabled="true"
        className={`browser-profile-card browser-profile-card--disabled ${
          selected ? 'browser-profile-card--selected' : ''
        }`}
      >
        {cardContent}
      </div>
    )
  }

  return (
    <button
      className={`browser-profile-card ${
        selected ? 'browser-profile-card--selected' : ''
      }`}
      type="button"
      aria-label={profileLabel}
      onClick={onSelect}
    >
      {cardContent}
    </button>
  )
}
