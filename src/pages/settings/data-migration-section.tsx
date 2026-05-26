/**
 * Settings → Data migration section.
 *
 * Exports the entire local PathKeep project (config, archive databases,
 * derived projections, audit ledger, raw snapshots, intelligence /
 * semantic sidecars) into a single `.pathkeep-bundle` zip, and imports
 * a bundle produced by another machine back into the live tree.
 *
 * ## Responsibilities
 * - Render the Export action: file picker → backend call → success card
 *   with bundle path + byte size.
 * - Render the Import action: file picker → backend preview → inline PME
 *   confirmation panel → apply → success card with applied migrations +
 *   ".bak preserved" notice.
 * - Surface errors as in-place callouts; the rest of Settings stays
 *   usable while the user retries.
 *
 * ## Not responsible for
 * - Stopping/resuming background workers; the desktop command façade
 *   handles that. The frontend just relays the in-flight state.
 * - Re-encrypting the archive with a different key on import; the user
 *   inherits the source machine's key and can rekey afterwards.
 * - Owning hash-link scrolling — the PaperCard `id` prop preserves the
 *   shared `settings-migration` anchor contract.
 *
 * ## Dependencies
 * - `backend-client.exportAppData / previewAppDataImport /
 *   applyAppDataImport` for the typed transport.
 * - `@tauri-apps/plugin-dialog` for the native save/open file pickers.
 * - `useI18n` for the three-locale copy contract.
 */

import { useCallback, useState } from 'react'
import { useI18n } from '@/lib/i18n'
import { backend } from '@/lib/backend-client'
import type {
  ExportedBundle,
  ImportPreview,
  ImportResult,
} from '@/lib/backend-client/migration'
import { describeError } from '@/lib/errors'
import { formatBytes } from '@/lib/format'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import type { SettingsSectionNavItem } from './section-nav-items'

export interface DataMigrationSectionProps {
  navItem: SettingsSectionNavItem
}

type Phase =
  | { kind: 'idle' }
  | { kind: 'exporting' }
  | { kind: 'exportError'; message: string }
  | { kind: 'exported'; result: ExportedBundle }
  | { kind: 'previewing' }
  | { kind: 'previewError'; message: string }
  | { kind: 'previewed'; bundlePath: string; preview: ImportPreview }
  | { kind: 'applying'; bundlePath: string; preview: ImportPreview }
  | {
      kind: 'applyError'
      message: string
      bundlePath: string
      preview: ImportPreview
    }
  | { kind: 'applied'; result: ImportResult }

/**
 * Default suggested filename for the save-bundle dialog. Embeds the local
 * date so two same-day exports do not silently collide; the `.pathkeep`
 * extension is what the Import picker filters on.
 */
function defaultBundleName(now: Date = new Date()): string {
  const yyyy = now.getFullYear()
  const mm = String(now.getMonth() + 1).padStart(2, '0')
  const dd = String(now.getDate()).padStart(2, '0')
  return `pathkeep-export-${yyyy}-${mm}-${dd}.pathkeep`
}

