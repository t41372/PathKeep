/**
 * @file archive-upgrade.ts
 * @description Owns all copy for the one-time "Upgrading your archive…" first-run screen.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `archiveUpgrade` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Cover every visible state: preparing, per-phase working, finishing, the Intelligence
 *   informational line, error + retry, and all aria-labels.
 * - Provide the stable `phase.*` keys the shell maps each `ArchiveUpgradePhase` enum value to.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so upgrade wording stays isolated from archive logic.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `archiveUpgrade` namespace payload for the shipped locales.
 *
 * All keys must appear in all three locales with no raw English in zh-CN or zh-TW.
 * The copy is intentionally calm and reassuring — this screen appears once after an
 * update and must never read as an error.
 */
export const archiveUpgradeNamespaceCatalog = {
  en: {
    // Screen header
    eyebrow: 'PathKeep',
    title: 'Upgrading your archive',
    body: 'PathKeep updated and is bringing your archive up to date. This one-time step runs only after this update, and can take a few minutes on a large archive.',

    // Three reassurances — shown persistently below the body.
    oneTimeNote: 'This happens only once, right after an update.',
    dataSafeNote:
      'Your history is safe — nothing is deleted or changed, only reindexed.',
    resumableNote:
      'You can quit at any time; the upgrade resumes where it left off.',

    // Phase names — the shell maps each ArchiveUpgradePhase enum value here.
    phase: {
      schemaMigration: 'Preparing the archive',
      registrableDomainBackfill: 'Grouping sites by domain',
      searchReprojection: 'Rebuilding search',
      intelligence: 'Refreshing insights',
      finalizing: 'Finishing up',
    },

    // Progress affordances
    stepIndicator: 'Step {current} of {total}',
    countProgress: '{processed} of {total}',
    preparing: 'Getting things ready…',
    working: 'Working…',
    finishing: 'Almost done…',

    // Non-streamed Intelligence phase — an informational line, not a bar.
    intelligenceInfo:
      'Insights will refresh quietly in the background once your archive is ready.',

    // Error state
    errorTitle: 'The upgrade could not finish',
    errorDetail: 'Detail: {detail}',
    retry: 'Try again',
    retryAria: 'Retry the archive upgrade',

    // aria-labels
    progressAria: 'Archive upgrade progress',
    statusAria: 'Archive upgrade status',
  },
  'zh-CN': {
    eyebrow: 'PathKeep',
    title: '正在升级你的存档',
    body: 'PathKeep 已更新，正在把你的存档更新到最新格式。这一步只在本次更新后运行一次，存档较大时可能需要几分钟。',

    oneTimeNote: '这只会在更新后进行一次。',
    dataSafeNote: '你的历史记录是安全的——不会删除或更改，只是重新建立索引。',
    resumableNote: '你可以随时退出，升级会从中断处继续。',

    phase: {
      schemaMigration: '准备存档',
      registrableDomainBackfill: '按域名归类站点',
      searchReprojection: '重建搜索',
      intelligence: '刷新洞察',
      finalizing: '即将完成',
    },

    stepIndicator: '第 {current} 步，共 {total} 步',
    countProgress: '{processed} / {total}',
    preparing: '正在准备…',
    working: '处理中…',
    finishing: '就快好了…',

    intelligenceInfo: '存档就绪后，洞察会在后台悄悄刷新。',

    errorTitle: '升级未能完成',
    errorDetail: '详情：{detail}',
    retry: '重试',
    retryAria: '重新尝试存档升级',

    progressAria: '存档升级进度',
    statusAria: '存档升级状态',
  },
  'zh-TW': {
    eyebrow: 'PathKeep',
    title: '正在升級你的封存',
    body: 'PathKeep 已更新，正在把你的封存更新到最新格式。這一步只在本次更新後執行一次，封存較大時可能需要幾分鐘。',

    oneTimeNote: '這只會在更新後進行一次。',
    dataSafeNote: '你的歷史記錄是安全的——不會刪除或更動，只是重新建立索引。',
    resumableNote: '你可以隨時退出，升級會從中斷處繼續。',

    phase: {
      schemaMigration: '準備封存',
      registrableDomainBackfill: '依網域歸類站點',
      searchReprojection: '重建搜尋',
      intelligence: '刷新洞察',
      finalizing: '即將完成',
    },

    stepIndicator: '第 {current} 步，共 {total} 步',
    countProgress: '{processed} / {total}',
    preparing: '正在準備…',
    working: '處理中…',
    finishing: '就快好了…',

    intelligenceInfo: '封存就緒後，洞察會在背景悄悄刷新。',

    errorTitle: '升級未能完成',
    errorDetail: '詳情：{detail}',
    retry: '重試',
    retryAria: '重新嘗試封存升級',

    progressAria: '封存升級進度',
    statusAria: '封存升級狀態',
  },
} as const
