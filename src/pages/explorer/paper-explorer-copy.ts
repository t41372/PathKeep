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

import type {
  PaperDetailPanelCopy,
  PaperEnrichedContentCopy,
  PaperIntelligenceViewCopy,
  PaperSearchViewCopy,
  PaperStarredViewCopy,
} from '@/components/explorer-paper'
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
      pagesLabel: t('paperBrowse.contactSheetPagesLabel'),
      empty: t('paperBrowse.contactSheetEmpty'),
      sessionGapLabel: t('paperBrowse.contactSheetSessionGap'),
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
      dialogLabel: t('paperBrowse.calendarDialogLabel'),
    },
    target: {
      fromOnThisDay: t('paperBrowse.targetFromOnThisDay'),
      fromSearch: t('paperBrowse.targetFromSearch'),
      fromSearchWithQuery: t('paperBrowse.targetFromSearchWithQuery'),
      fromIntelligence: t('paperBrowse.targetFromIntelligence'),
      pagesArchived: t('paperBrowse.targetPagesArchived'),
      noArchive: t('paperBrowse.targetNoArchive'),
    },
    pagination: {
      older: t('paperBrowse.paginationOlder'),
      newer: t('paperBrowse.paginationNewer'),
      summary: t('paperBrowse.paginationSummary'),
      summaryPending: t('paperBrowse.paginationSummaryPending'),
      pageSizeLabel: t('paperBrowse.paginationPageSizeLabel'),
    },
    infiniteScroll: {
      loadingMore: t('paperBrowse.infiniteLoadingMore'),
      endOfArchive: t('paperBrowse.infiniteEndOfArchive'),
      loadedSummary: t('paperBrowse.infiniteLoadedSummary'),
      capReached: t('paperBrowse.infiniteCapReached'),
      error: t('paperBrowse.infiniteError'),
    },
    dayInsights: {
      topDomainsTitle: t('paperBrowse.dayInsightsTopDomains'),
      activityTitle: t('paperBrowse.dayInsightsActivity'),
      hourlyTitle: t('paperBrowse.dayInsightsHourly'),
      pagesLabel: t('paperBrowse.dayInsightsPages'),
      typedLabel: t('paperBrowse.dayInsightsTyped'),
      linksLabel: t('paperBrowse.dayInsightsLinks'),
      searchesLabel: t('paperBrowse.dayInsightsSearches'),
      sessionsTemplate: t('paperBrowse.dayInsightsSessionsTemplate'),
      domainsTemplate: t('paperBrowse.dayInsightsDomainsTemplate'),
      moreDetailsLabel: t('paperBrowse.dayInsightsMoreDetailsLabel'),
      firstVisitLabel: t('paperBrowse.dayInsightsFirstVisitLabel'),
      lastVisitLabel: t('paperBrowse.dayInsightsLastVisitLabel'),
      peakHourLabel: t('paperBrowse.dayInsightsPeakHourLabel'),
      longestSessionLabel: t('paperBrowse.dayInsightsLongestSessionLabel'),
      topUrlsTitle: t('paperBrowse.dayInsightsTopUrlsTitle'),
      visitsCountTemplate: t('paperBrowse.dayInsightsVisitsCountTemplate'),
    },
  }
}

