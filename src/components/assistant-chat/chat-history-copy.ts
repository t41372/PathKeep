/**
 * @file chat-history-copy.ts
 * @description Projects the `assistant` namespace `history*` keys into the chat-history explorer
 *              copy bundle, including locale-aware relative-time and pluralized message-count
 *              builders.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Build the `ChatHistoryCopy` bundle the explorer consumes from one translator.
 * - Compute a compact relative-time label ("Just now" / "5 min ago" / "3 h ago" / "2 d ago")
 *   from an ISO timestamp, against an injectable "now" so it is deterministic in tests.
 *
 * ## Not responsible for
 * - Owning the strings (the catalog is the source of truth) or fetching conversations.
 */

import type { AssistantTranslator } from './assistant-chat-copy'
import type { ChatHistoryCopy } from './chat-history-explorer'

const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS
const WEEK_MS = 7 * DAY_MS
/** Approximate month/year boundaries; row recency does not need calendar precision. */
const MONTH_MS = 30 * DAY_MS
const YEAR_MS = 365 * DAY_MS

/**
 * Formats a compact relative-time label for a conversation's `updatedAt`.
 *
 * Buckets to "just now" under a minute, minutes under an hour, hours under a day, days under a
 * week, weeks under a month, then months — so an old chat reads "3 mo ago" instead of "412 d ago".
 * Beyond a year it switches to a localized short date (`Intl.DateTimeFormat`) so the label stays
 * readable for genuinely old conversations. An unparseable or future timestamp falls back to
 * "just now" so a clock skew never shows a negative age. `now` and `locale` are injectable so the
 * formatting is deterministic under test.
 */
export function formatRelativeTime(
  t: AssistantTranslator,
  iso: string,
  now: number = Date.now(),
  locale = 'en',
): string {
  const then = Date.parse(iso)
  if (Number.isNaN(then)) return t('historyRelativeJustNow')
  const elapsed = now - then
  if (elapsed < MINUTE_MS) return t('historyRelativeJustNow')
  if (elapsed < HOUR_MS) {
    return t('historyRelativeMinutes', {
      count: Math.floor(elapsed / MINUTE_MS),
    })
  }
  if (elapsed < DAY_MS) {
    return t('historyRelativeHours', { count: Math.floor(elapsed / HOUR_MS) })
  }
  if (elapsed < WEEK_MS) {
    return t('historyRelativeDays', { count: Math.floor(elapsed / DAY_MS) })
  }
  if (elapsed < MONTH_MS) {
    return t('historyRelativeWeeks', { count: Math.floor(elapsed / WEEK_MS) })
  }
  if (elapsed < YEAR_MS) {
    return t('historyRelativeMonths', {
      count: Math.floor(elapsed / MONTH_MS),
    })
  }
  // Beyond a year, a localized short date is clearer than a huge "N mo ago" count.
  return new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(then)
}

/**
 * Builds the explorer copy bundle. `now` is forwarded into the relative-time builder so callers
 * (and tests) can pin the clock; production passes the default `Date.now()`. `locale` is the active
 * UI language so the beyond-a-year short-date fallback localizes correctly.
 */
export function buildChatHistoryCopy(
  t: AssistantTranslator,
  now: number = Date.now(),
  locale = 'en',
): ChatHistoryCopy {
  return {
    title: t('historyTitle'),
    openLabel: t('historyOpen'),
    closeLabel: t('historyClose'),
    newChat: t('historyNewChat'),
    loading: t('historyLoading'),
    errorTitle: t('historyErrorTitle'),
    errorBody: t('historyErrorBody'),
    retry: t('historyRetry'),
    emptyTitle: t('historyEmptyTitle'),
    emptyBody: t('historyEmptyBody'),
    emptyCta: t('historyEmptyCta'),
    activeBadge: t('historyActiveBadge'),
    deleteAction: t('historyDeleteAction'),
    deleteConfirmTitle: t('historyDeleteConfirmTitle'),
    deleteConfirm: t('historyDeleteConfirm'),
    deleteCancel: t('historyDeleteCancel'),
    renameAction: t('historyRenameAction'),
    renameLabel: t('historyRenameLabel'),
    renameSave: t('historyRenameSave'),
    renameCancel: t('historyRenameCancel'),
    openConversationLabel: (title: string) =>
      t('historyConversationLabel', { title }),
    deleteConfirmBody: (title: string) =>
      t('historyDeleteConfirmBody', { title }),
    messageCount: (count: number) =>
      count === 1
        ? t('historyMessageCountOne')
        : t('historyMessageCount', { count }),
    relativeTime: (updatedAt: string) =>
      formatRelativeTime(t, updatedAt, now, locale),
  }
}
