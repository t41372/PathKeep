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
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import { Field } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'
import type { RemoteBackupSectionState } from './remote-backup-section'

const INPUT_CLASS =
  'border-border-default rounded-paper bg-paper text-ink w-full font-mono text-[11.5px] px-2 py-1.5 focus:border-accent focus:outline-none disabled:opacity-60'
const BUTTON_SECONDARY =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'
const BUTTON_DANGER =
  'border-danger text-danger hover:bg-danger-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'

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
 *
 * Paper aesthetic: PaperCard wrap with mono "S3-compatible" badge; checkboxes
 * + text inputs use paper tokens; credential review uses Field rows with
 * status + last-uploaded annotation.
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
    <PaperCard testId={navItem.id}>
      <PaperCardHeader
        title={navItem.label}
        right={<PaperCardBadge>{t('settings.s3Compatible')}</PaperCardBadge>}
      />
      <PaperCardBody>
        <div className="mb-4">
          <StatusCallout
            tone={configured ? 'info' : 'warning'}
            title={t('settings.remotePreferencesTitle')}
            body={t('settings.remotePreferencesBody')}
            actions={
              <Link
                className={BUTTON_SECONDARY}
                to="/maintenance#settings-remote"
              >
                {t('settings.openMaintenance')}
              </Link>
            }
          />
        </div>

        <Field label={t('settings.remoteEnabled')}>
          <label className="text-ink-muted flex items-center gap-2 font-sans text-[12px]">
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
        </Field>

        <Field label={t('settings.pathStyleLabel')}>
          <label className="text-ink-muted flex items-center gap-2 font-sans text-[12px]">
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
        </Field>

        <Field label={t('settings.uploadAfterBackup')}>
          <label className="text-ink-muted flex items-center gap-2 font-sans text-[12px]">
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
        </Field>

        <Field label={t('settings.bucketLabel')}>
          <input
            aria-label={t('settings.bucketLabel')}
            className={INPUT_CLASS}
            value={currentDraft.bucket}
            onChange={(event) => onDraftChange({ bucket: event.target.value })}
          />
        </Field>

        <Field label={t('settings.regionLabel')}>
          <input
            aria-label={t('settings.regionLabel')}
            className={INPUT_CLASS}
            value={currentDraft.region}
            onChange={(event) => onDraftChange({ region: event.target.value })}
          />
        </Field>

        <Field label={t('settings.endpointLabel')}>
          <input
            aria-label={t('settings.endpointLabel')}
            className={INPUT_CLASS}
            placeholder={t('settings.endpointPlaceholder')}
            value={currentDraft.endpoint ?? ''}
            onChange={(event) =>
              onDraftChange({ endpoint: event.target.value || null })
            }
          />
        </Field>

        <Field label={t('settings.prefixLabel')}>
          <input
            aria-label={t('settings.prefixLabel')}
            className={INPUT_CLASS}
            value={currentDraft.prefix}
            onChange={(event) => onDraftChange({ prefix: event.target.value })}
          />
        </Field>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            className={BUTTON_SECONDARY}
            type="button"
            disabled={Boolean(action)}
            onClick={() => {
              void onSaveConfig()
            }}
          >
            {t('settings.saveRemoteSettings')}
          </button>
        </div>

        <div className="border-border-light mt-4 flex flex-col gap-3 border-t pt-3">
          <Field label={t('settings.credentialsStatus')}>
            <div className="flex flex-col gap-1">
              <strong className="text-ink font-sans text-[12px]">
                {credentialsSaved
                  ? t('settings.credentialsSaved')
                  : t('settings.credentialsMissing')}
              </strong>
              <span className="text-ink-faint font-mono text-[10.5px]">
                {lastUploadedAt
                  ? `${t('settings.lastUploadedAt')}: ${lastUploadedAt}`
                  : t('settings.remoteNoUploadYet')}
              </span>
              {lastUploadedObjectKey ? (
                <span className="text-ink-faint font-mono text-[10.5px]">
                  {lastUploadedObjectKey}
                </span>
              ) : null}
              {lastError ? (
                <span className="text-danger font-mono text-[10.5px]">
                  {lastError}
                </span>
              ) : null}
            </div>
          </Field>

          <Field label={t('settings.accessKeyId')}>
            <input
              aria-label={t('settings.accessKeyId')}
              className={INPUT_CLASS}
              value={accessKeyId}
              onChange={(event) => onAccessKeyIdChange(event.target.value)}
            />
          </Field>

          <Field label={t('settings.secretAccessKey')}>
            <input
              aria-label={t('settings.secretAccessKey')}
              className={INPUT_CLASS}
              type="password"
              value={secretAccessKey}
              onChange={(event) => onSecretAccessKeyChange(event.target.value)}
            />
          </Field>

          <div className="flex flex-wrap items-center gap-2">
            <button
              className={BUTTON_SECONDARY}
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
              className={BUTTON_DANGER}
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
      </PaperCardBody>
    </PaperCard>
  )
}