export function buildPaperDetailPanelCopy(
  t: ExplorerTranslator,
): PaperDetailPanelCopy {
  return {
    recordEyebrow: t('paperBrowse.detailRecordEyebrow'),
    closeLabel: t('paperBrowse.detailClose'),
    openAction: t('paperBrowse.detailActionOpen'),
    copyAction: t('paperBrowse.detailActionCopy'),
    refindAction: t('paperBrowse.detailActionRefind'),
    exportAction: t('paperBrowse.detailActionExport'),
    starAction: t('star.starPageAria'),
    unstarAction: t('star.unstarPageAria'),
    starStatusStarred: t('star.statusStarred'),
    starStatusUnstarred: t('star.statusUnstarred'),
    starShortcutHint: t('star.shortcutHint'),
    provenanceHeading: t('paperBrowse.detailProvenanceHeading'),
    notesHeading: t('paperBrowse.detailNotesHeading'),
    tagsHeading: t('paperBrowse.detailTagsHeading'),
    lookFurtherHeading: t('paperBrowse.detailLookFurtherHeading'),
    firstVisitLabel: t('paperBrowse.detailFirstVisit'),
    lastVisitLabel: t('paperBrowse.detailLastVisit'),
    visitedLabel: t('paperBrowse.detailVisited'),
    totalVisitsLabel: t('paperBrowse.detailTotalVisits'),
    typedCountLabel: t('paperBrowse.detailTypedCount'),
    recentVisitsLabel: t('paperBrowse.detailRecentVisits'),
    sourceLabel: t('paperBrowse.detailSource'),
    transitionLabel: t('paperBrowse.detailTransition'),
    capturedInRunLabel: t('paperBrowse.detailCapturedInRun'),
    titleHistoryLabel: t('paperBrowse.detailTitleHistory'),
    notesPlaceholder: t('paperBrowse.detailNotesPlaceholder'),
    notesEmpty: t('paperBrowse.detailNotesEmpty'),
    notesSavedLocally: t('paperBrowse.detailNotesSavedLocally'),
    notesSaveError: t('paperBrowse.detailNotesSaveError'),
    notesCharSingular: t('paperBrowse.detailNotesCharSingular'),
    notesCharPlural: t('paperBrowse.detailNotesCharPlural'),
    tagInputPlaceholder: t('paperBrowse.detailTagInputPlaceholder'),
    tagRemoveAriaLabel: t('paperBrowse.detailTagRemoveAriaLabel'),
    pageLevelInsights: t('paperBrowse.detailLookPageInsights'),
    allOfDomain: t('paperBrowse.detailLookAllOfDomain'),
    threadLabel: t('paperBrowse.detailLookThread'),
    sessionLabel: t('paperBrowse.detailLookSession'),
    visitCountSuffix: t('paperBrowse.detailVisitCountSuffix'),
  }
}

export function buildPaperEnrichedContentCopy(
  t: ExplorerTranslator,
): PaperEnrichedContentCopy {
  return {
    heading: t('paperBrowse.detailEnrichedHeading'),
    loading: t('paperBrowse.detailEnrichedLoading'),
    empty: t('paperBrowse.detailEnrichedEmpty'),
    disabled: t('paperBrowse.detailEnrichedDisabled'),
    error: t('paperBrowse.detailEnrichedError'),
    fetchedAt: t('paperBrowse.detailEnrichedFetchedAt'),
    sourceGithub: t('paperBrowse.detailEnrichedSourceGithub'),
    sourceGeneric: t('paperBrowse.detailEnrichedSourceGeneric'),
    sourceUnknown: t('paperBrowse.detailEnrichedSourceUnknown'),
    topicsLabel: t('paperBrowse.detailEnrichedTopicsLabel'),
    statusEmpty: t('paperBrowse.detailEnrichedStatusEmpty'),
    statusBlocked: t('paperBrowse.detailEnrichedStatusBlocked'),
    statusError: t('paperBrowse.detailEnrichedStatusError'),
    statusLogin: t('paperBrowse.detailEnrichedStatusLogin'),
    statusUnsupported: t('paperBrowse.detailEnrichedStatusUnsupported'),
    statusRateLimited: t('paperBrowse.detailEnrichedStatusRateLimited'),
    fetchNowAction: t('paperBrowse.detailFetchNowAction'),
    fetchNowFetching: t('paperBrowse.detailFetchNowFetching'),
    fetchNowQueued: t('paperBrowse.detailFetchNowQueued'),
    fetchNowDisabledHint: t('paperBrowse.detailFetchNowDisabledHint'),
    fetchNowError: t('paperBrowse.detailFetchNowError'),
  }
}

export function buildPaperStarredViewCopy(
  t: ExplorerTranslator,
): PaperStarredViewCopy {
  return {
    eyebrow: t('star.hubEyebrow'),
    title: t('star.hubTitle'),
    description: t('star.hubDescription'),
    groupPages: t('star.hubGroupPages'),
    groupSources: t('star.hubGroupSources'),
    sortLabel: t('star.hubSortLabel'),
    sortRecent: t('star.hubSortRecent'),
    sortRevisited: t('star.hubSortRevisited'),
    loading: t('star.hubLoading'),
    emptyTitle: t('star.hubEmptyTitle'),
    emptyBody: t('star.hubEmptyBody'),
    emptyCta: t('star.hubEmptyCta'),
    visitCountTemplate: t('star.hubVisitCount'),
    starAction: t('star.starAction'),
    unstarAction: t('star.unstarAction'),
    statusStarred: t('star.statusStarred'),
    statusUnstarred: t('star.statusUnstarred'),
  }
}

