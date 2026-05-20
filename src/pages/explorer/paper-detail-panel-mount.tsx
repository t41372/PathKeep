/**
 * Glue component that projects an Explorer `HistoryEntry` selection into the
 * generic PaperDetailPanel.
 *
 * Splitting this out of `explorer/index.tsx` keeps the route file's render
 * block thinner and gives the visit → detail-panel mapping its own
 * unit-test surface. The route only has to decide whether the panel is
 * mounted; this component owns the entry shape, copy bundle, and
 * annotation read/write delegation.
 *
 * ## Responsibilities
 * - Translate a HistoryEntry into the PaperDetailPanel entry shape.
 * - Wire the Notes / Tags textbox + tag chip surface to the annotations
 *   hook supplied by the route.
 * - Forward open / copy-url interactions to route-supplied callbacks.
 *
 * ## Not responsible for
 * - Owning annotation state — the route picks between the local and
 *   desktop hooks and hands the unified `LocalAnnotations` object in.
 * - Tracking selection state.
 */

import { PaperDetailPanel } from '@/components/explorer-paper'
import type { HistoryEntry } from '@/lib/types/archive'
import { profileIdLabel } from '@/lib/profile-scope-context'
import { buildPaperDetailPanelCopy } from './paper-explorer-copy'
import type { LocalAnnotations } from './use-local-annotations'

export interface PaperDetailPanelMountProps {
  /** The currently-selected history entry, or `null` when nothing is open. */
  selectedEntry: HistoryEntry | null
  /** Annotation store — could be the local-storage hook or the desktop hook. */
  annotations: LocalAnnotations
  /** Explorer-namespace translator used to build the panel copy. */
  explorerT: (key: string, vars?: Record<string, string | number>) => string
  /** Tells the route to close the panel. */
  onClose: () => void
  /**
   * Triggered when the user clicks the panel's "Open" action. Receives the
   * canonical page URL so the route can pipe it into the same
   * `handleVisit(url)` / `openExternalUrl(url)` flow that v0.2 used —
   * passing the entry id here is a bug, because the route hands it to
   * `openExternalUrl` which would otherwise try to navigate to the literal
   * row id ("42").
   */
  onOpen: (url: string) => void
  /** Copy-URL action handler; falls back to the global clipboard when undefined. */
  onCopyUrl?: (url: string) => void
}

export function PaperDetailPanelMount({
  selectedEntry,
  annotations,
  explorerT,
  onClose,
  onOpen,
  onCopyUrl,
}: PaperDetailPanelMountProps) {
  if (!selectedEntry) return null
  return (
    <PaperDetailPanel
      entry={{
        id: selectedEntry.id,
        title: selectedEntry.title ?? selectedEntry.url,
        url: selectedEntry.url,
        domain: selectedEntry.domain,
        firstVisitAt: selectedEntry.visitedAt,
        lastVisitAt: selectedEntry.visitedAt,
        source: profileIdLabel(selectedEntry.profileId),
      }}
      notes={annotations.notesFor(selectedEntry.url)}
      tags={annotations.tagsFor(selectedEntry.url)}
      onClose={onClose}
      onOpen={(entry) => onOpen(entry.url)}
      onCopyUrl={(entry) => {
        if (onCopyUrl) {
          onCopyUrl(entry.url)
        } else {
          void globalThis.navigator?.clipboard?.writeText(entry.url)
        }
      }}
      onUpdateNotes={(next) => annotations.updateNotes(selectedEntry.url, next)}
      onUpdateTags={(next) => annotations.updateTags(selectedEntry.url, next)}
      copy={buildPaperDetailPanelCopy(explorerT)}
      testId="explorer-paper-detail-panel"
    />
  )
}
