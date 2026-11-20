/**
 * Typed client for the Agent space's Insights (`#/agent/insights`): the
 * viewer's saved choices and the streamed report with its sections. The
 * projects the filter offers come from `agent-projects.ts`.
 *
 * Responsible for: command names, request envelopes, the wire shapes and
 * the preference event's name. Not responsible for: opening the stream
 * (`src/lib/ipc/agent-read-stream.ts`), keeping sections current or
 * any copy (`src/features/agent-insights/`).
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * Insights, Agent Projects). Times are UTC epoch milliseconds, durations
 * milliseconds, money USD, days `YYYY-MM-DD` in the zone the `started`
 * frame names. A `null` amount means nothing was recorded, which is not
 * zero; a measured zero stays `0`.
 */
import type { ChatRange, TitleSource, TokenKinds } from './agent-chats'
import type { ProjectKind } from './agent-projects'
import type { AgentToolId } from './data-kinds'
import { call } from './shared'

export type { TokenKinds } from './agent-chats'

/** The six ranges of Insights, Records and project Insights (`agent-history.md` §1.2). */
export type InsightsRange = ChatRange

/** How the "≈" beside an hours figure reads (§1.12). */
export type EquivalentReading = 'nonstop' | 'workdays'

/** Saved for the viewer; the reader follows the two cost switches too. */
export interface InsightsPreferences {
  range: InsightsRange
  equivalent: EquivalentReading
  showCost: boolean
  estimateCost: boolean
}

export type InsightsPreferencePatch = Partial<InsightsPreferences>

/** Sent with every saved change, with the whole object. */
export const AGENT_INSIGHTS_PREFERENCES_CHANGED_EVENT =
  'pathkeep://agent-insights-preferences-changed'

/**
 * One report request. `report` is the KPI row and the wall (they follow
 * the range), `calendar` the year (it does not); a range change asks for
 * the report, a year step for the calendar, anything else for both.
 */
export interface InsightsQuery {
  range?: InsightsRange
  part?: 'report' | 'calendar' | 'both'
  year?: number
  harnesses?: AgentToolId[]
  projectId?: string
}

/** A project as a card shows it: its current name and classification. */
export interface InsightsProjectChip {
  id: string
  /** Empty for the loose folders, which have a name of their own. */
  name: string
  kind: ProjectKind
  /** Every folder of it is gone from the disk. */
  gone: boolean
}

export interface InsightsCost {
  /** Recorded, plus estimates while they are on. */
  totalUsd: number | null
  recordedUsd: number | null
  /** `null` while estimates are off; `availableEstimatedUsd` still has it. */
  estimatedUsd: number | null
  availableEstimatedUsd: number | null
  recordedConversations: number
  estimatedConversations: number
  /** Conversations with both: the two counts above overlap by this much. */
  bothConversations: number
  /** No usage record, or a model with no price: unknown, not 0. */
  unpricedConversations: number
  includeEstimates: boolean
  priceTableVersion: string
}

export interface InsightsOverview {
  /** Clock time with any agent working; `null` when no turn has a time. */
  agentBusyMs: number | null
  /** Each conversation's in-range work added up. */
  agentSummedMs: number | null
  concurrency: number | null
  presentMs: number | null
  timedHumanMessages: number
  undatedHumanMessages: number
  conversationCount: number
  subagentCount: number
  projectCount: number
  recordedDurationConversations: number
  derivedDurationConversations: number
  mixedDurationConversations: number
  unknownDurationConversations: number
  tokens: TokenKinds | null
  conversationsWithUsage: number
  averageTokensPerMeasuredConversation: number | null
  /** Most conversations without usage predate this tool's first record of it. */
  usageCoverageNote: { harness: AgentToolId; firstUsageAt: number } | null
  /** Absent while "Show cost" is off. */
  cost?: InsightsCost
  /** Conversations saved, the trash included. */
  savedRoots: number
  /** Not in the trash. */
  visibleRoots: number
  /** Not in the trash and matching the tool and project filters, at any time. */
  scopeRoots: number
}

export type BucketUnit = 'hour' | 'day' | 'week' | 'month'

/** The early, unfiltered work chart; `series` replaces it. */
export interface InsightsWork {
  unit: BucketUnit
  buckets: {
    bucketStart: number
    bucketEnd: number
    busyMs: number | null
    summedMs: number | null
  }[]
}

