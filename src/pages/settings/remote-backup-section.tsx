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
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import {
  PmeTabBar,
  ReviewSection,
  VerifyCheckList,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import type {
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
} from '../../lib/types'
import { Field } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'

const BUTTON_PRIMARY =
  'border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'
const BUTTON_SECONDARY =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'

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
 *
 * Paper aesthetic: PaperCard for the outer + nested for the PME tab body.
 * Config rows use Field; manual + execute + verify tab bodies arrange
 * StatusCallout + Field + mono pre blocks in a vertical paper rhythm.
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
    <PaperCard testId={navItem.id}>
      <span id={navItem.id} aria-hidden />
      <PaperCardHeader
        title={navItem.label}
        right={<PaperCardBadge>{t('settings.s3Compatible')}</PaperCardBadge>}
      />
      <PaperCardBody>
        <div className="mb-4">
          <StatusCallout
            tone={configured ? 'info' : 'warning'}
            title={t('settings.remoteMaintenanceConfigTitle')}
            body={t('settings.remoteMaintenanceConfigBody')}
            actions={
              <Link className={BUTTON_SECONDARY} to="/settings#settings-remote">
                {t('settings.remoteMaintenanceEditSettings')}
              </Link>
            }
          />
        </div>

        {configRows.map((row) => (
          <Field key={row.label} label={row.label}>
            <span className="text-ink-muted font-mono text-[11.5px]">
              {row.value}
            </span>
          </Field>
        ))}

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            className={BUTTON_SECONDARY}
            type="button"
            disabled={Boolean(action) || !configured}
            onClick={() => {
              void onPreview()
            }}
          >
            {t('settings.previewRemoteBackup')}
          </button>
          <button
            className={BUTTON_PRIMARY}
            type="button"
            disabled={Boolean(action) || !configured || !credentialsSaved}
            onClick={() => {
              void onExecute()
            }}
          >
            {t('settings.executeRemoteBackup')}
          </button>
          <button
            className={BUTTON_SECONDARY}
            type="button"
            disabled={Boolean(action) || !latestRemoteBundlePath}
            onClick={() => {
              void onVerify()
            }}
          >
            {t('settings.verifyRemoteBackup')}
          </button>
        </div>

        <div className="border-border-light mt-4 flex flex-col gap-1 border-t pt-3">
          <span className="text-ink-faint font-mono text-[10px] tracking-[0.08em] uppercase">
            {t('settings.credentialsStatus')}
          </span>
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

        <div className="border-border-light mt-5 rounded-paper border">
          <div className="border-border-light border-b px-3 py-2">
            <span className="text-ink-faint font-mono text-[10px] tracking-[0.08em] uppercase">
              {t('settings.remotePme')}
            </span>
          </div>
          <div className="p-3">
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
              <div className="mt-3">
                <StatusCallout tone="info" title={action} body="" />
              </div>
            ) : null}

            {tab === 'preview' ? (
              <div className="mt-3 flex flex-col gap-3">
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
                    <Field label={t('settings.bundlePath')}>
                      <span className="text-ink-muted font-mono text-[11px] break-all">
                        {preview.bundlePath}
                      </span>
                    </Field>
                    <Field label={t('settings.objectKey')}>
                      <span className="text-ink-muted font-mono text-[11px] break-all">
                        {preview.objectKey}
                      </span>
                    </Field>
                    <Field label={t('settings.uploadUrl')}>
                      <span className="text-ink-muted font-mono text-[11px] break-all">
                        {preview.uploadUrl}
                      </span>
                    </Field>
                    {preview.warnings.length ? (
                      <div className="flex flex-col gap-1">
                        {preview.warnings.map((warning) => (
                          <p
                            key={warning}
                            className="text-ink-faint m-0 font-mono text-[11px]"
                          >
                            {warning}
                          </p>
                        ))}
                      </div>
                    ) : null}
                  </>
                ) : null}
              </div>
            ) : null}

            {tab === 'manual' ? (
              <div className="mt-3 flex flex-col gap-3">
                <StatusCallout
                  tone="info"
                  title={t('settings.manualBoundaryTitle')}
                  body={t('settings.manualBoundaryBody')}
                />
                {preview ? (
                  <>
                    <Field label={t('settings.previewCommand')}>
                      <pre className="border-border-light bg-page text-ink-muted rounded-paper m-0 max-h-64 overflow-y-auto border px-3 py-2 font-mono text-[11px] whitespace-pre-wrap">
                        {preview.previewCommand}
                      </pre>
                    </Field>
                    <div className="flex flex-col gap-1">
                      {preview.manualSteps.map((step) => (
                        <p
                          key={step}
                          className="text-ink-muted m-0 font-mono text-[11px]"
                        >
                          {step}
                        </p>
                      ))}
                      <p className="text-ink-muted m-0 font-mono text-[11px]">
                        {t('settings.retentionGuidance')}
                      </p>
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
              <div className="mt-3 flex flex-col gap-3">
                <StatusCallout
                  tone={result?.uploaded ? 'success' : 'warning'}
                  title={t('settings.executeBoundaryTitle')}
                  body={t('settings.executeBoundaryBody')}
                />
                {result ? (
                  <>
                    <Field label={t('settings.bundlePath')}>
                      <span className="text-ink-muted font-mono text-[11px] break-all">
                        {result.bundlePath}
                      </span>
                    </Field>
                    <Field label={t('settings.objectKey')}>
                      <span className="text-ink-muted font-mono text-[11px] break-all">
                        {result.objectKey}
                      </span>
                    </Field>
                    <Field label={t('settings.executeMessage')}>
                      <span className="text-ink-muted font-mono text-[11.5px]">
                        {result.message}
                      </span>
                    </Field>
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
              <div className="mt-3 flex flex-col gap-3">
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
      </PaperCardBody>
    </PaperCard>
  )
}
