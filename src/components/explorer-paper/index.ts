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

export { PaperSessionGap } from './paper-session-gap'
export type { PaperSessionGapProps } from './paper-session-gap'

export { PaperFilterStrip } from './paper-filter-strip'
export type {
  PaperFilterChip,
  PaperFilterStripCopy,
  PaperFilterStripFormState,
  PaperFilterStripOption,
  PaperFilterStripProps,
} from './paper-filter-strip'

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

export { PaperContactSheet } from './paper-contact-sheet'
export type {
  PaperContactSheetCopy,
  PaperContactSheetDayNav,
  PaperContactSheetProps,
  PaperContactSheetTarget,
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

export { PaperAdvancedSearchHelp } from './paper-advanced-search-help'
export type {
  PaperAdvancedSearchHelpCopy,
  PaperAdvancedSearchHelpProps,
} from './paper-advanced-search-help'

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

export { PaperTopicTimeline } from './paper-topic-timeline'
export type {
  PaperTopicBar,
  PaperTopicRow,
  PaperTopicTimelineProps,
  PaperTopicTrend,
} from './paper-topic-timeline'

export { PaperRefindShelf } from './paper-refind-shelf'
export type {
  PaperRefindItem,
  PaperRefindShelfProps,
} from './paper-refind-shelf'

export { PaperAssistantMessage } from './paper-assistant-message'
export type {
  PaperAssistantEvidence,
  PaperAssistantMessageProps,
  PaperAssistantRole,
} from './paper-assistant-message'

export { PaperAssistantComposer } from './paper-assistant-composer'
export type {
  PaperAssistantComposerCopy,
  PaperAssistantComposerProps,
} from './paper-assistant-composer'

export { PaperAssistantGreeting } from './paper-assistant-greeting'
export type {
  PaperAssistantGreetingProps,
  PaperAssistantGreetingPrompt,
} from './paper-assistant-greeting'

export { PaperImportStepper } from './paper-import-stepper'
export type { PaperImportStepperProps } from './paper-import-stepper'

export { PaperImportMethodCard } from './paper-import-method-card'
export type { PaperImportMethodCardProps } from './paper-import-method-card'

export { PaperChainBlock } from './paper-chain-block'
export type { PaperChainBlockProps } from './paper-chain-block'

export { PaperStorageBar } from './paper-storage-bar'
export type {
  PaperStorageBarProps,
  PaperStorageBarTone,
} from './paper-storage-bar'

export { PaperIntelligenceView } from './paper-intelligence-view'
export type {
  PaperIntelligenceViewCopy,
  PaperIntelligenceViewProps,
} from './paper-intelligence-view'

export { PaperSearchView } from './paper-search-view'
export type {
  PaperSearchViewCopy,
  PaperSearchViewDayGroup,
  PaperSearchViewProps,
} from './paper-search-view'

export { PaperAssistantView } from './paper-assistant-view'
export type {
  PaperAssistantMessageDescriptor,
  PaperAssistantViewCopy,
  PaperAssistantViewProps,
} from './paper-assistant-view'

export { PaperImportView } from './paper-import-view'
export type {
  PaperImportMethod,
  PaperImportViewProps,
} from './paper-import-view'

export { PaperAuditView } from './paper-audit-view'
export type {
  PaperAuditChainEntry,
  PaperAuditViewCopy,
  PaperAuditViewProps,
} from './paper-audit-view'
