/**
 * Paper-redesign chrome rendered above the v0.2 Audit Ledger when the
 * route has `?layout=paper`. The current pass only wires the manifest
 * chain (the most visually distinctive surface from the design); recent
 * runs / storage / snapshots remain in the v0.2 panels below until they
 * each get their own slot.
 *
 * ## Responsibilities
 * - Map the snapshot's recent runs onto PaperAuditChainEntry[].
 * - Label run types and relative "when" strings via the i18n catalog.
 *
 * ## Not responsible for
 * - Fetching audit data (the route still owns the snapshot).
 * - Detail panels, severity calculations, or filter logic — they stay in
 *   the v0.2 Audit Ledger below.
 */

import {
  PaperAuditView,
  type PaperAuditChainEntry,
} from '@/components/explorer-paper'
import type { BackupRunOverview } from '../../lib/types'
import {
  paperRunTypeLabel,
  paperWhenLabel,
  type PaperAuditTranslate,
} from './paper-audit-helpers'

export interface PaperAuditPanelProps {
  recentRuns: readonly BackupRunOverview[]
  currentRunId: number | null
  onSelectRun: (runId: number) => void
  auditT: PaperAuditTranslate
  /** Override `Date.now()` for deterministic relative-time labels. */
  now?: () => number
}

export function PaperAuditPanel({
  recentRuns,
  currentRunId,
  onSelectRun,
  auditT,
  now = Date.now,
}: PaperAuditPanelProps) {
  const chain: PaperAuditChainEntry[] = recentRuns
    .slice(0, 6)
    .map((run) => ({
      id: String(run.id),
      hash: (run.manifestHash ?? '').slice(0, 7) || '—',
      type: paperRunTypeLabel(run.runType, auditT),
      when: paperWhenLabel(run.startedAt, auditT, now),
      current: run.id === currentRunId,
    }))
    .reverse()
  return (
    <div data-testid="paper-audit-panel" className="mb-6">
      <PaperAuditView
        chain={chain}
        onSelectChainBlock={(id) => onSelectRun(Number(id))}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={{
          manifestTitle: auditT('paperManifestTitle'),
          manifestBadge: auditT('paperManifestBadge'),
          manifestCallout: auditT('paperManifestCallout'),
          earlierBlockLabel: auditT('paperEarlierBlockLabel'),
          recentRunsTitle: auditT('paperRecentRunsTitle'),
          recentRunsBadge: auditT('paperRecentRunsBadge'),
          storageTitle: auditT('paperStorageTitle'),
          storageBadge: auditT('paperStorageBadge'),
          exportTitle: auditT('paperExportTitle'),
          snapshotsTitle: auditT('paperSnapshotsTitle'),
          snapshotsBadge: auditT('paperSnapshotsBadge'),
          footer: auditT('paperFooter'),
        }}
        testId="paper-audit-view"
      />
    </div>
  )
}