export function DataMigrationSection({ navItem }: DataMigrationSectionProps) {
  const { t } = useI18n()
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' })

  const handleExport = useCallback(async () => {
    setPhase({ kind: 'exporting' })
    try {
      const { save } = await import('@tauri-apps/plugin-dialog')
      const target = await save({
        defaultPath: defaultBundleName(),
        title: t('settings.migrationExportDialogTitle'),
        filters: [{ name: 'PathKeep bundle', extensions: ['pathkeep'] }],
      })
      if (typeof target !== 'string' || !target.trim()) {
        // User cancelled — reset to idle rather than leaving the
        // "exporting" spinner stuck.
        setPhase({ kind: 'idle' })
        return
      }
      const result = await backend.exportAppData(target)
      setPhase({ kind: 'exported', result })
    } catch (error) {
      setPhase({
        kind: 'exportError',
        message: describeError(error, 'export_app_data'),
      })
    }
  }, [t])

  const handlePickBundleForImport = useCallback(async () => {
    setPhase({ kind: 'previewing' })
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: false,
        multiple: false,
        title: t('settings.migrationImportDialogTitle'),
        filters: [{ name: 'PathKeep bundle', extensions: ['pathkeep'] }],
      })
      if (typeof selected !== 'string' || !selected.trim()) {
        setPhase({ kind: 'idle' })
        return
      }
      const preview = await backend.previewAppDataImport(selected)
      setPhase({ kind: 'previewed', bundlePath: selected, preview })
    } catch (error) {
      setPhase({
        kind: 'previewError',
        message: describeError(error, 'preview_app_data_import'),
      })
    }
  }, [t])

  const handleApplyImport = useCallback(async () => {
    if (phase.kind !== 'previewed') return
    const { bundlePath, preview } = phase
    setPhase({ kind: 'applying', bundlePath, preview })
    try {
      const result = await backend.applyAppDataImport(bundlePath, {
        confirmOverwrite: true,
      })
      setPhase({ kind: 'applied', result })
    } catch (error) {
      setPhase({
        kind: 'applyError',
        message: describeError(error, 'apply_app_data_import'),
        bundlePath,
        preview,
      })
    }
  }, [phase])

  const handleResetPreview = useCallback(() => setPhase({ kind: 'idle' }), [])

  const isExporting = phase.kind === 'exporting'
  const isImporting = phase.kind === 'previewing' || phase.kind === 'applying'

  return (
    <PaperCard id={navItem.id} testId={navItem.id}>
      <PaperCardHeader title={t('settings.migrationTitle')} />
      <PaperCardBody>
        <p className="text-ink-secondary font-serif text-[13.5px] leading-[1.55]">
          {t('settings.migrationIntro')}
        </p>

        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <ActionTile
            title={t('settings.migrationExportAction')}
            description={t('settings.migrationExportDescription')}
            actionLabel={
              isExporting
                ? t('settings.migrationExportingLabel')
                : t('settings.migrationExportAction')
            }
            onClick={handleExport}
            disabled={isExporting || isImporting}
            testId="settings-migration-export"
          />
          <ActionTile
            title={t('settings.migrationImportAction')}
            description={t('settings.migrationImportDescription')}
            actionLabel={
              isImporting
                ? t('settings.migrationImportingLabel')
                : t('settings.migrationImportAction')
            }
            onClick={handlePickBundleForImport}
            disabled={isExporting || isImporting}
            testId="settings-migration-import"
          />
        </div>

        {phase.kind === 'exportError' ? (
          <FeedbackBanner
            tone="error"
            testId="settings-migration-export-error"
            title={t('settings.migrationExportErrorTitle')}
            body={phase.message}
          />
        ) : null}

        {phase.kind === 'exported' ? (
          <FeedbackBanner
            tone="success"
            testId="settings-migration-exported"
            title={t('settings.migrationExportedTitle')}
            body={t('settings.migrationExportedBody')
              .replace('{path}', phase.result.bundlePath)
              .replace('{size}', formatBytes(phase.result.bytesWritten))
              .replace(
                '{fileCount}',
                String(phase.result.manifest.files.length),
              )}
          />
        ) : null}

        {phase.kind === 'previewError' ? (
          <FeedbackBanner
            tone="error"
            testId="settings-migration-preview-error"
            title={t('settings.migrationPreviewErrorTitle')}
            body={phase.message}
          />
        ) : null}

        {phase.kind === 'previewed' ||
        phase.kind === 'applying' ||
        phase.kind === 'applyError' ? (
          <ImportPreviewPanel
            bundlePath={phase.bundlePath}
            preview={phase.preview}
            applying={phase.kind === 'applying'}
            applyError={phase.kind === 'applyError' ? phase.message : null}
            onConfirm={handleApplyImport}
            onCancel={handleResetPreview}
          />
        ) : null}

        {phase.kind === 'applied' ? (
          <FeedbackBanner
            tone="success"
            testId="settings-migration-applied"
            title={t('settings.migrationAppliedTitle')}
            body={t('settings.migrationAppliedBody')
              .replace(
                '{finalSchemaVersion}',
                String(phase.result.finalSchemaVersion),
              )
              .replace(
                '{migrationsApplied}',
                phase.result.migrationsApplied.length === 0
                  ? t('settings.migrationAppliedNoMigrations')
                  : phase.result.migrationsApplied.join(', '),
              )
              .replace(
                '{bakNotice}',
                phase.result.preservedPreviousAsBak
                  ? t('settings.migrationAppliedBakNotice')
                  : '',
              )
              .trim()}
          />
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}

interface ActionTileProps {
  title: string
  description: string
  actionLabel: string
  onClick: () => void
  disabled: boolean
  testId: string
}

function ActionTile({
  title,
  description,
  actionLabel,
  onClick,
  disabled,
  testId,
}: ActionTileProps) {
  return (
    <div className="border-border-light rounded-paper flex flex-col gap-2 border p-3">
      <div className="text-ink font-serif text-[13px] font-medium tracking-[-0.005em]">
        {title}
      </div>
      <p className="text-ink-secondary font-serif text-[12px] leading-[1.5]">
        {description}
      </p>
      <button
        type="button"
        className="btn-secondary mt-auto self-start"
        onClick={onClick}
        disabled={disabled}
        data-testid={testId}
      >
        {actionLabel}
      </button>
    </div>
  )
}

interface ImportPreviewPanelProps {
  bundlePath: string
  preview: ImportPreview
  applying: boolean
  applyError: string | null
  onConfirm: () => void
  onCancel: () => void
}

function ImportPreviewPanel({
  bundlePath,
  preview,
  applying,
  applyError,
  onConfirm,
  onCancel,
}: ImportPreviewPanelProps) {
  const { t } = useI18n()
  return (
    <section
      data-testid="settings-migration-preview"
      className="border-border-default mt-4 rounded-paper border p-4"
    >
      <header className="border-border-light flex items-baseline justify-between border-b pb-2">
        <span className="text-ink font-serif text-[14px] font-medium tracking-[-0.005em]">
          {t('settings.migrationPreviewTitle')}
        </span>
        <span className="text-ink-faint font-mono text-[10.5px]">
          {bundlePath}
        </span>
      </header>
      <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12px]">
        <dt className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.04em]">
          {t('settings.migrationPreviewExportedAt')}
        </dt>
        <dd className="text-ink-secondary font-mono text-[11px]">
          {preview.manifest.exportedAt}
          {preview.manifest.exporterHostname
            ? ` · ${preview.manifest.exporterHostname}`
            : ''}
        </dd>
        <dt className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.04em]">
          {t('settings.migrationPreviewAppVersion')}
        </dt>
        <dd className="text-ink-secondary font-mono text-[11px]">
          {preview.manifest.appVersion}
        </dd>
        <dt className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.04em]">
          {t('settings.migrationPreviewSchemaVersion')}
        </dt>
        <dd className="text-ink-secondary font-mono text-[11px]">
          {preview.manifest.archiveSchemaVersion}
          {preview.schemaUpToDate
            ? ` · ${t('settings.migrationPreviewSchemaCurrent')}`
            : ` → ${t('settings.migrationPreviewSchemaWillMigrate').replace('{count}', String(preview.migrationsToApply.length))}`}
        </dd>
        <dt className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.04em]">
          {t('settings.migrationPreviewArchiveMode')}
        </dt>
        <dd className="text-ink-secondary font-mono text-[11px]">
          {preview.manifest.archiveMode}
        </dd>
        <dt className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.04em]">
          {t('settings.migrationPreviewFileCount')}
        </dt>
        <dd className="text-ink-secondary font-mono text-[11px]">
          {preview.manifest.files.length} ·{' '}
          {formatBytes(preview.bytesToExtract)}
        </dd>
      </dl>

      {preview.willOverwriteExisting ? (
        <p
          data-testid="settings-migration-preview-overwrite-warning"
          className="bg-paper-warm border-accent rounded-paper text-ink mt-3 border-l-[3px] p-3 font-serif text-[12.5px] leading-[1.55]"
        >
          {t('settings.migrationPreviewOverwriteWarning')}
        </p>
      ) : null}

      <details className="mt-3">
        <summary className="text-ink-faint cursor-pointer select-none font-mono text-[10px] uppercase tracking-[0.06em]">
          {t('settings.migrationPreviewExclusionsLabel')}
        </summary>
        <ul className="mt-2 flex flex-col gap-1 pl-2">
          {preview.exclusionNotes.map((note) => (
            <li
              key={note.path}
              className="text-ink-secondary font-mono text-[10.5px] leading-[1.5]"
            >
              <span className="text-ink font-medium">{note.path}</span>
              {' — '}
              <span>{note.reason}</span>
            </li>
          ))}
        </ul>
      </details>

      {applyError ? (
        <FeedbackBanner
          tone="error"
          testId="settings-migration-apply-error"
          title={t('settings.migrationApplyErrorTitle')}
          body={applyError}
        />
      ) : null}

      <div className="mt-4 flex gap-2">
        <button
          type="button"
          className="btn-primary"
          onClick={onConfirm}
          disabled={applying}
          data-testid="settings-migration-confirm"
        >
          {applying
            ? t('settings.migrationApplyingLabel')
            : t('settings.migrationConfirmAction')}
        </button>
        <button
          type="button"
          className="btn-secondary"
          onClick={onCancel}
          disabled={applying}
          data-testid="settings-migration-cancel"
        >
          {t('settings.migrationCancelAction')}
        </button>
      </div>
    </section>
  )
}

interface FeedbackBannerProps {
  tone: 'success' | 'error'
  title: string
  body: string
  testId: string
}

function FeedbackBanner({ tone, title, body, testId }: FeedbackBannerProps) {
  return (
    <div
      data-testid={testId}
      className={
        tone === 'success'
          ? 'border-accent bg-paper-warm text-ink mt-3 rounded-paper border-l-[3px] p-3'
          : 'border-blocked bg-paper-warm text-ink mt-3 rounded-paper border-l-[3px] p-3'
      }
    >
      <div className="font-serif text-[13px] font-medium">{title}</div>
      <div className="text-ink-secondary mt-1 font-serif text-[12px] leading-[1.5]">
        {body}
      </div>
    </div>
  )
}
