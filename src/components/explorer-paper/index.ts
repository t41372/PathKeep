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
