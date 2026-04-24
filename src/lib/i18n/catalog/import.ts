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
    trustTitle: 'Import browser history',
    trustBody:
      'PathKeep imports the dedicated Chrome history export in this flow. Other Takeout files stay out of browsing history unless a future format is explicitly supported.',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: 'Import from a Google data export',
    browserMethodTitle: 'Browser Direct',
    browserMethodBody: 'Import from a local browser database',
    goToSetup: 'Go to setup',
    takeoutPreparationHint:
      'Export Chrome in Google Takeout. You can choose the zip, the extracted folder, or the history JSON file itself here.',
    takeoutScopeTitle: 'Supported in this flow',
    takeoutScopeBody:
      'PathKeep imports the dedicated Chrome history JSON file. The filename can vary by language, but the payload starts with Browser History records.',
    takeoutScopeImportable:
      'Choose the zip, the extracted folder, or the history JSON file itself.',
    takeoutScopeIgnored:
      'My Activity JSON, My Activity HTML, typed URL, session, and other Google exports are not imported as browsing history.',
    takeoutScopeReview:
      'If a file looks related but ambiguous, PathKeep stops and asks you to review it.',
    takeoutGuideTitle: 'Get the right export',
    takeoutGuideBody: 'Run this quick check first.',
    takeoutGuideStepOne: 'In Google Takeout, export Chrome.',
    takeoutGuideStepTwo:
      'Open the zip or folder and confirm it contains one dedicated Chrome history JSON file.',
    takeoutGuideStepThree:
      'You can select the zip, the extracted folder, or that JSON file directly in PathKeep.',
    takeoutGuideSupportedExample:
      'Typical examples include BrowserHistory.json, History.json, Verlauf.json, and localized names with the same payload structure.',
    takeoutUnsupportedTitle: 'Not imported here',
    takeoutGuideUnsupportedExample:
      'Chrome My Activity JSON and HTML exports are not imported here.',
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
      'These files will become browsing history visits in your archive.',
    groupIgnoredTitle: 'Known but ignored',
    groupIgnoredBody:
      'These files belong to the export, but they are not imported as browsing history.',
    groupNeedsReviewTitle: 'Needs review',
    groupNeedsReviewBody:
      'These files look history-related, but PathKeep needs your review before treating them as browsing history.',
    groupParseErrorTitle: 'Parse errors',
    groupParseErrorBody:
      'These files matched a supported format, but parsing failed before the import could use them.',
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
      'This file uses the older JSONL history format. PathKeep can import it, but it is not the standard Google Takeout Chrome history export.',
    reasonSourceEvidenceOnly:
      'Kept as supporting evidence only. It does not create browsing history visits.',
    reasonTakeoutIndex:
      'Export manifest only. Helpful for review, but it does not contain history rows to import.',
    reasonChromeActivityOutsideScope:
      'Chrome My Activity covers more than browsing history, so it stays out of this import flow.',
    reasonChromeMyActivityJson:
      'This is a Chrome My Activity JSON export, not the dedicated browser-history JSON PathKeep imports in this build.',
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
    importProgressActiveLabel: 'File {current} of {total}',
    importProgressPrepareDetail:
      'Checking {files} import payloads before writing archive rows.',
    importProgressImportActiveDetail:
      'Writing file {current} of {total}: {source}',
    importProgressImportDetail: 'Finished file {current} of {total}: {source}',
    importProgressFinalizeDetail:
      'Updating search recall and import review data.',
    importProgressCompleteDetail:
      'Import review is ready. Follow-up rebuild work continues in Background Jobs.',
    importingBody: 'Writing records to the archive. This may take a moment.',
    completeTitle: 'Step 5: Import Complete',
    completeBody:
      'Records have been written to the archive. Review this import below.',
    imported: 'Imported',
    duplicatesSkipped: 'Duplicates Skipped',
    importAnother: 'Import another',
    workflowLabel: 'Why PathKeep previews first',
    workflowCollapsedHint:
      'Scan first. Review the result. Import only after you confirm.',
    showWorkflow: 'Show details',
    hideWorkflow: 'Hide details',
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
    browserProfileNeedsAccess: 'Access needed',
    safariFullDiskAccessHint:
      'Safari History.db may require Full Disk Access before PathKeep can stage it.',
    browserProfileUnreadable:
      'The history file is not readable right now. Check the file path and permissions.',
    noDetectedBrowserProfilesTitle: 'No browser profile was detected',
    noDetectedBrowserProfilesBody:
      'Use the file picker, or expand manual path entry as a fallback.',
    showManualPath: 'Enter path manually',
    hideManualPath: 'Hide manual path',
    selectedSource: 'SELECTED SOURCE',
    actionErrorTitle: 'Import could not continue',
    batchReviewTitle: 'Current import result',
    batchReviewBody:
      'PathKeep shows the latest imported batch here so you can verify it right away or undo it if needed.',
    historyToolsTitle: 'Import history and maintenance',
    historyToolsBody:
      'Open older batches, restore or undo them, and run a health check when you need it.',
    historyToolsCollapsedHint:
      'Older batches and repair tools stay here when you need them.',
    showHistoryTools: 'Show history',
    hideHistoryTools: 'Hide history',
    recentBatches: 'Recent imports',
    recentBatchesBody:
      'Pick an older batch when you want to review or undo it.',
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
      'PathKeep did not find a dedicated Chrome history JSON payload in this selection. Check that you exported browser history, not My Activity.',
    takeoutMismatchDetectedTitle:
      'This export is not the browser-history format PathKeep imports today',
    takeoutMismatchJsonBody:
      'PathKeep detected a Chrome My Activity JSON export. Re-export Chrome history, then scan the zip, folder, or dedicated history JSON again.',
    takeoutMismatchHtmlBody:
      'PathKeep detected a Chrome My Activity HTML export. Re-export Chrome history and make sure the archive contains a dedicated history JSON payload before you scan again.',
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
    trustTitle: '导入浏览历史',
    trustBody:
      '这个流程只导入专门的 Chrome 历史导出。其他 Takeout 文件不会写入浏览历史，除非后续明确支持。',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: '从 Google 数据导出导入',
    browserMethodTitle: '浏览器直接导入',
    browserMethodBody: '从本地浏览器数据库导入',
    goToSetup: '前往设置',
    takeoutPreparationHint:
      '请先在 Google Takeout 里导出 Chrome。这里可以直接选择 zip、解压后的文件夹，或历史 JSON 文件本身。',
    takeoutScopeTitle: '这个流程支持什么',
    takeoutScopeBody:
      'PathKeep 导入的是专门的 Chrome 历史 JSON。文件名可能因语言而不同，但内容会以 Browser History 记录开头。',
    takeoutScopeImportable:
      '可以直接选择 zip、解压后的文件夹，或历史 JSON 文件本身。',
    takeoutScopeIgnored:
      'My Activity JSON、My Activity HTML、Typed URL、Session 和其他 Google 产品导出都不会写入浏览历史。',
    takeoutScopeReview:
      '如果文件看起来相关但不够明确，PathKeep 会停下来让你复核。',
    takeoutGuideTitle: '先拿到正确的导出',
    takeoutGuideBody: '开始前先确认这三件事。',
    takeoutGuideStepOne: '在 Google Takeout 里导出 Chrome。',
    takeoutGuideStepTwo:
      '打开 zip 或文件夹，确认里面有一个专门的 Chrome 历史 JSON 文件。',
    takeoutGuideStepThree:
      '你可以在 PathKeep 里直接选择 zip、文件夹，或那个 JSON 文件本身。',
    takeoutGuideSupportedExample:
      '常见文件名包括 BrowserHistory.json、History.json、Verlauf.json，以及其他同结构的本地化名称。',
    takeoutUnsupportedTitle: '这里不会导入',
    takeoutGuideUnsupportedExample:
      'Chrome My Activity JSON 和 HTML 导出不会在这里导入。',
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
    groupWillImportBody: '这些文件会写入你的浏览历史存档。',
    groupIgnoredTitle: '已知但忽略',
    groupIgnoredBody: '这些文件属于导出的一部分，但不会作为浏览历史导入。',
    groupNeedsReviewTitle: '需要人工复核',
    groupNeedsReviewBody:
      '这些文件看起来和历史有关，但 PathKeep 需要你先确认后再决定。',
    groupParseErrorTitle: '解析失败',
    groupParseErrorBody: '这些文件命中了支持的格式，但在导入前解析失败了。',
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
      '这是较早的 JSONL 历史格式。PathKeep 可以导入，但它不是标准的 Google Takeout Chrome 历史导出。',
    reasonSourceEvidenceOnly: '只保留为辅助证据，不会生成浏览历史访问记录。',
    reasonTakeoutIndex:
      '这是导出清单页，可用于核对，但不包含可导入的历史记录。',
    reasonChromeActivityOutsideScope:
      'Chrome My Activity 覆盖的内容比浏览历史更宽，所以不会走这个导入流程。',
    reasonChromeMyActivityJson:
      '这是 Chrome My Activity JSON 导出，不是这一版 PathKeep 支持的专门浏览历史 JSON。',
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
    importProgressActiveLabel: '第 {current} / {total} 个文件',
    importProgressPrepareDetail: '正在检查 {files} 个待导入文件。',
    importProgressImportActiveDetail:
      '正在写入第 {current} / {total} 个文件：{source}',
    importProgressImportDetail: '已完成第 {current} / {total} 个文件：{source}',
    importProgressFinalizeDetail: '正在更新搜索检索和导入复核数据。',
    importProgressCompleteDetail: '导入复核已就绪，后续重建会在后台继续进行。',
    importingBody: '正在写入记录，请稍候。',
    completeTitle: '第 5 步：完成',
    completeBody: '记录已写入存档。下面可以继续核对这次导入。',
    imported: '已导入',
    duplicatesSkipped: '跳过重复',
    importAnother: '继续导入',
    workflowLabel: '为什么要先预览',
    workflowCollapsedHint: '先扫描，再核对，确认后才会写入存档。',
    showWorkflow: '查看细节',
    hideWorkflow: '隐藏细节',
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
    browserProfileNeedsAccess: '需要权限',
    safariFullDiskAccessHint:
      'Safari History.db 通常需要授予 PathKeep 或当前开发进程“完全磁盘访问权限”后才能暂存。',
    browserProfileUnreadable: '当前无法读取这个历史文件，请检查路径和权限。',
    noDetectedBrowserProfilesTitle: '没有检测到可直接导入的浏览器配置',
    noDetectedBrowserProfilesBody:
      '可以改用文件选择器，或展开手动路径作为兜底。',
    showManualPath: '手动输入路径',
    hideManualPath: '隐藏手动路径',
    selectedSource: '当前来源',
    actionErrorTitle: '导入无法继续',
    batchReviewTitle: '当前导入结果',
    batchReviewBody: '最新导入的批次会先显示在这里，方便你立刻核对或撤销。',
    historyToolsTitle: '导入记录与维护工具',
    historyToolsBody:
      '需要时再展开这里，查看旧批次、撤销或恢复，以及运行健康检查。',
    historyToolsCollapsedHint: '旧批次和修复工具放在这里，需要时再展开。',
    showHistoryTools: '显示记录',
    hideHistoryTools: '隐藏记录',
    recentBatches: '最近导入',
    recentBatchesBody: '想复查旧导入时，再从这里选择批次。',
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
      '这次选择里没有找到专门的 Chrome 历史 JSON。请确认你导出的是浏览历史，而不是 My Activity。',
    takeoutMismatchDetectedTitle:
      '这份导出不是当前 PathKeep 会导入的浏览历史格式',
    takeoutMismatchJsonBody:
      'PathKeep 检测到的是 Chrome My Activity JSON。请重新导出 Chrome 历史，再重新扫描 zip、文件夹或历史 JSON。',
    takeoutMismatchHtmlBody:
      'PathKeep 检测到的是 Chrome My Activity HTML。请重新导出 Chrome 历史，并确认压缩包里有专门的历史 JSON 后再扫描。',
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
    trustTitle: '匯入瀏覽歷史',
    trustBody:
      '這個流程只匯入專門的 Chrome 歷史匯出。其他 Takeout 檔案不會寫入瀏覽歷史，除非後續明確支援。',
    takeoutMethodTitle: 'Google Takeout',
    takeoutMethodBody: '從 Google 資料匯出檔匯入',
    browserMethodTitle: '瀏覽器直接匯入',
    browserMethodBody: '從本機瀏覽器資料庫匯入',
    goToSetup: '前往設定',
    takeoutPreparationHint:
      '請先在 Google Takeout 裡匯出 Chrome。這裡可以直接選 zip、解壓後的資料夾，或歷史 JSON 檔本身。',
    takeoutScopeTitle: '這個流程支援什麼',
    takeoutScopeBody:
      'PathKeep 匯入的是專門的 Chrome 歷史 JSON。檔名可能因語言不同而改變，但內容會以 Browser History 紀錄開頭。',
    takeoutScopeImportable:
      '可以直接選 zip、解壓後的資料夾，或歷史 JSON 檔本身。',
    takeoutScopeIgnored:
      'My Activity JSON、My Activity HTML、Typed URL、Session 與其他 Google 產品匯出都不會寫入瀏覽歷史。',
    takeoutScopeReview:
      '如果檔案看起來相關但不夠明確，PathKeep 會停下來讓你複核。',
    takeoutGuideTitle: '先拿到正確的匯出',
    takeoutGuideBody: '開始前先確認這三件事。',
    takeoutGuideStepOne: '在 Google Takeout 裡匯出 Chrome。',
    takeoutGuideStepTwo:
      '打開 zip 或資料夾，確認裡面有一個專門的 Chrome 歷史 JSON 檔。',
    takeoutGuideStepThree:
      '你可以在 PathKeep 裡直接選 zip、資料夾，或那個 JSON 檔本身。',
    takeoutGuideSupportedExample:
      '常見檔名包括 BrowserHistory.json、History.json、Verlauf.json，以及其他同結構的在地化名稱。',
    takeoutUnsupportedTitle: '這裡不會匯入',
    takeoutGuideUnsupportedExample:
      'Chrome My Activity JSON 和 HTML 匯出不會在這裡匯入。',
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
    groupWillImportBody: '這些檔案會寫入你的瀏覽歷史封存。',
    groupIgnoredTitle: '已知但忽略',
    groupIgnoredBody: '這些檔案屬於匯出的一部分，但不會作為瀏覽歷史匯入。',
    groupNeedsReviewTitle: '需要人工複核',
    groupNeedsReviewBody:
      '這些檔案看起來和歷史有關，但 PathKeep 需要你先確認後再決定。',
    groupParseErrorTitle: '解析失敗',
    groupParseErrorBody: '這些檔案命中了支援的格式，但在匯入前解析失敗了。',
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
      '這是較早的 JSONL 歷史格式。PathKeep 可以匯入，但它不是標準的 Google Takeout Chrome 歷史匯出。',
    reasonSourceEvidenceOnly: '只保留為輔助證據，不會生成瀏覽歷史造訪紀錄。',
    reasonTakeoutIndex:
      '這是匯出清單頁，可用來核對，但不包含可匯入的歷史紀錄。',
    reasonChromeActivityOutsideScope:
      'Chrome My Activity 涵蓋的內容比瀏覽歷史更寬，所以不會走這個匯入流程。',
    reasonChromeMyActivityJson:
      '這是 Chrome My Activity JSON 匯出，不是這一版 PathKeep 支援的專門瀏覽歷史 JSON。',
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
    importProgressActiveLabel: '第 {current} / {total} 個檔案',
    importProgressPrepareDetail: '正在檢查 {files} 個待匯入檔案。',
    importProgressImportActiveDetail:
      '正在寫入第 {current} / {total} 個檔案：{source}',
    importProgressImportDetail: '已完成第 {current} / {total} 個檔案：{source}',
    importProgressFinalizeDetail: '正在更新搜尋檢索和匯入複核資料。',
    importProgressCompleteDetail:
      '匯入複核已就緒，後續重建會在背景工作中繼續進行。',
    importingBody: '正在寫入紀錄，請稍候。',
    completeTitle: '第 5 步：完成',
    completeBody: '紀錄已寫入封存。下面可以繼續核對這次匯入。',
    imported: '已匯入',
    duplicatesSkipped: '略過重複',
    importAnother: '繼續匯入',
    workflowLabel: '為什麼要先預覽',
    workflowCollapsedHint: '先掃描，再核對，確認後才會寫入封存。',
    showWorkflow: '查看細節',
    hideWorkflow: '隱藏細節',
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
    browserProfileNeedsAccess: '需要權限',
    safariFullDiskAccessHint:
      'Safari History.db 通常需要授予 PathKeep 或目前開發行程「完整磁碟取用權」後才能暫存。',
    browserProfileUnreadable: '目前無法讀取這個歷史檔案，請檢查路徑和權限。',
    noDetectedBrowserProfilesTitle: '沒有偵測到可直接匯入的瀏覽器設定檔',
    noDetectedBrowserProfilesBody:
      '可以改用檔案選擇器，或展開手動路徑作為兜底。',
    showManualPath: '手動輸入路徑',
    hideManualPath: '隱藏手動路徑',
    selectedSource: '目前來源',
    actionErrorTitle: '匯入無法繼續',
    batchReviewTitle: '目前匯入結果',
    batchReviewBody: '最新匯入的批次會先顯示在這裡，方便你立刻核對或復原。',
    historyToolsTitle: '匯入紀錄與維護工具',
    historyToolsBody:
      '需要時再展開這裡，查看舊批次、撤銷或恢復，以及執行健康檢查。',
    historyToolsCollapsedHint: '舊批次和修復工具收在這裡，需要時再展開。',
    showHistoryTools: '顯示紀錄',
    hideHistoryTools: '隱藏紀錄',
    recentBatches: '最近匯入',
    recentBatchesBody: '想複查舊匯入時，再從這裡選擇批次。',
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
      '這次選擇裡沒有找到專門的 Chrome 歷史 JSON。請確認你匯出的是瀏覽歷史，而不是 My Activity。',
    takeoutMismatchDetectedTitle:
      '這份匯出不是目前 PathKeep 會匯入的瀏覽歷史格式',
    takeoutMismatchJsonBody:
      'PathKeep 偵測到的是 Chrome My Activity JSON。請重新匯出 Chrome 歷史，再重新掃描 zip、資料夾或歷史 JSON。',
    takeoutMismatchHtmlBody:
      'PathKeep 偵測到的是 Chrome My Activity HTML。請重新匯出 Chrome 歷史，並確認壓縮包裡有專門的歷史 JSON 後再掃描。',
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
