/**
 * @file import.ts
 * @description Owns import workflow, batch review, and repair copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `import` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so import copy changes stay separate from workflow state machines.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `import` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const importNamespaceCatalog = {
  en: {
    archiveNotInitialized: 'Archive not set up yet',
    archiveNotInitializedBody:
      'Set up your archive first, then come back to import history data.',
    trustTitle: 'Import with review',
    trustBody:
      'PathKeep only imports dedicated Chrome history payloads in this pass. Everything else is either ignored on purpose or held for review.',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: 'Import from a Google data export',
    browserMethodTitle: 'Browser Direct',
    browserMethodBody: 'Import from a local browser database',
    goToSetup: 'Go to setup',
    takeoutPreparationHint:
      'Download your data from Google Takeout first. You can import the zip file directly or extract it first.',
    takeoutScopeTitle: 'Chrome-first scope',
    takeoutScopeBody:
      'This redesign only imports dedicated Chrome history payloads. It does not silently treat broader Google activity as browser history.',
    takeoutScopeImportable:
      'Imports Chrome history files such as BrowserHistory.json, History.json, and localized variants like Verlauf.json.',
    takeoutScopeIgnored:
      'Leaves typed URL companions, session companions, index files, and unrelated Google products out of canonical history.',
    takeoutScopeReview:
      'Flags Chrome-related My Activity files and other history-like files for manual review instead of guessing.',
    takeoutGuideTitle: 'Get the right Takeout export',
    takeoutGuideBody:
      'PathKeep does not import every Chrome-related Takeout file. Check the export before you scan it here.',
    takeoutGuideStepOne:
      'In Google Takeout, choose the Chrome export that includes a dedicated history JSON payload.',
    takeoutGuideStepTwo:
      'Before importing, open the zip or extracted folder and confirm it contains Chrome/BrowserHistory.json, Chrome/History.json, or a localized equivalent such as Chrome/Verlauf.json.',
    takeoutGuideStepThree:
      'If the export only contains My Activity files such as My Activity/Chrome/MyActivity.json, 我的活動/Chrome/我的活動.json, or any .html activity file, PathKeep will not import it in this build.',
    takeoutGuideSupportedExample:
      'Supported today: dedicated Chrome history payloads only.',
    takeoutGuideUnsupportedExample:
      'Not supported in this build: My Activity JSON, My Activity HTML, and unrelated Google exports.',
    browserPreparationHint:
      "Find your browser's History file. It's usually in the browser's profile folder. Close the browser first for best results.",
    stepUpload: 'Upload',
    stepScan: 'Scan',
    stepPreview: 'Preview',
    stepConfirm: 'Confirm',
    stepImport: 'Import',
    selectTitle: 'Step 1: Select Source',
    takeoutSelectBody:
      'Provide the path to your Google Takeout export (zip or folder).',
    browserSelectBody: 'Provide the path to a browser History database file.',
    sourcePath: 'SOURCE PATH',
    takeoutPathPlaceholder: '/path/to/takeout.zip',
    browserPathPlaceholder: '/path/to/History',
    scanSource: 'Scan source →',
    scanningTitle: 'Step 2: Scanning...',
    scanningBody: 'Inspecting source files for recognized history formats.',
    previewTitle: 'Step 3: Preview Import',
    previewBody: 'Review what will be imported before confirming.',
    recordsFound: 'Records Found',
    timeRange: 'Time Range',
    importableFiles: 'Importable Files',
    reviewNeededFiles: 'Needs Review',
    duplicates: 'Duplicates',
    newRecords: 'New Records',
    detectedFiles: 'DETECTED FILES',
    quarantinedFiles: 'QUARANTINED FILES',
    detectedLocaleLabel: 'Detected layout',
    timeRangeLabel: 'Time range',
    ignoredFilesInline:
      '{count} file(s) are known but intentionally ignored in this pass.',
    groupWillImportTitle: 'Will import',
    groupWillImportBody:
      'These files map to the dedicated Chrome history payloads PathKeep imports today.',
    groupIgnoredTitle: 'Known but ignored',
    groupIgnoredBody:
      'These files were recognized, but PathKeep keeps them out of canonical history in this Chrome-first pass.',
    groupNeedsReviewTitle: 'Needs review',
    groupNeedsReviewBody:
      'These files look history-related enough that PathKeep will not guess. Review them before treating them as browser history.',
    groupParseErrorTitle: 'Parse errors',
    groupParseErrorBody:
      'These files matched a supported family, but parsing failed and needs attention before you trust the import result.',
    kindJsonl: 'JSONL history file',
    kindBrowserHistory: 'Chrome history payload',
    kindTypedUrl: 'Typed URL companion',
    kindSession: 'Session companion',
    kindTakeoutIndex: 'Takeout index file',
    kindChromeActivity: 'Chrome My Activity file',
    kindChromeSupportingFile: 'Chrome supporting file',
    kindHistoryLikeFile: 'History-like file',
    kindOutsideScope: 'Outside current scope',
    reasonChromeHistoryJson:
      'Dedicated Chrome history export. PathKeep will import visits from this file.',
    reasonJsonlHistoryFixture:
      'Imported through the legacy JSONL compatibility path. Useful for existing fixtures and manual exports, but not a standard Google Takeout history file.',
    reasonSourceEvidenceOnly:
      'Used as supporting evidence only. PathKeep keeps it out of canonical visit history.',
    reasonTakeoutIndex:
      'Export manifest only. Helpful for review, but it does not contain history rows to import.',
    reasonChromeActivityOutsideScope:
      'Chrome-related My Activity is broader than browser history. PathKeep leaves it out until that contract is designed explicitly.',
    reasonChromeMyActivityJson:
      'This is a Chrome My Activity JSON export. PathKeep currently imports dedicated Chrome history payloads instead.',
    reasonChromeMyActivityHtml:
      'This is a Chrome My Activity HTML export. PathKeep does not import HTML activity files in this build.',
    reasonActivityOutsideScope:
      'This is a Google activity export, not a dedicated Chrome history payload.',
    reasonOutsideChromeScope:
      'This file is outside the current Chrome-first Takeout import scope.',
    reasonChromeSupportingFile:
      'Recognized as a Chrome export companion, but not as a browser history payload.',
    reasonUnrecognizedHistoryFile:
      'Looks history-related, but PathKeep does not have a safe import rule for it yet.',
    reasonParseError:
      'Parsing failed. Review the file before relying on this import result.',
    localeEnglish: 'English',
    localeGerman: 'German',
    localeChineseSimplified: 'Simplified Chinese',
    localeChineseTraditional: 'Traditional Chinese',
    localeMixed: 'Mixed',
    localeUnknown: 'Unknown',
    rangeUnavailable: 'No preview range yet',
    fileDispositionLabel: 'File disposition',
    fileRecordsLabel: '{count} records',
    backAction: '← Back',
    confirmImport: 'Confirm import →',
    importingTitle: 'Step 4: Importing...',
    importingProgressDetail: '{records} records · {files} files',
    importProgressPrepareDetail:
      'Checking {files} import payloads before writing archive rows.',
    importProgressImportDetail: 'Processing {current} / {total}: {source}',
    importProgressFinalizeDetail:
      'Refreshing keyword recall and import-review metadata.',
    importProgressCompleteDetail:
      'Import review is ready. Follow-up rebuild work continues in Background Jobs.',
    importingBody: 'Writing records to the archive. This may take a moment.',
    completeTitle: 'Step 5: Import Complete',
    completeBody: 'Records have been written to the archive.',
    imported: 'Imported',
    duplicatesSkipped: 'Duplicates Skipped',
    importAnother: 'Import another',
    workflowLabel: 'How import works',
    workflowCollapsedHint:
      'The flow below should already be enough in most cases. Expand this only when you want to inspect the full trust model.',
    showWorkflow: 'Show steps',
    hideWorkflow: 'Hide steps',
    workflowPreviewTitle: '1. Preview',
    workflowPreviewSummary:
      'See what was found, how many records are new, and what was quarantined — before anything is written.',
    workflowPreviewReason:
      'So you know exactly what will be imported and what was skipped.',
    workflowManualTitle: '2. Manual check',
    workflowManualSummary:
      'Optionally inspect the source files yourself outside of PathKeep.',
    workflowManualReason:
      'For peace of mind that the files are what you expect.',
    workflowExecuteTitle: '3. Import',
    workflowExecuteSummary: 'Write the recognized records to your archive.',
    workflowExecuteReason:
      'Only happens after you confirm. Creates a new import batch you can undo later.',
    workflowVerifyTitle: '4. Verify',
    workflowVerifySummary:
      'Check the results and undo controls right after import.',
    workflowVerifyReason:
      'So you can immediately undo if something looks wrong.',
    workflowFinishTitle: '5. Done',
    workflowFinishSummary:
      'Your import is complete and fully undoable from the batch list below.',
    workflowFinishReason:
      "Every import can be reverted. You're always in control.",
    manualLocateStep: 'Find the export file or extracted folder.',
    manualInspectStep:
      'Check the files against the list of recognized formats.',
    manualContinueStep:
      "Come back here when you're satisfied with what you see.",
    chooseTakeoutFile: 'Choose Takeout file',
    chooseTakeoutFolder: 'Choose Takeout folder',
    chooseHistoryFile: 'Choose History file',
    filePickerUnavailable:
      'The desktop file picker is unavailable right now. You can still paste a path manually.',
    detectedBrowserProfiles: 'DETECTED BROWSER PROFILES',
    detectedBrowserProfilesCount: '{count} ready',
    browserProfileReady: 'Ready',
    noDetectedBrowserProfilesTitle: 'No browser profile was detected',
    noDetectedBrowserProfilesBody:
      'Use the file picker, or expand manual path entry as a fallback.',
    showManualPath: 'Enter path manually',
    hideManualPath: 'Hide manual path',
    selectedSource: 'SELECTED SOURCE',
    recentBatches: 'Recent imports',
    recentBatchesBody: 'Every import can be reviewed, undone, or restored.',
    noImportBatches: 'No imports yet.',
    selectedBatch: 'Selected import',
    selectedBatchBody:
      'Select an import to see its contents and undo controls.',
    previewRows: 'Sample rows',
    noPreviewRows: 'No sample rows available yet.',
    candidateRows: 'Found',
    importedRows: 'Imported',
    duplicateRows: 'Duplicates',
    visibleRows: 'Visible',
    auditPath: 'Audit log',
    revertBatch: 'Undo import',
    restoreBatch: 'Restore import',
    revertConfirm:
      'Undo this import? The records will be hidden from your archive but the audit trail is preserved.',
    restoreConfirm:
      'Restore this undone import? The records will be visible in your archive again.',
    healthReport: 'HEALTH CHECK',
    healthReportBody:
      'Run a health check after importing, undoing, or restoring to make sure everything is consistent.',
    noHealthChecks: 'No health checks run yet.',
    confirmSummaryTitle: 'Import Summary',
    confirmSummaryBody:
      'The following data will be written to your archive. This action creates a new import batch that you can undo later.',
    confirmSummaryNewRecords: 'New records to import',
    confirmSummaryDuplicates: 'Duplicates to skip',
    confirmSummaryFiles: 'Source files',
    confirmSummaryReview: 'Files needing review',
    confirmSummaryIgnored: 'Known ignored files',
    noImportableFilesNotice:
      'Nothing in this selection matches the current Chrome-first import scope yet. Review the grouped scan results before proceeding.',
    takeoutMismatchDetectedTitle:
      'This export is not the Takeout format PathKeep imports today',
    takeoutMismatchJsonBody:
      'PathKeep detected a Chrome My Activity JSON export. That data is broader than the dedicated Chrome history payloads this importer expects, so nothing will be imported from it in this build.',
    takeoutMismatchHtmlBody:
      'PathKeep detected a Chrome My Activity HTML export. This importer does not read HTML activity files. Re-export and confirm the archive contains a dedicated Chrome history JSON payload before you scan again.',
    runHealthCheckAction: 'Run health check',
    repairDescription:
      'Attempt to fix inconsistencies found by the health check. This clears stale derived data, repairs visibility links, and rebuilds audit records.',
    healthCheckName: 'Check',
    healthCheckMessage: 'Details',
    batchIdLabel: 'Batch #{id}',
    repairSummary:
      'Cleaned up {derivedRows} derived rows, fixed {visibilityRows} visibility links, and rebuilt {importAudits} audit records.',
  },
  'zh-CN': {
    archiveNotInitialized: '还没有设置存档',
    archiveNotInitializedBody: '请先完成初始设置，然后回来导入历史数据。',
    trustTitle: '安全的导入流程',
    trustBody:
      '这次只正式导入专门的 Chrome 历史导出。其他 Takeout 文件会被明确标成忽略或待复核，而不是偷偷混进浏览记录。',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: '从 Google 数据导出导入',
    browserMethodTitle: '浏览器直接导入',
    browserMethodBody: '从本地浏览器数据库导入',
    goToSetup: '前往设置',
    takeoutPreparationHint:
      '请先从 Google Takeout 下载数据。可以直接导入 zip 文件，也可以先解压。',
    takeoutScopeTitle: '当前范围：只做 Chrome 历史',
    takeoutScopeBody:
      '这一版只导入专门的 Chrome 历史 payload，不会把更宽泛的 Google 活动误当成浏览历史。',
    takeoutScopeImportable:
      '会导入 BrowserHistory.json、History.json，以及像 Verlauf.json 这样的本地化 Chrome 历史文件。',
    takeoutScopeIgnored:
      'Typed URL、Session、Takeout 索引页和其他 Google 产品导出会保留为说明信息或直接忽略，不写入浏览历史。',
    takeoutScopeReview:
      '像 Chrome 相关的 My Activity 这类边界不清的文件会标成待复核，而不是直接猜测导入。',
    takeoutGuideTitle: '先确认导出类型',
    takeoutGuideBody:
      'PathKeep 不会导入所有和 Chrome 相关的 Takeout 文件。扫描前先确认导出内容是不是当前支持的那一类。',
    takeoutGuideStepOne:
      '在 Google Takeout 里，选择会产出专门历史 JSON payload 的 Chrome 导出。',
    takeoutGuideStepTwo:
      '导入前先打开 zip 或解压后的文件夹，确认里面有 Chrome/BrowserHistory.json、Chrome/History.json，或像 Chrome/Verlauf.json 这样的本地化等价文件。',
    takeoutGuideStepThree:
      '如果你看到的只有 My Activity/Chrome/MyActivity.json、我的活動/Chrome/我的活動.json，或任何 .html 活动文件，这一版 PathKeep 都不会导入。',
    takeoutGuideSupportedExample: '当前支持：专门的 Chrome 历史 payload。',
    takeoutGuideUnsupportedExample:
      '当前不支持：My Activity JSON、My Activity HTML，以及其他 Google 产品导出。',
    browserPreparationHint:
      '找到浏览器的 History 文件，通常在浏览器的个人资料文件夹中。建议先关闭浏览器再操作。',
    stepUpload: '上传',
    stepScan: '扫描',
    stepPreview: '预览',
    stepConfirm: '确认',
    stepImport: '导入',
    selectTitle: '第 1 步：选择来源',
    takeoutSelectBody: '选择 Google Takeout 导出文件（zip 或文件夹）。',
    browserSelectBody: '选择浏览器的 History 数据库文件。',
    sourcePath: '文件路径',
    takeoutPathPlaceholder: '/path/to/takeout.zip',
    browserPathPlaceholder: '/path/to/History',
    scanSource: '开始扫描 →',
    scanningTitle: '第 2 步：扫描中…',
    scanningBody: '正在识别文件中的历史记录。',
    previewTitle: '第 3 步：预览',
    previewBody: '确认前先看看会导入哪些内容。',
    recordsFound: '找到的记录',
    timeRange: '时间范围',
    importableFiles: '可导入文件',
    reviewNeededFiles: '待复核文件',
    duplicates: '重复',
    newRecords: '新记录',
    detectedFiles: '识别到的文件',
    quarantinedFiles: '被隔离的文件',
    detectedLocaleLabel: '检测到的导出布局',
    timeRangeLabel: '时间范围',
    ignoredFilesInline: '这次有 {count} 个文件属于已知但刻意忽略的范围。',
    groupWillImportTitle: '会导入',
    groupWillImportBody: '这些文件属于当前正式支持的 Chrome 历史 payload。',
    groupIgnoredTitle: '已知但忽略',
    groupIgnoredBody:
      '这些文件我们认得出来，但在当前 Chrome-first 方案里不会写入浏览历史。',
    groupNeedsReviewTitle: '需要人工复核',
    groupNeedsReviewBody:
      '这些文件看起来和历史记录有关，但 PathKeep 现在不会猜测处理方式。',
    groupParseErrorTitle: '解析失败',
    groupParseErrorBody:
      '这些文件命中了支持的家族，但解析失败，需要先处理再信任导入结果。',
    kindJsonl: 'JSONL 历史文件',
    kindBrowserHistory: 'Chrome 历史 payload',
    kindTypedUrl: 'Typed URL 辅助文件',
    kindSession: 'Session 辅助文件',
    kindTakeoutIndex: 'Takeout 索引页',
    kindChromeActivity: 'Chrome My Activity 文件',
    kindChromeSupportingFile: 'Chrome 辅助导出文件',
    kindHistoryLikeFile: '疑似历史文件',
    kindOutsideScope: '当前范围外',
    reasonChromeHistoryJson:
      '这是专门的 Chrome 历史导出，PathKeep 会从中导入访问记录。',
    reasonJsonlHistoryFixture:
      '这是旧版 JSONL 兼容导入路径，适合现有夹具或手工导出，但不是标准的 Google Takeout 历史文件。',
    reasonSourceEvidenceOnly:
      '只作为辅助证据保留，不会写入规范化的浏览访问历史。',
    reasonTakeoutIndex:
      '这是导出清单页，可用于核对，但不包含可导入的历史记录。',
    reasonChromeActivityOutsideScope:
      'Chrome 相关 My Activity 的范围比浏览历史更宽，这一版先不直接导入。',
    reasonChromeMyActivityJson:
      '这是 Chrome My Activity JSON 导出，不是当前 importer 需要的专门 Chrome 历史 payload。',
    reasonChromeMyActivityHtml:
      '这是 Chrome My Activity HTML 导出，这一版不会导入 HTML 活动文件。',
    reasonActivityOutsideScope:
      '这是 Google 活动导出，不是专门的 Chrome 历史 payload。',
    reasonOutsideChromeScope: '这个文件不在当前 Chrome-first 的导入范围内。',
    reasonChromeSupportingFile:
      '这是 Chrome 的辅助导出文件，但不是可直接导入的浏览历史 payload。',
    reasonUnrecognizedHistoryFile:
      '看起来像历史相关文件，但 PathKeep 还没有安全的导入规则。',
    reasonParseError: '解析失败，请先检查文件，再决定是否继续信任这次导入。',
    localeEnglish: '英文',
    localeGerman: '德文',
    localeChineseSimplified: '简体中文',
    localeChineseTraditional: '繁体中文',
    localeMixed: '混合',
    localeUnknown: '未知',
    rangeUnavailable: '还没有可显示的时间范围',
    fileDispositionLabel: '文件处理方式',
    fileRecordsLabel: '{count} 条记录',
    backAction: '← 返回',
    confirmImport: '确认导入 →',
    importingTitle: '第 4 步：导入中…',
    importingProgressDetail: '{records} 条记录 · {files} 个文件',
    importProgressPrepareDetail: '正在检查 {files} 个待导入文件。',
    importProgressImportDetail: '正在处理 {current} / {total}：{source}',
    importProgressFinalizeDetail: '正在刷新关键词检索和批次复核信息。',
    importProgressCompleteDetail: '导入复核已就绪，后续重建会在后台继续进行。',
    importingBody: '正在写入记录，请稍候。',
    completeTitle: '第 5 步：完成',
    completeBody: '记录已写入存档。',
    imported: '已导入',
    duplicatesSkipped: '跳过重复',
    importAnother: '继续导入',
    workflowLabel: '导入流程说明',
    workflowCollapsedHint:
      '下面的操作流程通常已经够用了。只有想检查完整信任边界时，再展开这部分说明。',
    showWorkflow: '展开说明',
    hideWorkflow: '收起说明',
    workflowPreviewTitle: '1. 预览',
    workflowPreviewSummary: '查看识别到的文件、预计导入的记录和被隔离的文件。',
    workflowPreviewReason: '确认导入范围和重复情况，被隔离的文件不会被写入。',
    workflowManualTitle: '2. 手动检查',
    workflowManualSummary: '你也可以在应用外自己检查源文件。',
    workflowManualReason: '确认文件内容是你想要导入的。',
    workflowExecuteTitle: '3. 执行导入',
    workflowExecuteSummary: '把识别到的记录写入存档。',
    workflowExecuteReason: '只有在确认预览后才会写入。每次导入都可以撤销。',
    workflowVerifyTitle: '4. 验证',
    workflowVerifySummary: '导入后检查结果和撤销选项。',
    workflowVerifyReason: '如果发现问题，可以立即撤销。',
    workflowFinishTitle: '5. 完成',
    workflowFinishSummary: '导入完成，随时可以在下方的导入记录中撤销。',
    workflowFinishReason: '每次导入都可以随时撤销，你始终掌控自己的数据。',
    manualLocateStep: '找到导出文件或解压后的文件夹。',
    manualInspectStep: '对照识别列表检查文件内容。',
    manualContinueStep: '确认没问题后回来继续。',
    chooseTakeoutFile: '选择 Takeout 文件',
    chooseTakeoutFolder: '选择 Takeout 文件夹',
    chooseHistoryFile: '选择 History 文件',
    filePickerUnavailable: '暂时无法打开文件选择器，你仍然可以手动粘贴路径。',
    detectedBrowserProfiles: '检测到的浏览器配置',
    detectedBrowserProfilesCount: '已就绪 {count} 个',
    browserProfileReady: '可导入',
    noDetectedBrowserProfilesTitle: '没有检测到可直接导入的浏览器配置',
    noDetectedBrowserProfilesBody:
      '可以改用文件选择器，或展开手动路径作为兜底。',
    showManualPath: '手动输入路径',
    hideManualPath: '隐藏手动路径',
    selectedSource: '当前来源',
    recentBatches: '最近导入',
    recentBatchesBody: '每次导入都可以查看、撤销或恢复。',
    noImportBatches: '还没有导入记录。',
    selectedBatch: '当前选中的导入',
    selectedBatchBody: '选择一个导入批次查看内容和撤销选项。',
    previewRows: '示例记录',
    noPreviewRows: '暂无示例记录。',
    candidateRows: '找到',
    importedRows: '已导入',
    duplicateRows: '重复',
    visibleRows: '当前可见',
    auditPath: '审计日志',
    revertBatch: '撤销导入',
    restoreBatch: '恢复导入',
    revertConfirm: '撤销这次导入？记录会从存档中隐藏，但审计日志仍保留。',
    restoreConfirm: '恢复这次已撤销的导入？记录将重新在存档中可见。',
    healthReport: '健康检查',
    healthReportBody: '导入、撤销或恢复后运行健康检查，确认数据一致性。',
    noHealthChecks: '还没有运行过健康检查。',
    confirmSummaryTitle: '导入摘要',
    confirmSummaryBody:
      '以下数据将写入存档。此操作会创建一个可撤销的导入批次。',
    confirmSummaryNewRecords: '将导入的新记录',
    confirmSummaryDuplicates: '将跳过的重复记录',
    confirmSummaryFiles: '源文件',
    confirmSummaryReview: '待复核文件',
    confirmSummaryIgnored: '已知忽略文件',
    noImportableFilesNotice:
      '这次选择的内容里还没有命中当前 Chrome-first 的正式导入范围。请先查看上面的分组扫描结果。',
    takeoutMismatchDetectedTitle:
      '这份导出不是当前 PathKeep 会导入的 Takeout 格式',
    takeoutMismatchJsonBody:
      'PathKeep 检测到的是 Chrome My Activity JSON。它的范围比当前 importer 预期的专门 Chrome 历史 payload 更宽，所以这份数据在这一版不会被导入。',
    takeoutMismatchHtmlBody:
      'PathKeep 检测到的是 Chrome My Activity HTML。当前 importer 不读取 HTML 活动文件。请重新导出，并在再次扫描前确认压缩包里有专门的 Chrome 历史 JSON payload。',
    runHealthCheckAction: '运行健康检查',
    repairDescription:
      '尝试修复健康检查发现的不一致问题。会清理过时的分析数据、修复引用链接，并重建审计记录。',
    healthCheckName: '检查项',
    healthCheckMessage: '详情',
    batchIdLabel: '批次 #{id}',
    repairSummary:
      '修复了 {derivedRows} 条分析数据、{visibilityRows} 条引用链接，并重建了 {importAudits} 条审计记录。',
  },
  'zh-TW': {
    archiveNotInitialized: '還沒有設定封存',
    archiveNotInitializedBody: '請先完成初始設定，再回來匯入歷史資料。',
    trustTitle: '安全的匯入流程',
    trustBody:
      '這次只正式匯入專門的 Chrome 歷史匯出。其他 Takeout 檔案會被明確標成忽略或待複核，不會偷偷混進瀏覽紀錄。',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: '從 Google 資料匯出檔匯入',
    browserMethodTitle: '瀏覽器直接匯入',
    browserMethodBody: '從本機瀏覽器資料庫匯入',
    goToSetup: '前往設定',
    takeoutPreparationHint:
      '請先從 Google Takeout 下載資料。可以直接匯入 zip 檔案，也可以先解壓縮。',
    takeoutScopeTitle: '目前範圍：只做 Chrome 歷史',
    takeoutScopeBody:
      '這一版只匯入專門的 Chrome 歷史 payload，不會把更寬泛的 Google 活動誤當成瀏覽歷史。',
    takeoutScopeImportable:
      '會匯入 BrowserHistory.json、History.json，以及像 Verlauf.json 這種在地化 Chrome 歷史檔。',
    takeoutScopeIgnored:
      'Typed URL、Session、Takeout 索引頁與其他 Google 產品匯出只保留為說明資訊或直接忽略，不寫入瀏覽歷史。',
    takeoutScopeReview:
      '像 Chrome 相關的 My Activity 這類邊界不清的檔案，會標成待複核，而不是直接猜測匯入。',
    takeoutGuideTitle: '先確認匯出類型',
    takeoutGuideBody:
      'PathKeep 不會匯入所有和 Chrome 相關的 Takeout 檔案。掃描前先確認匯出內容是不是目前支援的那一類。',
    takeoutGuideStepOne:
      '在 Google Takeout 裡，選擇會產出專門歷史 JSON payload 的 Chrome 匯出。',
    takeoutGuideStepTwo:
      '匯入前先打開 zip 或解壓後的資料夾，確認裡面有 Chrome/BrowserHistory.json、Chrome/History.json，或像 Chrome/Verlauf.json 這樣的在地化等價檔案。',
    takeoutGuideStepThree:
      '如果你看到的只有 My Activity/Chrome/MyActivity.json、我的活動/Chrome/我的活動.json，或任何 .html 活動檔，這一版 PathKeep 都不會匯入。',
    takeoutGuideSupportedExample: '目前支援：專門的 Chrome 歷史 payload。',
    takeoutGuideUnsupportedExample:
      '目前不支援：My Activity JSON、My Activity HTML，以及其他 Google 產品匯出。',
    browserPreparationHint:
      '找到瀏覽器的 History 檔案，通常在瀏覽器的設定檔資料夾中。建議先關閉瀏覽器再操作。',
    stepUpload: '上傳',
    stepScan: '掃描',
    stepPreview: '預覽',
    stepConfirm: '確認',
    stepImport: '匯入',
    selectTitle: '第 1 步：選擇來源',
    takeoutSelectBody: '選擇 Google Takeout 匯出檔（zip 或資料夾）。',
    browserSelectBody: '選擇瀏覽器的 History 資料庫檔案。',
    sourcePath: '檔案路徑',
    takeoutPathPlaceholder: '/path/to/takeout.zip',
    browserPathPlaceholder: '/path/to/History',
    scanSource: '開始掃描 →',
    scanningTitle: '第 2 步：掃描中…',
    scanningBody: '正在辨識檔案中的歷史紀錄。',
    previewTitle: '第 3 步：預覽',
    previewBody: '確認前先看看會匯入哪些內容。',
    recordsFound: '找到的紀錄',
    timeRange: '時間範圍',
    importableFiles: '可匯入檔案',
    reviewNeededFiles: '待複核檔案',
    duplicates: '重複',
    newRecords: '新紀錄',
    detectedFiles: '辨識到的檔案',
    quarantinedFiles: '被隔離的檔案',
    detectedLocaleLabel: '偵測到的匯出版型',
    timeRangeLabel: '時間範圍',
    ignoredFilesInline: '這次有 {count} 個檔案屬於已知但刻意忽略的範圍。',
    groupWillImportTitle: '會匯入',
    groupWillImportBody: '這些檔案屬於目前正式支援的 Chrome 歷史 payload。',
    groupIgnoredTitle: '已知但忽略',
    groupIgnoredBody:
      '這些檔案我們認得出來，但在目前 Chrome-first 方案裡不會寫入瀏覽歷史。',
    groupNeedsReviewTitle: '需要人工複核',
    groupNeedsReviewBody:
      '這些檔案看起來和歷史紀錄有關，但 PathKeep 現在不會猜測處理方式。',
    groupParseErrorTitle: '解析失敗',
    groupParseErrorBody:
      '這些檔案命中了支援的家族，但解析失敗，需要先處理再信任匯入結果。',
    kindJsonl: 'JSONL 歷史檔案',
    kindBrowserHistory: 'Chrome 歷史 payload',
    kindTypedUrl: 'Typed URL 輔助檔',
    kindSession: 'Session 輔助檔',
    kindTakeoutIndex: 'Takeout 索引頁',
    kindChromeActivity: 'Chrome My Activity 檔案',
    kindChromeSupportingFile: 'Chrome 輔助匯出檔',
    kindHistoryLikeFile: '疑似歷史檔案',
    kindOutsideScope: '目前範圍外',
    reasonChromeHistoryJson:
      '這是專門的 Chrome 歷史匯出，PathKeep 會從中匯入造訪紀錄。',
    reasonJsonlHistoryFixture:
      '這是舊版 JSONL 相容匯入路徑，適合現有測試夾具或手動匯出，但不是標準的 Google Takeout 歷史檔。',
    reasonSourceEvidenceOnly:
      '只作為輔助證據保留，不會寫入規範化的瀏覽造訪歷史。',
    reasonTakeoutIndex:
      '這是匯出清單頁，可用來核對，但不包含可匯入的歷史紀錄。',
    reasonChromeActivityOutsideScope:
      'Chrome 相關 My Activity 的範圍比瀏覽歷史更寬，這一版先不直接匯入。',
    reasonChromeMyActivityJson:
      '這是 Chrome My Activity JSON 匯出，不是目前 importer 需要的專門 Chrome 歷史 payload。',
    reasonChromeMyActivityHtml:
      '這是 Chrome My Activity HTML 匯出，這一版不會匯入 HTML 活動檔。',
    reasonActivityOutsideScope:
      '這是 Google 活動匯出，不是專門的 Chrome 歷史 payload。',
    reasonOutsideChromeScope: '這個檔案不在目前 Chrome-first 的匯入範圍內。',
    reasonChromeSupportingFile:
      '這是 Chrome 的輔助匯出檔，但不是可直接匯入的瀏覽歷史 payload。',
    reasonUnrecognizedHistoryFile:
      '看起來像歷史相關檔案，但 PathKeep 還沒有安全的匯入規則。',
    reasonParseError: '解析失敗，請先檢查檔案，再決定是否繼續信任這次匯入。',
    localeEnglish: '英文',
    localeGerman: '德文',
    localeChineseSimplified: '簡體中文',
    localeChineseTraditional: '繁體中文',
    localeMixed: '混合',
    localeUnknown: '未知',
    rangeUnavailable: '還沒有可顯示的時間範圍',
    fileDispositionLabel: '檔案處理方式',
    fileRecordsLabel: '{count} 筆紀錄',
    backAction: '← 返回',
    confirmImport: '確認匯入 →',
    importingTitle: '第 4 步：匯入中…',
    importingProgressDetail: '{records} 筆紀錄 · {files} 個檔案',
    importProgressPrepareDetail: '正在檢查 {files} 個待匯入檔案。',
    importProgressImportDetail: '正在處理 {current} / {total}：{source}',
    importProgressFinalizeDetail: '正在刷新關鍵字檢索和批次複核資訊。',
    importProgressCompleteDetail:
      '匯入複核已就緒，後續重建會在背景工作中繼續進行。',
    importingBody: '正在寫入紀錄，請稍候。',
    completeTitle: '第 5 步：完成',
    completeBody: '紀錄已寫入封存。',
    imported: '已匯入',
    duplicatesSkipped: '略過重複',
    importAnother: '繼續匯入',
    workflowLabel: '匯入流程說明',
    workflowCollapsedHint:
      '下面的操作流程通常已經夠用了。只有想檢查完整信任邊界時，再展開這段說明。',
    showWorkflow: '展開說明',
    hideWorkflow: '收起說明',
    workflowPreviewTitle: '1. 預覽',
    workflowPreviewSummary: '查看辨識到的檔案、預計匯入的紀錄和被隔離的檔案。',
    workflowPreviewReason: '確認匯入範圍和重複情形，被隔離的檔案不會被寫入。',
    workflowManualTitle: '2. 手動檢查',
    workflowManualSummary: '你也可以在應用程式外自行檢查原始檔案。',
    workflowManualReason: '確認檔案內容是你想匯入的。',
    workflowExecuteTitle: '3. 執行匯入',
    workflowExecuteSummary: '將辨識到的紀錄寫入封存。',
    workflowExecuteReason: '只有在確認預覽後才會寫入。每次匯入都可以復原。',
    workflowVerifyTitle: '4. 驗證',
    workflowVerifySummary: '匯入後檢查結果和復原選項。',
    workflowVerifyReason: '發現問題時可以立即復原。',
    workflowFinishTitle: '5. 完成',
    workflowFinishSummary: '匯入完成，隨時可以在下方的匯入紀錄中復原。',
    workflowFinishReason: '每次匯入都可以隨時復原，你始終掌控自己的資料。',
    manualLocateStep: '找到匯出檔或解壓後的資料夾。',
    manualInspectStep: '對照辨識清單檢查檔案內容。',
    manualContinueStep: '確認沒問題後回來繼續。',
    chooseTakeoutFile: '選擇 Takeout 檔案',
    chooseTakeoutFolder: '選擇 Takeout 資料夾',
    chooseHistoryFile: '選擇 History 檔案',
    filePickerUnavailable: '暫時無法開啟檔案選擇器，你仍然可以手動貼上路徑。',
    detectedBrowserProfiles: '偵測到的瀏覽器設定檔',
    detectedBrowserProfilesCount: '已就緒 {count} 個',
    browserProfileReady: '可匯入',
    noDetectedBrowserProfilesTitle: '沒有偵測到可直接匯入的瀏覽器設定檔',
    noDetectedBrowserProfilesBody:
      '可以改用檔案選擇器，或展開手動路徑作為兜底。',
    showManualPath: '手動輸入路徑',
    hideManualPath: '隱藏手動路徑',
    selectedSource: '目前來源',
    recentBatches: '最近匯入',
    recentBatchesBody: '每次匯入都可以查看、復原或恢復。',
    noImportBatches: '還沒有匯入紀錄。',
    selectedBatch: '目前選中的匯入',
    selectedBatchBody: '選擇一個匯入批次查看內容和復原選項。',
    previewRows: '範例紀錄',
    noPreviewRows: '暫無範例紀錄。',
    candidateRows: '找到',
    importedRows: '已匯入',
    duplicateRows: '重複',
    visibleRows: '目前可見',
    auditPath: '稽核日誌',
    revertBatch: '復原匯入',
    restoreBatch: '恢復匯入',
    revertConfirm: '復原這次匯入？紀錄會從封存中隱藏，但稽核日誌仍保留。',
    restoreConfirm: '恢復這次已復原的匯入？紀錄將重新在封存中顯示。',
    healthReport: '健康檢查',
    healthReportBody: '匯入、復原或恢復後執行健康檢查，確認資料一致性。',
    noHealthChecks: '還沒有執行過健康檢查。',
    confirmSummaryTitle: '匯入摘要',
    confirmSummaryBody:
      '以下資料將寫入封存。此操作會建立一個可復原的匯入批次。',
    confirmSummaryNewRecords: '將匯入的新紀錄',
    confirmSummaryDuplicates: '將略過的重複紀錄',
    confirmSummaryFiles: '來源檔案',
    confirmSummaryReview: '待複核檔案',
    confirmSummaryIgnored: '已知忽略檔案',
    noImportableFilesNotice:
      '這次選到的內容裡還沒有命中目前 Chrome-first 的正式匯入範圍。請先查看上面的分組掃描結果。',
    takeoutMismatchDetectedTitle:
      '這份匯出不是目前 PathKeep 會匯入的 Takeout 格式',
    takeoutMismatchJsonBody:
      'PathKeep 偵測到的是 Chrome My Activity JSON。它的範圍比目前 importer 預期的專門 Chrome 歷史 payload 更寬，所以這份資料在這一版不會被匯入。',
    takeoutMismatchHtmlBody:
      'PathKeep 偵測到的是 Chrome My Activity HTML。現在的 importer 不讀取 HTML 活動檔。請重新匯出，並在再次掃描前確認壓縮包裡有專門的 Chrome 歷史 JSON payload。',
    runHealthCheckAction: '執行健康檢查',
    repairDescription:
      '嘗試修復健康檢查發現的不一致問題。會清理過時的分析資料、修復參照連結，並重建稽核紀錄。',
    healthCheckName: '檢查項目',
    healthCheckMessage: '詳情',
    batchIdLabel: '批次 #{id}',
    repairSummary:
      '修復了 {derivedRows} 筆分析資料、{visibilityRows} 筆參照連結，並重建了 {importAudits} 筆稽核紀錄。',
  },
} as const
