/**
 * Dashboard archive card: location, sizes, manifest hash, Export / Reveal.
 *
 * Reads readonly archive surface info from the snapshot + dashboard. Uses
 * existing helper APIs for opening paths and exports later in the redesign;
 * for now Export deep-links to /audit and Reveal deep-links to /maintenance.
 */

import { useNavigate } from 'react-router-dom'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { useI18n } from '@/lib/i18n'
import type { StorageSummary } from '@/lib/types'

export interface DashboardArchiveCardProps {
  databasePath: string
  archiveMode: 'Plaintext' | 'Encrypted'
  totalBytes: number
  storage: StorageSummary
  latestManifestHash: string | null
}

export function DashboardArchiveCard({
  databasePath,
  archiveMode,
  totalBytes,
  storage,
  latestManifestHash,
}: DashboardArchiveCardProps) {
  const { t } = useI18n()
  const navigate = useNavigate()

  const lines: Array<[string, number]> = [
    [t('dashboard.archiveLineCore'), storage.archiveDatabaseBytes],
    [
      t('dashboard.archiveLineFts'),
      storage.searchDatabaseBytes + storage.intelligenceDatabaseBytes,
    ],
    [
      t('dashboard.archiveLineSnapshots'),
      storage.snapshotBytes + storage.manifestBytes,
    ],
  ]

  return (
    <PaperCard testId="dashboard-archive-card">
      <PaperCardHeader title={t('dashboard.archiveCardTitle')} />
      <PaperCardBody className="px-4 py-3">
        <div className="bg-page mb-2.5 break-all rounded-paper px-2 py-1.5 font-mono text-[10.5px] leading-[1.4] text-ink-muted">
          {databasePath || '~/PathKeep/archive.db'}
        </div>
        <div className="text-ink-faint mb-2.5 font-mono text-[9.5px] tracking-[0.08em] uppercase">
          {archiveMode === 'Encrypted'
            ? t('dashboard.archiveModeEncrypted')
            : t('dashboard.archiveModePlaintext')}
          {' · '}
          {humanizeBytes(totalBytes)}
        </div>
        {lines.map(([label, bytes]) => (
          <div
            key={label}
            className="flex justify-between py-1 text-[12px] text-ink-secondary"
          >
            <span>{label}</span>
            <span className="font-mono text-[11px] text-ink-muted">
              {humanizeBytes(bytes)}
            </span>
          </div>
        ))}
        <div className="border-border-light mt-2.5 border-t pt-2 font-serif text-[12px] leading-[1.4] italic text-ink-faint">
          {latestManifestHash
            ? `${t('dashboard.archiveChainVerified')} · `
            : `${t('dashboard.archiveAwaitingFirstRun')} · `}
          <code className="font-mono text-[10px] not-italic text-ink-muted">
            {latestManifestHash
              ? `${latestManifestHash.slice(0, 4)}…${latestManifestHash.slice(-4)}`
              : '----…----'}
          </code>
        </div>
        <div className="mt-2.5 flex gap-1">
          <button
            type="button"
            className="border-border-default text-ink hover:bg-hover hover:border-ink-muted flex-1 border px-2 py-1 font-sans text-[11px] transition-colors"
            onClick={() => void navigate('/audit')}
          >
            {t('dashboard.archiveExport')}
          </button>
          <button
            type="button"
            className="border-border-default text-ink hover:bg-hover hover:border-ink-muted flex-1 border px-2 py-1 font-sans text-[11px] transition-colors"
            onClick={() => void navigate('/maintenance')}
          >
            {t('dashboard.archiveReveal')}
          </button>
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}

function humanizeBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`
}
