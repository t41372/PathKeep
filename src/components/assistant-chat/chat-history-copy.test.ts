/**
 * @file chat-history-copy.test.ts
 * @description Coverage for the chat-history copy builder and relative-time formatter, against the
 *              real `assistant` i18n namespace so the keys are proven to exist.
 */

import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import { buildChatHistoryCopy, formatRelativeTime } from './chat-history-copy'

const t = createNamespaceTranslator('en', 'assistant')

const NOW = Date.parse('2026-06-20T12:00:00Z')

describe('formatRelativeTime', () => {
  test('buckets elapsed time into just-now / minutes / hours / days', () => {
    expect(formatRelativeTime(t, '2026-06-20T11:59:30Z', NOW)).toBe('Just now')
    expect(formatRelativeTime(t, '2026-06-20T11:45:00Z', NOW)).toBe(
      '15 min ago',
    )
    expect(formatRelativeTime(t, '2026-06-20T09:00:00Z', NOW)).toBe('3 h ago')
    expect(formatRelativeTime(t, '2026-06-18T12:00:00Z', NOW)).toBe('2 d ago')
  })

  test('buckets older timestamps into weeks then months (no "412 d ago")', () => {
    // ~10 days → 1 week.
    expect(formatRelativeTime(t, '2026-06-10T12:00:00Z', NOW)).toBe('1 w ago')
    // ~3 weeks.
    expect(formatRelativeTime(t, '2026-05-30T12:00:00Z', NOW)).toBe('3 w ago')
    // ~3 months.
    expect(formatRelativeTime(t, '2026-03-20T12:00:00Z', NOW)).toBe('3 mo ago')
  })

  test('beyond a year switches to a localized short date', () => {
    // A timestamp over a year old is rendered as a calendar date (mid-day UTC keeps the year stable
    // across timezones), not "N mo ago".
    const label = formatRelativeTime(t, '2024-01-15T12:00:00Z', NOW, 'en')
    expect(label).not.toContain('ago')
    expect(label).toMatch(/2024/)
    // The locale parameter feeds Intl.DateTimeFormat (zh-TW renders its own form, still year 2024).
    const zh = formatRelativeTime(t, '2024-01-15T12:00:00Z', NOW, 'zh-TW')
    expect(zh).toMatch(/2024/)
  })

  test('falls back to just-now for unparseable or future timestamps', () => {
    expect(formatRelativeTime(t, 'not-a-date', NOW)).toBe('Just now')
    // A future timestamp (clock skew) yields a negative elapsed → just-now bucket.
    expect(formatRelativeTime(t, '2026-06-20T12:30:00Z', NOW)).toBe('Just now')
  })

  test('defaults now to Date.now when omitted', () => {
    // A timestamp far in the past is always beyond a year → a calendar date, never an "ago" bucket.
    const label = formatRelativeTime(t, '2000-01-01T00:00:00Z')
    expect(label).not.toContain('ago')
    expect(label).toMatch(/\d{4}/)
  })
})

describe('buildChatHistoryCopy', () => {
  test('projects every history key and singular/plural message counts', () => {
    const copy = buildChatHistoryCopy(t, NOW)
    expect(copy.title).toBe('Conversations')
    expect(copy.newChat).toBe('New chat')
    expect(copy.emptyTitle).toBe('No saved chats yet')
    expect(copy.deleteConfirm).toBe('Delete')
    expect(copy.openConversationLabel('My chat')).toBe(
      'Open conversation: My chat',
    )
    expect(copy.deleteConfirmBody('My chat')).toContain('My chat')
    expect(copy.messageCount(1)).toBe('1 message')
    expect(copy.messageCount(5)).toBe('5 messages')
    expect(copy.relativeTime('2026-06-20T11:45:00Z')).toBe('15 min ago')
    // New copy keys (empty CTA + rename affordance) are projected.
    expect(copy.emptyCta).toBe('Start a chat')
    expect(copy.renameAction).toBe('Rename conversation')
    expect(copy.renameSave).toBe('Save')
    expect(copy.renameCancel).toBe('Cancel')
    expect(copy.renameLabel).toBe('New conversation title')
  })

  test('forwards the locale into the beyond-a-year date fallback', () => {
    const copy = buildChatHistoryCopy(t, NOW, 'en')
    expect(copy.relativeTime('2024-01-15T12:00:00Z')).toMatch(/2024/)
  })

  test('defaults now to the real clock when omitted', () => {
    const copy = buildChatHistoryCopy(t)
    const label = copy.relativeTime('2000-01-01T00:00:00Z')
    expect(label).not.toContain('ago')
    expect(label).toMatch(/\d{4}/)
  })
})
