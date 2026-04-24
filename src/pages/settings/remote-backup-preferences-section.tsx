/**
 * @file remote-backup-preferences-section.tsx
 * @description Renders saved cloud-backup preferences without the upload/verify workflow.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Keep persistent S3-compatible backup configuration and credentials controls on the Settings page.
 * - Let users edit saved backup preferences without exposing workflow-heavy Preview/Execute/Verify panels.
 * - Link the full backup workflow to Maintenance, the route that owns advanced data operations.
 *
 * ## Not responsible for
 * - Running backup previews, uploads, restore checks, or PME tab flows.
 * - Owning credential storage or config persistence.
 * - Rendering support diagnostics or retention cleanup.
 *
 * ## Dependencies
 * - Consumes the route-owned remote backup state from `use-settings-remote-state`.
 * - Uses the shared Settings section descriptor for stable anchors and labels.
 *
 * ## Performance notes
 * - The component only edits an in-memory config draft and invokes explicit save/credential actions.
 */

import { Link } from 'react-router-dom'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { SettingsSectionNavItem } from './section-nav-items'
import type { RemoteBackupSectionState } from './remote-backup-section'

/**
 * Props for the preference-only cloud-backup settings panel.
 */
export interface RemoteBackupPreferencesSectionProps {
  credentialsSaved: boolean
  lastError: string | null
  lastUploadedAt: string | null
  lastUploadedObjectKey: string | null
  navItem: SettingsSectionNavItem
  state: RemoteBackupSectionState
}

/**
 * Renders saved backup preferences and credential controls without running backup workflows.
 */
export function RemoteBackupPreferencesSection({
  credentialsSaved,
  lastError,
  lastUploadedAt,
  lastUploadedObjectKey,
  navItem,
  state,
}: RemoteBackupPreferencesSectionProps) {
  const { t } = useI18n()
  const {
    accessKeyId,
    action,
    configured,
    currentDraft,
    secretAccessKey,
    onAccessKeyIdChange,
    onClearCredentials,
    onDraftChange,
    onSaveConfig,
    onSecretAccessKeyChange,
    onStoreCredentials,
  } = state

  if (!currentDraft) {
    return null
  }

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
          title={t('settings.remotePreferencesTitle')}
          body={t('settings.remotePreferencesBody')}
          actions={
            <Link className="btn-secondary" to="/maintenance#settings-remote">
              {t('settings.openMaintenance')}
            </Link>
          }
        />

        <div className="settings-field-grid">
          <label className="checkbox-row">
            <input
              aria-label={t('settings.remoteEnabled')}
              checked={currentDraft.enabled}
              type="checkbox"
              onChange={(event) =>
                onDraftChange({ enabled: event.target.checked })
              }
            />
            <span>{t('settings.remoteEnabled')}</span>
          </label>
          <label className="checkbox-row">
            <input
              aria-label={t('settings.pathStyleLabel')}
              checked={currentDraft.pathStyle}
              type="checkbox"
              onChange={(event) =>
                onDraftChange({ pathStyle: event.target.checked })
              }
            />
            <span>{t('settings.pathStyleLabel')}</span>
          </label>
          <label className="checkbox-row">
            <input
              aria-label={t('settings.uploadAfterBackup')}
              checked={currentDraft.uploadAfterBackup}
              type="checkbox"
              onChange={(event) =>
                onDraftChange({ uploadAfterBackup: event.target.checked })
              }
            />
            <span>{t('settings.uploadAfterBackup')}</span>
          </label>
          <label className="field-stack">
            <span>{t('settings.bucketLabel')}</span>
            <input
              aria-label={t('settings.bucketLabel')}
              value={currentDraft.bucket}
              onChange={(event) =>
                onDraftChange({ bucket: event.target.value })
              }
            />
          </label>
          <label className="field-stack">
            <span>{t('settings.regionLabel')}</span>
            <input
              aria-label={t('settings.regionLabel')}
              value={currentDraft.region}
              onChange={(event) =>
                onDraftChange({ region: event.target.value })
              }
            />
          </label>
          <label className="field-stack">
            <span>{t('settings.endpointLabel')}</span>
            <input
              aria-label={t('settings.endpointLabel')}
              placeholder={t('settings.endpointPlaceholder')}
              value={currentDraft.endpoint ?? ''}
              onChange={(event) =>
                onDraftChange({ endpoint: event.target.value || null })
              }
            />
          </label>
          <label className="field-stack">
            <span>{t('settings.prefixLabel')}</span>
            <input
              aria-label={t('settings.prefixLabel')}
              value={currentDraft.prefix}
              onChange={(event) =>
                onDraftChange({ prefix: event.target.value })
              }
            />
          </label>
        </div>

        <div className="settings-action-row">
          <button
            className="btn-secondary"
            type="button"
            disabled={Boolean(action)}
            onClick={() => {
              void onSaveConfig()
            }}
          >
            {t('settings.saveRemoteSettings')}
          </button>
        </div>

        <div className="settings-remote-columns">
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

          <div className="settings-field-grid">
            <label className="field-stack">
              <span>{t('settings.accessKeyId')}</span>
              <input
                aria-label={t('settings.accessKeyId')}
                value={accessKeyId}
                onChange={(event) => onAccessKeyIdChange(event.target.value)}
              />
            </label>
            <label className="field-stack">
              <span>{t('settings.secretAccessKey')}</span>
              <input
                aria-label={t('settings.secretAccessKey')}
                type="password"
                value={secretAccessKey}
                onChange={(event) =>
                  onSecretAccessKeyChange(event.target.value)
                }
              />
            </label>
            <div className="settings-action-row">
              <button
                className="btn-secondary"
                type="button"
                disabled={
                  Boolean(action) ||
                  !accessKeyId.trim() ||
                  !secretAccessKey.trim()
                }
                onClick={() => {
                  void onStoreCredentials()
                }}
              >
                {t('settings.storeRemoteCredentials')}
              </button>
              <button
                className="btn-danger"
                type="button"
                disabled={Boolean(action) || !credentialsSaved}
                onClick={() => {
                  void onClearCredentials()
                }}
              >
                {t('settings.clearRemoteCredentials')}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
