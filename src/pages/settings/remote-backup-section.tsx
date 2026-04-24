/**
 * @file remote-backup-section.tsx
 * @description Renders the remote-backup PME surface from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 remote backup saved-config summary、PME tabs、execute result 與 verify checklist。
 * - 把 preview / execute / verify actions 交回 route-owned handlers。
 * - 維持 remote backup 的 preview-first honesty，不在 section 內繞過 PME。
 *
 * ## 不負責
 * - 不直接編輯或儲存 remote config / credentials；那些 persistent preferences 屬於 `/settings`。
 * - 不觸發 archive export 以外的 side effects。
 * - 不改變 S3-compatible backend contract。
 *
 * ## 依賴關係
 * - 依賴 route hook 提供 draft state、PME tab state 與 mutation handlers。
 * - 依賴 shared review components 呈現 Verify checklist 與 PME tabs。
 *
 * ## 性能備注
 * - 本模組只渲染 route 已載入的 preview/result payload，不自行新增背景查詢。
 */

import { Link } from 'react-router-dom'
import {
  PmeTabBar,
  ReviewSection,
  VerifyCheckList,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type {
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned remote-backup state consumed by the extracted section.
 */
export interface RemoteBackupSectionState {
  accessKeyId: string
  action: string | null
  configured: boolean
  currentDraft: RemoteBackupConfig | null
  latestRemoteBundlePath: string | null
  preview: RemoteBackupPreview | null
  result: RemoteBackupResult | null
  secretAccessKey: string
  tab: 'preview' | 'manual' | 'execute' | 'verify'
  verification: RemoteBackupVerification | null
  onAccessKeyIdChange: (value: string) => void
  onClearCredentials: () => Promise<void>
  onDraftChange: (patch: Partial<RemoteBackupConfig>) => void
  onExecute: () => Promise<void>
  onPreview: () => Promise<void>
  onSaveConfig: () => Promise<void>
  onSecretAccessKeyChange: (value: string) => void
  onSetTab: (tab: 'preview' | 'manual' | 'execute' | 'verify') => void
  onStoreCredentials: () => Promise<void>
  onVerify: () => Promise<void>
}

/**
 * Groups the stable section anchor descriptor with the remote-backup view-model.
 */
export interface RemoteBackupSectionProps {
  credentialsSaved: boolean
  lastError: string | null
  lastUploadedAt: string | null
  lastUploadedObjectKey: string | null
  navItem: SettingsSectionNavItem
  state: RemoteBackupSectionState
}

/**
 * Renders the remote-backup PME review surface from route-owned state.
 */
export function RemoteBackupSection({
  credentialsSaved,
  lastError,
  lastUploadedAt,
  lastUploadedObjectKey,
  navItem,
  state,
}: RemoteBackupSectionProps) {
  const { t } = useI18n()
  const {
    action,
    configured,
    currentDraft,
    latestRemoteBundlePath,
    preview,
    result,
    tab,
    verification,
    onExecute,
    onPreview,
    onSetTab,
    onVerify,
  } = state

  if (!currentDraft) {
    return null
  }
  const configRows = [
    {
      label: t('settings.remoteEnabled'),
      value: currentDraft.enabled ? t('common.yes') : t('common.no'),
    },
    {
      label: t('settings.bucketLabel'),
      value: currentDraft.bucket || t('common.notAvailable'),
    },
    {
      label: t('settings.regionLabel'),
      value: currentDraft.region || t('common.notAvailable'),
    },
    {
      label: t('settings.endpointLabel'),
      value: currentDraft.endpoint || t('common.notAvailable'),
    },
    {
      label: t('settings.prefixLabel'),
      value: currentDraft.prefix || t('common.notAvailable'),
    },
    {
      label: t('settings.pathStyleLabel'),
      value: currentDraft.pathStyle ? t('common.yes') : t('common.no'),
    },
    {
      label: t('settings.uploadAfterBackup'),
      value: currentDraft.uploadAfterBackup ? t('common.yes') : t('common.no'),
    },
  ]

  return (
    <div className="panel panel--optional" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-badge">{t('settings.s3Compatible')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone={configured ? 'info' : 'warning'}
          title={t('settings.remoteMaintenanceConfigTitle')}
          body={t('settings.remoteMaintenanceConfigBody')}
          actions={
            <Link className="btn-secondary" to="/settings#settings-remote">
              {t('settings.remoteMaintenanceEditSettings')}
            </Link>
          }
        />

        <div className="settings-field-grid settings-field-grid--readonly">
          {configRows.map((row) => (
            <div className="config-row" key={row.label}>
              <span className="config-label">{row.label}</span>
              <span className="config-value mono">{row.value}</span>
            </div>
          ))}
        </div>

        <div className="settings-action-row">
          <button
            className="btn-secondary"
            type="button"
            disabled={Boolean(action) || !configured}
            onClick={() => {
              void onPreview()
            }}
          >
            {t('settings.previewRemoteBackup')}
          </button>
          <button
            className="btn-primary"
            type="button"
            disabled={Boolean(action) || !configured || !credentialsSaved}
            onClick={() => {
              void onExecute()
            }}
          >
            {t('settings.executeRemoteBackup')}
          </button>
          <button
            className="btn-secondary"
            type="button"
            disabled={Boolean(action) || !latestRemoteBundlePath}
            onClick={() => {
              void onVerify()
            }}
          >
            {t('settings.verifyRemoteBackup')}
          </button>
        </div>

        <div className="settings-field-grid settings-field-grid--readonly">
          <div className="field-stack">
            <span>{t('settings.credentialsStatus')}</span>
            <strong>
              {credentialsSaved
                ? t('settings.credentialsSaved')
                : t('settings.credentialsMissing')}
            </strong>
            <span className="dim">
              {lastUploadedAt
                ? `${t('settings.lastUploadedAt')}: ${lastUploadedAt}`
                : t('settings.remoteNoUploadYet')}
            </span>
            {lastUploadedObjectKey ? (
              <span className="dim mono">{lastUploadedObjectKey}</span>
            ) : null}
            {lastError ? <span className="dim">{lastError}</span> : null}
          </div>
        </div>

        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">
              <Glyph icon="preview" filled />
              <span>{t('settings.remotePme')}</span>
            </span>
          </div>
          <div className="panel-body">
            <PmeTabBar
              activeTab={tab}
              onChange={onSetTab}
              tabs={[
                { key: 'preview', label: t('common.previewTab') },
                { key: 'manual', label: t('common.manualTab') },
                { key: 'execute', label: t('common.executeTab') },
                { key: 'verify', label: t('common.verifyTab') },
              ]}
            />

            {action ? (
              <StatusCallout tone="info" title={action} body="" />
            ) : null}

            {tab === 'preview' ? (
              <div className="settings-result-list">
                <StatusCallout
                  tone={preview ? 'info' : 'warning'}
                  title={t('settings.previewBoundaryTitle')}
                  body={
                    preview
                      ? t('settings.previewBoundaryReady')
                      : t('settings.previewBoundaryBody')
                  }
                />
                {preview ? (
                  <>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.bundlePath')}
                      </span>
                      <span className="config-value mono">
                        {preview.bundlePath}
                      </span>
                    </div>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.objectKey')}
                      </span>
                      <span className="config-value mono">
                        {preview.objectKey}
                      </span>
                    </div>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.uploadUrl')}
                      </span>
                      <span className="config-value mono">
                        {preview.uploadUrl}
                      </span>
                    </div>
                    <div className="inline-note-list">
                      {preview.warnings.map((warning) => (
                        <div className="result-row" key={warning}>
                          <p>{warning}</p>
                        </div>
                      ))}
                    </div>
                  </>
                ) : null}
              </div>
            ) : null}

            {tab === 'manual' ? (
              <div className="settings-result-list">
                <StatusCallout
                  tone="info"
                  title={t('settings.manualBoundaryTitle')}
                  body={t('settings.manualBoundaryBody')}
                />
                {preview ? (
                  <>
                    <div className="code-panel">
                      <span>{t('settings.previewCommand')}</span>
                      <pre>{preview.previewCommand}</pre>
                    </div>
                    <div className="inline-note-list">
                      {preview.manualSteps.map((step) => (
                        <div className="result-row" key={step}>
                          <p>{step}</p>
                        </div>
                      ))}
                      <div className="result-row">
                        <p>{t('settings.retentionGuidance')}</p>
                      </div>
                    </div>
                  </>
                ) : (
                  <StatusCallout
                    tone="warning"
                    title={t('settings.previewFirstTitle')}
                    body={t('settings.previewFirstBody')}
                  />
                )}
              </div>
            ) : null}

            {tab === 'execute' ? (
              <div className="settings-result-list">
                <StatusCallout
                  tone={result?.uploaded ? 'success' : 'warning'}
                  title={t('settings.executeBoundaryTitle')}
                  body={t('settings.executeBoundaryBody')}
                />
                {result ? (
                  <>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.bundlePath')}
                      </span>
                      <span className="config-value mono">
                        {result.bundlePath}
                      </span>
                    </div>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.objectKey')}
                      </span>
                      <span className="config-value mono">
                        {result.objectKey}
                      </span>
                    </div>
                    <div className="config-row">
                      <span className="config-label">
                        {t('settings.executeMessage')}
                      </span>
                      <span className="config-value">{result.message}</span>
                    </div>
                  </>
                ) : (
                  <StatusCallout
                    tone="info"
                    title={t('settings.executeNotRunTitle')}
                    body={t('settings.executeNotRunBody')}
                  />
                )}
              </div>
            ) : null}

            {tab === 'verify' ? (
              <div className="settings-result-list">
                <StatusCallout
                  tone={verification?.restoreReady ? 'success' : 'warning'}
                  title={t('settings.verifyBoundaryTitle')}
                  body={t('settings.verifyBoundaryBody')}
                />
                {verification ? (
                  <>
                    <VerifyCheckList
                      items={[
                        {
                          key: 'bundle-version',
                          label: t('settings.bundleVersion'),
                          status: verification.bundleVersion,
                        },
                        {
                          key: 'restore-ready',
                          label: t('settings.restoreReady'),
                          status: verification.restoreReady
                            ? t('common.statusClear')
                            : t('common.statusNeedsAttention'),
                        },
                        ...verification.checks.map((check) => ({
                          body: check.message,
                          key: check.name,
                          label: check.name,
                          status: check.status,
                        })),
                      ]}
                    />
                    {verification.restoreSteps.length > 0 ? (
                      <ReviewSection title={t('settings.restoreReady')}>
                        {verification.restoreSteps.map((step) => (
                          <p key={step}>{step}</p>
                        ))}
                      </ReviewSection>
                    ) : null}
                  </>
                ) : (
                  <StatusCallout
                    tone="info"
                    title={t('settings.verifyNotRunTitle')}
                    body={t('settings.verifyNotRunBody')}
                  />
                )}
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  )
}
