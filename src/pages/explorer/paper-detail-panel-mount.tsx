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
 * - Computing the URL's overall first/last visit span, total visit count,
 *   or typed count. A browse `HistoryEntry` is a single visit row carrying
 *   only that visit's timestamp; it has no per-URL aggregate. So this mount
 *   feeds the panel the one honest fact it has — `visitedAt`, the time of the
 *   opened visit — via the panel's single-visit `visitedAt` field, rather
 *   than fabricating identical First/Last dates. Surfacing the real per-URL
 *   summary needs a backend per-URL read (none exists today) and is tracked
 *   for the backend track.
 */

import {
  PaperDetailPanel,
  PaperEnrichedContent,
} from '@/components/explorer-paper'
import type { HistoryEntry } from '@/lib/types/archive'
import { profileIdLabel } from '@/lib/profile-scope-context'
import {
  buildPaperDetailPanelCopy,
  buildPaperEnrichedContentCopy,
} from './paper-explorer-copy'
import type { LocalAnnotations } from './use-local-annotations'
import type { VisitEnrichment } from './use-visit-enrichment'

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
  /**
   * "All of {domain}" action handler. Lives at the route because
   * `useNavigate` requires a Router context — the mount tests render
   * without one. When omitted the panel suppresses the
   * Look-further row entirely (the rest are still unwired placeholders).
   */
  onOpenDomain?: (domain: string) => void
  /**
   * Star affordance for the open page. When provided the panel shows a star
   * toggle in its action row; `isStarred` reads the route's optimistic star
   * cache and `onToggleStar` flips it (write-through happens in the hook).
   */
  stars?: {
    isStarred: (url: string) => boolean
    onToggleStar: (url: string) => void
  }
  /**
   * Site-content enrichment for the open visit (W-ENRICH-1). When provided the
   * panel shows the "Enriched content" section + Fetch-now PME. The route owns
   * the hook (it needs the shell snapshot for consent state); this mount only
   * renders the section into the panel's `enrichedSlot`. Omit to hide it.
   */
  enrichment?: VisitEnrichment
}

/**
 * Format a visit timestamp into the panel's `YYYY-MM-DD HH:mm` shape.
 *
 * Exists so the single-visit "Visited" field reads like the rest of the
 * detail panel (which documents formatted strings such as "2025-11-04
 * 09:17") instead of dumping the raw RFC3339 string. The digits are a
 * locale-neutral timestamp, not user copy, so they carry no i18n key.
 *
 * `visitTime` is epoch from the archive. Production rows store milliseconds
 * while some fixtures use seconds, so this normalizes both with the same
 * `> 1e12` heuristic the contact sheet uses. An unparseable value yields
 * `'—'` so a corrupt row degrades to the panel's standard empty marker
 * rather than rendering "Invalid Date".
 *
 * Kept module-private (not exported) so this file stays a pure
 * component-export surface for react-refresh; its branches are covered
 * through the rendered panel in the mount's test.
 */
function formatVisitTimestamp(visitTime: number): string {
  const ms = visitTime > 1e12 ? visitTime : visitTime * 1000
  const date = new Date(ms)
  if (Number.isNaN(date.getTime())) return '—'
  const pad = (value: number) => String(value).padStart(2, '0')
  const year = date.getFullYear()
  const month = pad(date.getMonth() + 1)
  const day = pad(date.getDate())
  const hour = pad(date.getHours())
  const minute = pad(date.getMinutes())
  return `${year}-${month}-${day} ${hour}:${minute}`
}

export function PaperDetailPanelMount({
  selectedEntry,
  annotations,
  explorerT,
  onClose,
  onOpen,
  onCopyUrl,
  onOpenDomain,
  stars,
  enrichment,
}: PaperDetailPanelMountProps) {
  if (!selectedEntry) return null
  return (
    <PaperDetailPanel
      entry={{
        id: selectedEntry.id,
        title: selectedEntry.title ?? selectedEntry.url,
        url: selectedEntry.url,
        domain: selectedEntry.domain,
        visitedAt: formatVisitTimestamp(selectedEntry.visitTime),
        source: profileIdLabel(selectedEntry.profileId),
        faviconDataUrl: selectedEntry.favicon?.dataUrl ?? null,
        ogImageDataUrl: selectedEntry.ogImage?.dataUrl ?? null,
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
      annotationError={annotations.lastError}
      isStarred={stars?.isStarred(selectedEntry.url) ?? false}
      onToggleStar={
        stars ? () => stars.onToggleStar(selectedEntry.url) : undefined
      }
      onOpenDomain={
        onOpenDomain
          ? (entry) => {
              onOpenDomain(entry.domain)
              onClose()
            }
          : undefined
      }
      enrichedSlot={
        enrichment ? (
          <PaperEnrichedContent
            state={enrichment.state}
            copy={buildPaperEnrichedContentCopy(explorerT)}
            fetchEnabled={enrichment.fetchEnabled}
            fetchPending={enrichment.fetchPending}
            fetchError={enrichment.fetchError}
            onFetchNow={enrichment.fetchNow}
            testId="explorer-paper-detail-enriched"
          />
        ) : undefined
      }
      copy={buildPaperDetailPanelCopy(explorerT)}
      testId="explorer-paper-detail-panel"
    />
  )
}
