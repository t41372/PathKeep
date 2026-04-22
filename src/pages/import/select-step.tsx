/**
 * Import wizard selection step.
 *
 * ## 職責
 * - 渲染 takeout / browser method picker 與 source selection UI。
 * - 顯示 detected browser profiles、manual path toggle、與 selected source summary。
 * - 把 select-step 的互動轉回 route owner 提供的 callbacks。
 *
 * ## 不負責
 * - 不做 inspect/import backend mutation。
 * - 不管理 wizard stepper、preview、或 follow-through review。
 * - 不持有 route-level selection state。
 *
 * ## 依賴關係
 * - 依賴 `src/components/primitives/status-callout.tsx` 呈現 browser discovery fallback。
 * - 依賴 `./shared.ts` 的 `ImportMethod` 型別。
 *
 * ## 性能備注
 * - 只渲染 route owner 已持有的 detected profiles，不做額外查詢。
 */

import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
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
  onScan,
  onSelectBrowserProfile,
  onSourcePathChange,
}: ImportSelectStepProps) {
  const { t } = useI18n()

  return (
    <>
      <div className="import-methods">
        <button
          className={`import-card ${method === 'takeout' ? 'active-import' : ''}`}
          type="button"
          aria-pressed={method === 'takeout'}
          onClick={() => onMethodChange('takeout')}
        >
          <div className="import-card-icon">↓</div>
          <div className="import-card-title">
            {t('import.takeoutMethodTitle')}
          </div>
          <div className="import-card-desc dim">
            {t('import.takeoutMethodBody')}
          </div>
        </button>
        <button
          className={`import-card ${method === 'browser' ? 'active-import' : ''}`}
          type="button"
          aria-pressed={method === 'browser'}
          onClick={() => onMethodChange('browser')}
        >
          <div className="import-card-icon">⊕</div>
          <div className="import-card-title">
            {t('import.browserMethodTitle')}
          </div>
          <div className="import-card-desc dim">
            {t('import.browserMethodBody')}
          </div>
        </button>
      </div>
      {method === 'browser' ? (
        <p className="dim" style={{ marginTop: 'var(--space-2)' }}>
          {t('import.browserPreparationHint')}
        </p>
      ) : null}

      {method === 'takeout' ? (
        <div className="import-guide-shell">
          <div className="import-guide-card import-guide-card--hero">
            <span className="mono-kicker">{t('import.takeoutGuideTitle')}</span>
            <p className="dashboard-next-action">
              {t('import.takeoutScopeBody')}
            </p>
            <p className="dim">{t('import.takeoutPreparationHint')}</p>
          </div>
          <div className="import-guide-grid">
            <div className="import-guide-card" data-tone="ok">
              <span className="mono-kicker">
                {t('import.takeoutScopeTitle')}
              </span>
              <ul className="import-scope-list dim">
                <li>{t('import.takeoutScopeImportable')}</li>
                <li>{t('import.takeoutGuideSupportedExample')}</li>
              </ul>
            </div>
            <div className="import-guide-card" data-tone="warn">
              <span className="mono-kicker">
                {t('import.takeoutUnsupportedTitle')}
              </span>
              <ul className="import-scope-list dim">
                <li>{t('import.takeoutScopeIgnored')}</li>
                <li>{t('import.takeoutScopeReview')}</li>
                <li>{t('import.takeoutGuideUnsupportedExample')}</li>
              </ul>
            </div>
          </div>
          <ol className="import-guide-list import-guide-list--steps dim">
            <li>{t('import.takeoutGuideStepOne')}</li>
            <li>{t('import.takeoutGuideStepTwo')}</li>
            <li>{t('import.takeoutGuideStepThree')}</li>
          </ol>
        </div>
      ) : null}

      <div className="wizard-title">{t('import.selectTitle')}</div>
      <div className="wizard-description dim">
        {method === 'takeout'
          ? t('import.takeoutSelectBody')
          : t('import.browserSelectBody')}
      </div>

      {method === 'browser' ? (
        <div
          className="import-source-stack"
          style={{ marginTop: 'var(--space-4)' }}
        >
          <div className="row-between">
            <span className="mono-kicker">
              {t('import.detectedBrowserProfiles')}
            </span>
            <span className="mono-support">
              {t('import.detectedBrowserProfilesCount', {
                count: detectedBrowserProfiles.length.toLocaleString(language),
              })}
            </span>
          </div>
          {detectedBrowserProfiles.length > 0 ? (
            <div className="import-profile-list">
              {detectedBrowserProfiles.map((profile) => (
                <button
                  key={profile.profileId}
                  className={`result-row import-profile-card ${
                    selectedBrowserProfileId === profile.profileId
                      ? 'result-row--active'
                      : ''
                  }`}
                  type="button"
                  onClick={() => onSelectBrowserProfile(profile)}
                >
                  <div className="result-row__header">
                    <strong>
                      {profile.browserName} · {profile.profileName}
                    </strong>
                    <span className="status-badge">
                      {t('import.browserProfileReady')}
                    </span>
                  </div>
                  <div className="result-row__meta">
                    <span className="mono-support">{profile.profileId}</span>
                    <span className="mono-support">
                      {profile.historyFileName}
                    </span>
                  </div>
                  <p className="mono-support">{profile.historyPath}</p>
                </button>
              ))}
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
          className="btn-secondary"
          type="button"
          onClick={() => {
            void onBrowseSource({ directory: false })
          }}
        >
          {method === 'takeout'
            ? t('import.chooseTakeoutFile')
            : t('import.chooseHistoryFile')}
        </button>
        {method === 'takeout' ? (
          <button
            className="btn-secondary"
            type="button"
            onClick={() => {
              void onBrowseSource({ directory: true })
            }}
          >
            {t('import.chooseTakeoutFolder')}
          </button>
        ) : (
          <button
            aria-expanded={manualPathExpanded}
            className="btn-ghost"
            type="button"
            onClick={() => onManualPathExpandedChange(!manualPathExpanded)}
          >
            {manualPathExpanded
              ? t('import.hideManualPath')
              : t('import.showManualPath')}
          </button>
        )}
      </div>

      {sourcePath.trim() ? (
        <div className="import-source-summary">
          <span className="mono-kicker">{t('import.selectedSource')}</span>
          <span className="mono-support">{sourcePath}</span>
          {method === 'browser' && selectedBrowserProfile ? (
            <span className="mono-support">
              {selectedBrowserProfile.browserName} ·{' '}
              {selectedBrowserProfile.profileName}
            </span>
          ) : null}
        </div>
      ) : null}

      {(method === 'takeout' ||
        manualPathExpanded ||
        detectedBrowserProfiles.length === 0) && (
        <label
          className="field-stack import-manual-path"
          style={{ marginTop: 'var(--space-4)' }}
        >
          <span className="mono-kicker">{t('import.sourcePath')}</span>
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

      <div className="wizard-actions">
        <button
          className="btn-primary"
          type="button"
          onClick={() => {
            void onScan()
          }}
          disabled={!sourcePath.trim()}
          aria-disabled={!sourcePath.trim()}
        >
          {t('import.scanSource')}
        </button>
      </div>
    </>
  )
}