export interface InsightsBucket {
  /** Half open: `[bucketStart, bucketEnd)`. */
  bucketStart: number
  bucketEnd: number
  busyMs: number | null
  summedMs: number | null
  presentMs: number
  tokens: TokenKinds | null
  /** A tool missing here, or `null`, has no usage record in the bucket. */
  tokensByHarness: Partial<Record<AgentToolId, TokenKinds | null>>
  /** Aligned with `modelKeys`. */
  tokensByModel: (TokenKinds | null)[]
  otherModelTokens: TokenKinds | null
  conversationCount: number
  conversationsWithUsage: number
}

export interface InsightsSeries {
  unit: BucketUnit
  /** The five models with the most tokens; an empty key is unidentified. */
  modelKeys: string[]
  otherModelCount: number
  buckets: InsightsBucket[]
}

/** Root counts with how many of them have the measure. */
export interface InsightsGroup {
  conversations: number
  summedMs: number | null
  tokens: TokenKinds | null
  conversationsWithUsage: number
  conversationsWithDuration: number
}

export interface InsightsHarnessRow extends InsightsGroup {
  harness: AgentToolId
}

export interface InsightsProjectRow extends InsightsGroup {
  project: InsightsProjectChip | null
  harnesses: AgentToolId[]
}

export interface InsightsLongest {
  conversationId: string
  title: string | null
  titleSource: TitleSource
  harness: AgentToolId
  project: InsightsProjectChip | null
  turnsInRange: number
  agentMsInRange: number
}

export interface InsightsBreakdowns {
  /** `[hour][weekday]`, Monday is 0: messages the person sent, local time. */
  hourOfWeek: number[][]
  byHarness: InsightsHarnessRow[]
  topProjects: InsightsProjectRow[]
  models: {
    /** `null`: the record does not say which model. */
    model: string | null
    tokens: TokenKinds | null
    /** Conversations whose main model this is. */
    primaryConversations: number
  }[]
  longest: InsightsLongest[]
  linesAdded: number | null
  linesRemoved: number | null
  conversationsWithEdits: number
  editsWithUnknownLineCounts: number
  toolCalls: {
    total: number
    top: { harness: AgentToolId; name: string; count: number }[]
  }
}

export interface InsightsCalendar {
  year: number
  currentYear: number
  firstYearWithData: number | null
  timeZone: string
  /** Every day of the year; future days and unknown work are `null`. */
  days: {
    day: string
    busyMs: number | null
    active: boolean
    future: boolean
  }[]
  /** 1–12. */
  months: { month: number; busyMs: number | null; future: boolean }[]
  busyMs: number | null
  elapsedDays: number
  activeDays: number
  timedActiveDays: number
  averageBusyMs: number | null
  longestStreak: number
  streakStart: string | null
  streakEnd: string | null
  busiestDay: string | null
  busiestMonth: number | null
}

interface FrameBase {
  streamId: string
  sequence: string
}

/** One frame of a report. `finished`, `cancelled` and `error` end it. */
export type InsightsFrame = FrameBase &
  (
    | {
        type: 'started'
        payload: {
          range: InsightsRange
          year: number
          today: string
          timeZone: string
          firstBackupRunning: boolean
          viewToken: string
        }
      }
    | { type: 'progress'; payload: { phase: string; done: number } }
    | { type: 'work'; payload: InsightsWork }
    | { type: 'overview'; payload: InsightsOverview }
    | { type: 'series'; payload: InsightsSeries }
    | { type: 'breakdowns'; payload: InsightsBreakdowns }
    | { type: 'calendar'; payload: InsightsCalendar }
    | { type: 'finished'; payload: { newerViewAvailable: boolean } }
    | { type: 'cancelled'; payload: { code: string } }
    | { type: 'error'; payload: { code: string } }
  )

export const agentInsightsClient = {
  preferences: () =>
    call<InsightsPreferences>('get_agent_insights_preferences', {
      request: {},
    }),
  /** The complete preferences after the change. */
  setPreferences: (request: InsightsPreferencePatch) =>
    call<InsightsPreferences>('set_agent_insights_preferences', { request }),
}