export function buildPaperIntelligenceCopy(
  t: ExplorerTranslator,
  options: { topicsSummary?: PaperIntelligenceViewCopy['topicsSummary'] } = {},
): PaperIntelligenceViewCopy {
  return {
    topicsTitle: t('paperIntelligence.topicsTitle'),
    topicsRangeBadge: t('paperIntelligence.topicsRangeBadge'),
    topicsSummary:
      options.topicsSummary ?? t('paperIntelligence.topicsSummaryFallback'),
    domainsTitle: t('paperIntelligence.domainsTitle'),
    domainsBadge: t('paperIntelligence.domainsBadge'),
    sessionsTitle: t('paperIntelligence.sessionsTitle'),
    sessionsBadge: t('paperIntelligence.sessionsBadge'),
    threadsTitle: t('paperIntelligence.threadsTitle'),
    refindTitle: t('paperIntelligence.refindTitle'),
    refindBadge: t('paperIntelligence.refindBadge'),
    sessionPagesLabel: t('paperIntelligence.sessionPagesLabel'),
    threadPagesLabel: t('paperIntelligence.threadPagesLabel'),
  }
}

/**
 * Bundle PaperSearchView strings from the `explorer.paperSearchView.*` keys
 * into the nested copy shape PaperSearchView expects.
 *
 * Splitting hero / empty into sub-objects mirrors the component's prop
 * shape: PaperSearchView passes `copy.hero` to PaperSearchHero and
 * `copy.empty` to PaperSearchEmpty unchanged.
 */
export function buildPaperSearchViewCopy(
  t: ExplorerTranslator,
): PaperSearchViewCopy {
  return {
    hero: {
      prompt: t('paperSearchView.heroPrompt'),
      inputPlaceholder: t('paperSearchView.heroInputPlaceholder'),
      modesLabel: t('paperSearchView.heroModesLabel'),
      filtersLabel: t('paperSearchView.heroFiltersLabel'),
      modeKeyword: t('paperSearchView.heroModeKeyword'),
      modeRegex: t('paperSearchView.heroModeRegex'),
      modeSemantic: t('paperSearchView.heroModeSemantic'),
      modeHintKeyword: t('paperSearchView.heroModeHintKeyword'),
      modeHintRegex: t('paperSearchView.heroModeHintRegex'),
      modeHintSemantic: t('paperSearchView.heroModeHintSemantic'),
      addFilterDate: t('paperSearchView.heroAddFilterDate'),
      addFilterSource: t('paperSearchView.heroAddFilterSource'),
      addFilterDomain: t('paperSearchView.heroAddFilterDomain'),
      addFilterVisitCount: t('paperSearchView.heroAddFilterVisitCount'),
      addFilterTag: t('paperSearchView.heroAddFilterTag'),
      addFilterNote: t('paperSearchView.heroAddFilterNote'),
      removeChipLabel: t('paperSearchView.heroRemoveChipLabel'),
      advancedSyntaxHelp: {
        ariaLabel: t('advancedSearchHelpAria'),
        title: t('advancedSearchHelpTitle'),
        intro: t('advancedSearchHelpIntro'),
        siteExclude: t('advancedSearchHelpSiteExclude'),
        exactPhrase: t('advancedSearchHelpExactPhrase'),
        or: t('advancedSearchHelpOr'),
        field: t('advancedSearchHelpField'),
        fileDate: t('advancedSearchHelpFileDate'),
        tag: t('advancedSearchHelpTag'),
        note: t('advancedSearchHelpNote'),
        starred: t('star.facetIsStarred'),
        regexNote: t('advancedSearchHelpRegexNote'),
      },
    },
    empty: {
      tryAskingHeading: t('paperSearchView.emptyTryAskingHeading'),
      recentHeading: t('paperSearchView.emptyRecentHeading'),
      recentMeta: t('paperSearchView.emptyRecentMeta'),
      footer: t('paperSearchView.emptyFooter'),
    },
    resultsCount: t('paperSearchView.resultsCount'),
    resultsRange: t('paperSearchView.resultsRange'),
    pageSuffixSingular: t('paperSearchView.pageSuffixSingular'),
    pageSuffixPlural: t('paperSearchView.pageSuffixPlural'),
    noMatchesTitle: t('paperSearchView.noMatchesTitle'),
    noMatchesBody: t('paperSearchView.noMatchesBody'),
    seeInContextLabel: t('paperSearchView.seeInContextLabel'),
    dayCountTemplate: t('paperSearchView.dayCountTemplate'),
    enrichmentMatchLabel: t('paperSearchView.enrichmentMatchLabel'),
  }
}
