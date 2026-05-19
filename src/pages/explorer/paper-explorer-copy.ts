/**
 * Build a `PaperExplorerCopy` object from the Explorer namespace translator.
 *
 * Routes use this to feed translated strings into PaperExplorerView without
 * hand-assembling every i18n key. Keeping the mapping in one place means the
 * upcoming Search / Intelligence / Assistant pages can rely on the same
 * pattern (catalog → typed copy bundle) without copy-pasting key names.
 *
 * ## Responsibilities
 * - Read each `explorer.paperBrowse.*` key once and return a typed object
 *   shaped exactly like PaperExplorerCopy.
 *
 * ## Not responsible for
 * - Resolving the active language; caller passes the namespace translator
 *   already pinned to one language.
 */

import type { PaperExplorerCopy } from './paper-view'

export type ExplorerTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export function buildPaperExplorerCopy(
  t: ExplorerTranslator,
): PaperExplorerCopy {
  return {
    contactSheet: {
      view: t('paperBrowse.contactSheetView'),
      cards: t('paperBrowse.contactSheetCards'),
      list: t('paperBrowse.contactSheetList'),
      dayMeta: t('paperBrowse.contactSheetDayMeta'),
      dayIndex: t('paperBrowse.contactSheetDayIndex'),
      clearTarget: t('paperBrowse.contactSheetClearTarget'),
      expandStack: t('paperBrowse.contactSheetExpandStack'),
      moreInStack: t('paperBrowse.contactSheetMoreInStack'),
      pagesLabel: t('paperBrowse.contactSheetPagesLabel'),
      empty: t('paperBrowse.contactSheetEmpty'),
    },
    dayNav: {
      prev: t('paperBrowse.dayNavPrev'),
      next: t('paperBrowse.dayNavNext'),
      today: t('paperBrowse.dayNavToday'),
      openCalendar: t('paperBrowse.dayNavOpenCalendar'),
    },
    relative: {
      today: t('paperBrowse.relativeToday'),
      yesterday: t('paperBrowse.relativeYesterday'),
      daysAgo: t('paperBrowse.relativeDaysAgo'),
      weeksAgo: t('paperBrowse.relativeWeeksAgo'),
      monthsAgo: t('paperBrowse.relativeMonthsAgo'),
      yearsAgo: t('paperBrowse.relativeYearsAgo'),
    },
    calendar: {
      prevMonth: t('paperBrowse.calendarPrevMonth'),
      nextMonth: t('paperBrowse.calendarNextMonth'),
      months: [
        t('paperBrowse.calendarMonthJanuary'),
        t('paperBrowse.calendarMonthFebruary'),
        t('paperBrowse.calendarMonthMarch'),
        t('paperBrowse.calendarMonthApril'),
        t('paperBrowse.calendarMonthMay'),
        t('paperBrowse.calendarMonthJune'),
        t('paperBrowse.calendarMonthJuly'),
        t('paperBrowse.calendarMonthAugust'),
        t('paperBrowse.calendarMonthSeptember'),
        t('paperBrowse.calendarMonthOctober'),
        t('paperBrowse.calendarMonthNovember'),
        t('paperBrowse.calendarMonthDecember'),
      ],
      dowLabels: [
        t('paperBrowse.calendarDowMonday'),
        t('paperBrowse.calendarDowTuesday'),
        t('paperBrowse.calendarDowWednesday'),
        t('paperBrowse.calendarDowThursday'),
        t('paperBrowse.calendarDowFriday'),
        t('paperBrowse.calendarDowSaturday'),
        t('paperBrowse.calendarDowSunday'),
      ],
      today: t('paperBrowse.calendarToday'),
      oneYearAgo: t('paperBrowse.calendarOneYearAgo'),
      pagesArchived: t('paperBrowse.calendarPagesArchived'),
      monthSummary: t('paperBrowse.calendarMonthSummary'),
      boundsMeta: t('paperBrowse.calendarBoundsMeta'),
    },
    yearRailTitle: t('paperBrowse.yearRailTitle'),
  }
}
