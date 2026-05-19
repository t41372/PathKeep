/**
 * Barrel for the paper Browse / contact-sheet primitives.
 *
 * These components are the reusable visual layer for the v0.3 paper redesign
 * of the Explorer route. Each is presentation-only and consumes data from the
 * Explorer route's existing hooks (`useExplorerData`, `useExplorerUrlState`,
 * `useExplorerFavicons`).
 *
 * See `docs/design/handoff/paper-redesign/project/pk-contactsheet.jsx` and
 * `pk-tokens.css` for the visual contract.
 */

export { PaperDayHeader } from './paper-day-header'
export type { PaperDayHeaderProps } from './paper-day-header'

export { PaperSessionHeader } from './paper-session-header'
export type { PaperSessionHeaderProps } from './paper-session-header'

export { PaperTargetBanner } from './paper-target-banner'
export type {
  PaperTargetBannerProps,
  PaperTargetBannerSource,
} from './paper-target-banner'

export { PaperContactFrame } from './paper-contact-frame'
export type {
  PaperContactFrameEntry,
  PaperContactFrameProps,
} from './paper-contact-frame'

export { PaperListRow } from './paper-list-row'
export type { PaperListRowEntry, PaperListRowProps } from './paper-list-row'

export { PaperDomainStack } from './paper-domain-stack'
export type {
  PaperDomainStackEntry,
  PaperDomainStackProps,
} from './paper-domain-stack'

export { PaperViewToggle } from './paper-view-toggle'
export type {
  PaperViewToggleOption,
  PaperViewToggleProps,
} from './paper-view-toggle'

export { PaperDayNavControl } from './paper-day-nav-control'
export type {
  PaperDayNavControlCopy,
  PaperDayNavControlProps,
} from './paper-day-nav-control'

export { PaperCalendarPopover } from './paper-calendar-popover'
export type {
  PaperCalendarPopoverBounds,
  PaperCalendarPopoverCopy,
  PaperCalendarPopoverProps,
} from './paper-calendar-popover'

export { PaperYearRail } from './paper-year-rail'
export type { PaperYearRailProps } from './paper-year-rail'
export { pickYearJumpIso } from './paper-year-rail-helpers'

export { PaperContactSheet } from './paper-contact-sheet'
export type {
  PaperContactSheetCopy,
  PaperContactSheetDayNav,
  PaperContactSheetProps,
  PaperContactSheetTarget,
  PaperContactSheetYearRail,
  PaperViewMode,
} from './paper-contact-sheet'

export { PaperDetailPanel } from './paper-detail-panel'
export type {
  PaperDetailPanelCopy,
  PaperDetailPanelEntry,
  PaperDetailPanelLookFurtherCounts,
  PaperDetailPanelProps,
  PaperDetailPanelTitleVersion,
  PaperDetailPanelVisitHistoryRow,
} from './paper-detail-panel'

export { PaperSearchHero } from './paper-search-hero'
export type {
  PaperSearchHeroCopy,
  PaperSearchHeroFilter,
  PaperSearchHeroProps,
  PaperSearchMode,
} from './paper-search-hero'

export { PaperSearchResult } from './paper-search-result'
export type {
  PaperSearchResultEntry,
  PaperSearchResultProps,
} from './paper-search-result'

export { PaperSearchEmpty } from './paper-search-empty'
export type {
  PaperSearchEmptyCopy,
  PaperSearchEmptyProps,
  PaperSearchRecent,
  PaperSearchSuggestion,
} from './paper-search-empty'

export { PaperKpiStrip } from './paper-kpi-strip'
export type { PaperKpiCell, PaperKpiStripProps } from './paper-kpi-strip'

export { PaperDomainRankList } from './paper-domain-rank'
export type {
  PaperDomainRankListProps,
  PaperDomainRankRow,
} from './paper-domain-rank'

export { PaperThreadList } from './paper-thread-list'
export type {
  PaperThreadListProps,
  PaperThreadRow,
  PaperThreadTone,
} from './paper-thread-list'
