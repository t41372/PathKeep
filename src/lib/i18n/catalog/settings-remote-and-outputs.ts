/**
 * @file settings-remote-and-outputs.ts
 * @description Defines one focused settings translation owner so the overall settings namespace no longer lives in a single mega-file.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide one bounded subsection of the settings namespace for en, zh-CN, and zh-TW.
 * - Keep related settings copy together without reintroducing a second language-specific owner.
 *
 * ## Not responsible for
 * - Other settings copy that belongs to different subgroup owners
 * - Translator creation, flattening, or language resolution
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` imports this static dictionary during catalog assembly.
 *
 * ## Performance notes
 * - Static dictionary data only; keep this file side-effect free so large locale loads stay cheap.
 */

/**
 * Keeps the settings namespace subsection aligned across shipping locales so copy updates stay in one owner.
 */
export const settingsRemoteAndOutputsNamespace = {
  en: {
    restoreReady: 'Restorable',
    externalOutputsTitle: 'EXTERNAL OUTPUTS',
    externalOutputsManualBadge: 'MANUAL ONLY',
    externalOutputsSummaryTitle:
      'Review Core Intelligence outputs before using them elsewhere',
    externalOutputsSummaryBody:
      'Preview embed cards, widget snapshots, and public snapshots here, then copy the payload you need into another trusted local host yourself.',
    externalOutputsScopedTitle: 'Inherited shared profile scope',
    externalOutputsScopedBody:
      'These previews are only reading {profile} right now. Clear the shared profile scope if you want archive-wide output payloads.',
    externalOutputsArchiveWideTitle: 'Archive-wide preview',
    externalOutputsArchiveWideBody:
      'These previews currently use the whole visible archive. Pick a shared profile scope in the shell if you want profile-scoped output payloads.',
    externalOutputsNeedsArchiveTitle:
      'Create an archive before reviewing manual outputs',
    externalOutputsNeedsArchiveBody:
      'PathKeep needs an initialized archive before it can compose embed cards, widget snapshots, or public snapshots.',
    externalOutputsUnlockTitle: 'Unlock the archive to review manual outputs',
    externalOutputsUnlockBody:
      'Manual output previews only load while the current archive session stays unlocked.',
    externalOutputsManualOnlyTitle: 'Manual copy/export only',
    externalOutputsManualOnlyBody:
      'PathKeep does not install widgets, publish a localhost API, or save reusable host artifacts here. Review the payloads and copy them into another trusted local surface yourself.',
    externalOutputsTabEmbed: 'Embed cards',
    externalOutputsTabWidget: 'Widget snapshot',
    externalOutputsTabPublic: 'Public snapshot',
    externalOutputsLoading: 'Loading manual output previews',
    externalOutputsUnavailableTitle: 'Manual outputs are unavailable',
    externalOutputsUnavailableBody:
      'PathKeep could not load the current manual output preview. Try refreshing after the shell finishes reloading.',
    externalOutputsEmbedPreviewTitle: 'Embed card preview',
    externalOutputsTrustedOnlyBadge: 'TRUSTED ONLY',
    externalOutputsHref: 'Payload href',
    externalOutputsOpenInsights: 'Open insights',
    externalOutputsEmbedEmpty:
      'No embed cards are available for this scope yet.',
    externalOutputsCardVisitsTitle: 'Visits',
    externalOutputsCardSearchesTitle: 'Searches',
    externalOutputsCardOnThisDayTitle: 'On This Day · {year}',
    externalOutputsCardTopSiteEyebrow: 'TOP SITE',
    externalOutputsCardRefindEyebrow: 'REFIND',
    externalOutputsCardStableSourceEyebrow: 'STABLE SOURCE',
    externalOutputsCardTotalVisitsBody:
      'Total visits in the selected intelligence window.',
    externalOutputsCardTotalSearchesBody:
      'Total search events observed in the selected intelligence window.',
    externalOutputsCardTopDomainBody:
      '{domain} was one of the most frequently visited domains in this window.',
    externalOutputsCardRefindBody:
      'This page kept resurfacing across {days} days and {trails} trails.',
    externalOutputsCardSourceReference: 'reference',
    externalOutputsCardSourceBody:
      '{domain} often resolves trails as a {source} source.',
    externalOutputsCardMostlyBrowsingBody: 'Mostly browsing {domain}',
    externalOutputsJsonTitle: 'Raw JSON payload',
    externalOutputsWidgetPreviewTitle: 'Widget snapshot preview',
    externalOutputsWindowLabel: 'Window: {start} → {end}',
    externalOutputsWidgetTrustedTitle: 'Trusted-host review required',
    externalOutputsWidgetTrustedBody:
      'This widget snapshot still includes cards marked trusted-only. Keep it inside a trusted PathKeep-controlled surface instead of treating it like a public export.',
    externalOutputsPublicPreviewTitle: 'Public snapshot preview',
    externalOutputsPublicRedactedTitle: 'Public snapshot stays redacted',
    externalOutputsPublicRedactedBody:
      'This payload intentionally omits visit IDs and direct page URLs so it stays safer to share outside trusted PathKeep surfaces.',
    externalOutputsTopDomains: 'Top domains',
    externalOutputsSearchEngines: 'Search engines',
    externalOutputsNoSearchEngines:
      'No search-engine activity was available for this window.',
    externalOutputsDiscoveryTrend: 'Discovery trend',
    externalOutputsNoDiscoveryTrend:
      'No discovery trend points were available for this window.',
    externalOutputsCopyFailed:
      'Could not copy this payload. Copy it manually from the JSON block instead.',
    externalOutputsLocalHostTitle: 'Trusted local host',
    externalOutputsLocalHostBadge: 'TRUSTED LOCAL',
    externalOutputsLocalHostSummaryTitle: 'Reusable browser snippet',
    externalOutputsLocalHostSummaryBody:
      'Build a browser-openable local snippet under the app data folder. It reuses the same shared profile scope and local time window as the manual previews above.',
    externalOutputsLocalHostLoading: 'Loading local host preview',
    externalOutputsLocalHostUnavailableTitle:
      'Trusted local host preview is unavailable',
    externalOutputsLocalHostUnavailableBody:
      'PathKeep could not prepare the current local host preview. Try refreshing after the shell finishes reloading.',
    externalOutputsLocalHostPreviewTitle: 'Preview',
    externalOutputsLocalHostPreviewBody:
      'PathKeep will write or update the trusted local snippet at {path}. Review the generated files before creating it.',
    externalOutputsLocalHostBoundaryTitle: 'Boundary notes',
    externalOutputsLocalHostWarningsTitle: 'Warnings',
    externalOutputsLocalHostManualTitle: 'Manual review',
    externalOutputsLocalHostExecuteTitle: 'Create or update the local snippet',
    externalOutputsLocalHostExecuteBody:
      'This writes index.html and bundle.json into the fixed local host folder. Rebuild it whenever scope, window, or locale changes.',
    externalOutputsLocalHostCreateAction: 'Create local snippet',
    externalOutputsLocalHostUpdateAction: 'Update local snippet',
    externalOutputsLocalHostBuilding: 'Building local snippet…',
    externalOutputsLocalHostBuilt:
      'PathKeep refreshed the trusted local snippet. Review the verify section below.',
    externalOutputsLocalHostVerifyTitle: 'Verify',
    externalOutputsLocalHostVerifyUnavailable:
      'No trusted local snippet is installed for this scope yet.',
    externalOutputsLocalHostScopeLabel: 'Scope',
    externalOutputsLocalHostWindowLabel: 'Window',
    externalOutputsLocalHostGeneratedAtLabel: 'Generated at',
    externalOutputsLocalHostEntryPathLabel: 'Entry file',
    externalOutputsLocalHostArtifactRootLabel: 'Artifact root',
    externalOutputsLocalHostOpenAction: 'Open local host',
    externalOutputsLocalHostCopyPathAction: 'Copy path',
    externalOutputsLocalHostBoundaryDeterministic:
      'This local host only uses deterministic Core Intelligence read models.',
    externalOutputsLocalHostBoundaryTrusted:
      'Trusted-only cards must stay inside PathKeep-controlled local surfaces.',
    externalOutputsLocalHostBoundaryPublic:
      'Public snapshots stay redacted and omit visit-level URLs or identifiers.',
    externalOutputsLocalHostManualReview:
      'Review index.html and bundle.json before handing this folder to another trusted local tool.',
    externalOutputsLocalHostManualOpen:
      'Open index.html from this folder inside a trusted local browser surface.',
    externalOutputsLocalHostManualRebuild:
      'Rebuild this local snippet whenever scope, window, or locale changes.',
    externalOutputsLocalHostWarningTrusted:
      'This local snippet includes trusted-only cards and should not be treated like a public export.',
    externalOutputsLocalHostPurposeEntry:
      'Core Intelligence snippet that can be opened directly in a local browser.',
    externalOutputsLocalHostPurposeBundle:
      'Machine-readable JSON bundle for the same local host artifact.',
    migrationTitle: 'DATA MIGRATION',
    migrationIntro:
      'Move your entire PathKeep archive — config, history, derived projections, audit ledger, raw snapshots, and intelligence sidecars — to another machine, or restore from a previous export. App Lock secrets and platform scheduler artifacts stay on the source machine.',
    migrationExportAction: 'Export bundle',
    migrationExportDescription:
      'Pack the live project into a single .pathkeep file you can carry to another machine.',
    migrationExportingLabel: 'Exporting…',
    migrationExportDialogTitle: 'Save PathKeep export bundle',
    migrationExportErrorTitle: 'Export failed',
    migrationExportedTitle: 'Export complete',
    migrationExportedBody: 'Wrote {fileCount} files ({size}) to {path}.',
    migrationImportAction: 'Import bundle…',
    migrationImportDescription:
      'Restore a previously-exported .pathkeep bundle onto this machine. You will preview the bundle before any data is overwritten.',
    migrationImportingLabel: 'Reading bundle…',
    migrationImportDialogTitle: 'Select PathKeep export bundle',
    migrationPreviewTitle: 'Import preview',
    migrationPreviewErrorTitle: 'Could not read this bundle',
    migrationPreviewExportedAt: 'Exported',
    migrationPreviewAppVersion: 'Source app',
    migrationPreviewSchemaVersion: 'Archive schema',
    migrationPreviewSchemaCurrent: 'matches this build',
    migrationPreviewSchemaWillMigrate:
      'will apply {count} forward migration(s)',
    migrationPreviewArchiveMode: 'Archive mode',
    migrationPreviewFileCount: 'Payload',
    migrationPreviewOverwriteWarning:
      'This will replace the live archive on this machine. The previous project will be preserved next to each restored directory as .bak-<timestamp> sidecars so you can recover if you imported the wrong bundle.',
    migrationPreviewExclusionsLabel: 'What stays on the source machine',
    migrationApplyErrorTitle: 'Import failed',
    migrationConfirmAction: 'Confirm import',
    migrationApplyingLabel: 'Importing…',
    migrationCancelAction: 'Cancel',
    migrationAppliedTitle: 'Import complete',
    migrationAppliedBody:
      'Archive is now at schema v{finalSchemaVersion}. Applied migrations: {migrationsApplied}. {bakNotice}',
    migrationAppliedNoMigrations: 'none',
    migrationAppliedBakNotice:
      'Previous project preserved as .bak-<timestamp> sidecars.',
  },
  'zh-CN': {
    restoreReady: '可恢复',
    externalOutputsTitle: '外部输出',
    externalOutputsManualBadge: '仅手动',
    externalOutputsSummaryTitle:
      '先检查 Core Intelligence 输出，再带到别处使用',
    externalOutputsSummaryBody:
      '在这里预览嵌入卡片、小组件快照和公开快照，然后把需要的载荷手动复制到你信任的本地宿主。',
    externalOutputsScopedTitle: '沿用共享浏览器范围',
    externalOutputsScopedBody:
      '这些预览现在只会读取 {profile}。如果你想看全存档输出，请先清除顶部的共享浏览器范围。',
    externalOutputsArchiveWideTitle: '当前是全存档预览',
    externalOutputsArchiveWideBody:
      '这些预览会读取整个当前可见存档。如果你想只看某个浏览器配置的输出，请先在顶部切换共享浏览器范围。',
    externalOutputsNeedsArchiveTitle: '先创建存档，才能检查手动输出',
    externalOutputsNeedsArchiveBody:
      'PathKeep 需要先完成存档初始化，才能生成嵌入卡片、小组件快照和公开快照。',
    externalOutputsUnlockTitle: '先解锁存档，才能检查手动输出',
    externalOutputsUnlockBody:
      '只有当前存档会话处于解锁状态时，手动输出预览才会加载。',
    externalOutputsManualOnlyTitle: '仅支持手动复制 / 导出',
    externalOutputsManualOnlyBody:
      '这里不会安装小组件、发布本机 API，也不会保存可复用的宿主产物。请先检查载荷，再手动复制到你信任的本地界面。',
    externalOutputsTabEmbed: '嵌入卡片',
    externalOutputsTabWidget: '小组件快照',
    externalOutputsTabPublic: '公开快照',
    externalOutputsLoading: '正在加载手动输出预览',
    externalOutputsUnavailableTitle: '手动输出暂时不可用',
    externalOutputsUnavailableBody:
      'PathKeep 现在无法加载这组手动输出预览。等主界面完成刷新后再试一次。',
    externalOutputsEmbedPreviewTitle: '嵌入卡片预览',
    externalOutputsTrustedOnlyBadge: '仅限受信任宿主',
    externalOutputsHref: '载荷链接',
    externalOutputsOpenInsights: '打开洞察',
    externalOutputsEmbedEmpty: '这个范围里暂时没有可用的嵌入卡片。',
    externalOutputsCardVisitsTitle: '访问',
    externalOutputsCardSearchesTitle: '搜索',
    externalOutputsCardOnThisDayTitle: '历史今日 · {year}',
    externalOutputsCardTopSiteEyebrow: '常访站点',
    externalOutputsCardRefindEyebrow: '反复回访',
    externalOutputsCardStableSourceEyebrow: '稳定来源',
    externalOutputsCardTotalVisitsBody: '这个智能时间窗口内的总访问次数。',
    externalOutputsCardTotalSearchesBody:
      '这个智能时间窗口内观察到的搜索事件总数。',
    externalOutputsCardTopDomainBody:
      '{domain} 是这个时间窗口中最常访问的域名之一。',
    externalOutputsCardRefindBody:
      '这个页面在 {days} 天、{trails} 条轨迹中反复出现。',
    externalOutputsCardSourceReference: '参考',
    externalOutputsCardSourceBody:
      '{domain} 经常作为{source}来源帮助收束浏览轨迹。',
    externalOutputsCardMostlyBrowsingBody: '主要在浏览 {domain}',
    externalOutputsJsonTitle: '原始 JSON 载荷',
    externalOutputsWidgetPreviewTitle: '小组件快照预览',
    externalOutputsWindowLabel: '时间范围：{start} → {end}',
    externalOutputsWidgetTrustedTitle: '需要受信任宿主审查',
    externalOutputsWidgetTrustedBody:
      '这份小组件快照仍包含仅限受信任宿主的卡片。请把它留在 PathKeep 控制的受信任宿主里，不要把它当成公开导出内容。',
    externalOutputsPublicPreviewTitle: '公开快照预览',
    externalOutputsPublicRedactedTitle: '公开快照会保持脱敏',
    externalOutputsPublicRedactedBody:
      '这份载荷会刻意省略访问 ID 和直接页面 URL，离开 PathKeep 受信任界面时会更安全。',
    externalOutputsTopDomains: '常访域名',
    externalOutputsSearchEngines: '搜索引擎',
    externalOutputsNoSearchEngines: '这个时间范围里没有可用的搜索引擎活动。',
    externalOutputsDiscoveryTrend: '发现趋势',
    externalOutputsNoDiscoveryTrend: '这个时间范围里没有可用的发现趋势点。',
    externalOutputsCopyFailed:
      '这份载荷无法直接复制，请改为手动从 JSON 区块复制。',
    externalOutputsLocalHostTitle: '受信任本地宿主',
    externalOutputsLocalHostBadge: '仅限本地',
    externalOutputsLocalHostSummaryTitle: '可复用的浏览器片段',
    externalOutputsLocalHostSummaryBody:
      '在应用资料目录下生成一个可直接打开的本地浏览器片段。它会沿用上面手动预览同一套共享浏览器范围和本地时间窗口。',
    externalOutputsLocalHostLoading: '正在加载本地宿主预览',
    externalOutputsLocalHostUnavailableTitle: '本地宿主预览暂时不可用',
    externalOutputsLocalHostUnavailableBody:
      'PathKeep 现在无法准备这组本地宿主预览。等主界面完成刷新后再试一次。',
    externalOutputsLocalHostPreviewTitle: '预览',
    externalOutputsLocalHostPreviewBody:
      'PathKeep 会在 {path} 写入或更新这个受信任的本地片段。先检查生成文件，再决定是否创建它。',
    externalOutputsLocalHostBoundaryTitle: '边界说明',
    externalOutputsLocalHostWarningsTitle: '警告',
    externalOutputsLocalHostManualTitle: '手动检查',
    externalOutputsLocalHostExecuteTitle: '创建或更新本地片段',
    externalOutputsLocalHostExecuteBody:
      '这个动作会把 index.html 和 bundle.json 写入固定的本地宿主目录。只要范围、时间窗口或语言变了，就应该重新生成。',
    externalOutputsLocalHostCreateAction: '创建本地片段',
    externalOutputsLocalHostUpdateAction: '更新本地片段',
    externalOutputsLocalHostBuilding: '正在生成本地片段…',
    externalOutputsLocalHostBuilt:
      'PathKeep 已刷新受信任的本地片段。请在下面的验证区继续检查。',
    externalOutputsLocalHostVerifyTitle: '验证',
    externalOutputsLocalHostVerifyUnavailable:
      '这个范围目前还没有已安装的受信任本地片段。',
    externalOutputsLocalHostScopeLabel: '范围',
    externalOutputsLocalHostWindowLabel: '时间窗口',
    externalOutputsLocalHostGeneratedAtLabel: '生成时间',
    externalOutputsLocalHostEntryPathLabel: '入口文件',
    externalOutputsLocalHostArtifactRootLabel: '产物目录',
    externalOutputsLocalHostOpenAction: '打开本地宿主',
    externalOutputsLocalHostCopyPathAction: '复制路径',
    externalOutputsLocalHostBoundaryDeterministic:
      '这个本地宿主只使用确定性的 Core Intelligence 读取模型。',
    externalOutputsLocalHostBoundaryTrusted:
      '仅限受信任宿主的卡片必须留在 PathKeep 控制的本地界面内。',
    externalOutputsLocalHostBoundaryPublic:
      '公开快照会保持脱敏，不包含访问级 URL 或标识字段。',
    externalOutputsLocalHostManualReview:
      '先检查 index.html 和 bundle.json，再把这个文件夹交给其他受信任的本地工具。',
    externalOutputsLocalHostManualOpen:
      '直接从这个文件夹打开 index.html，在受信任的本地浏览器宿主里查看它。',
    externalOutputsLocalHostManualRebuild:
      '只要范围、时间窗口或语言发生变化，就重新创建这个本地片段。',
    externalOutputsLocalHostWarningTrusted:
      '这个本地片段包含仅限受信任宿主的卡片，不能把它当成公开导出。',
    externalOutputsLocalHostPurposeEntry:
      '可直接在本机浏览器打开的 Core Intelligence 片段。',
    externalOutputsLocalHostPurposeBundle:
      '同一份本地宿主数据的机器可读 JSON 包。',
    migrationTitle: '数据迁移',
    migrationIntro:
      '把整个 PathKeep 项目——配置、历史记录、派生数据、审计账本、原始快照与智能侧链——迁移到另一台机器，或者还原一份之前导出的数据。App Lock 密钥与平台定时任务保留在源机器上。',
    migrationExportAction: '导出数据包',
    migrationExportDescription:
      '把当前项目打包成一个 .pathkeep 文件，可以带去另一台机器。',
    migrationExportingLabel: '导出中…',
    migrationExportDialogTitle: '保存 PathKeep 导出数据包',
    migrationExportErrorTitle: '导出失败',
    migrationExportedTitle: '导出完成',
    migrationExportedBody: '已把 {fileCount} 个文件（{size}）写入 {path}。',
    migrationImportAction: '导入数据包…',
    migrationImportDescription:
      '把之前导出的 .pathkeep 数据包还原到这台机器。会先预览数据包内容，再确认覆盖。',
    migrationImportingLabel: '读取数据包中…',
    migrationImportDialogTitle: '选择 PathKeep 导出数据包',
    migrationPreviewTitle: '导入预览',
    migrationPreviewErrorTitle: '无法读取此数据包',
    migrationPreviewExportedAt: '导出于',
    migrationPreviewAppVersion: '源端 App',
    migrationPreviewSchemaVersion: '存档 schema',
    migrationPreviewSchemaCurrent: '与当前版本一致',
    migrationPreviewSchemaWillMigrate: '将向前应用 {count} 次迁移',
    migrationPreviewArchiveMode: '存档模式',
    migrationPreviewFileCount: '内容',
    migrationPreviewOverwriteWarning:
      '这会替换这台机器上的现有存档。原先的项目会以 .bak-<时间戳> 为后缀保留在每个目录旁边，万一导入错的数据包还可以恢复。',
    migrationPreviewExclusionsLabel: '哪些数据会留在源机器',
    migrationApplyErrorTitle: '导入失败',
    migrationConfirmAction: '确认导入',
    migrationApplyingLabel: '导入中…',
    migrationCancelAction: '取消',
    migrationAppliedTitle: '导入完成',
    migrationAppliedBody:
      '存档现在是 schema v{finalSchemaVersion}。已应用的迁移：{migrationsApplied}。{bakNotice}',
    migrationAppliedNoMigrations: '无',
    migrationAppliedBakNotice: '原先的项目已以 .bak-<时间戳> 为后缀保留。',
  },
  'zh-TW': {
    restoreReady: '可還原',
    externalOutputsTitle: '外部輸出',
    externalOutputsManualBadge: '僅手動',
    externalOutputsSummaryTitle:
      '先檢查 Core Intelligence 輸出，再帶到別處使用',
    externalOutputsSummaryBody:
      '在這裡預覽嵌入卡片、小工具快照和公開快照，然後把需要的載荷手動複製到你信任的本地宿主。',
    externalOutputsScopedTitle: '沿用共享瀏覽器範圍',
    externalOutputsScopedBody:
      '這些預覽現在只會讀取 {profile}。如果你想看全封存輸出，請先清除頂部的共享瀏覽器範圍。',
    externalOutputsArchiveWideTitle: '目前是全封存預覽',
    externalOutputsArchiveWideBody:
      '這些預覽會讀取整個目前可見封存。如果你想只看某個瀏覽器設定檔的輸出，請先在頂部切換共享瀏覽器範圍。',
    externalOutputsNeedsArchiveTitle: '先建立封存，才能檢查手動輸出',
    externalOutputsNeedsArchiveBody:
      'PathKeep 需要先完成封存初始化，才能產生嵌入卡片、小工具快照和公開快照。',
    externalOutputsUnlockTitle: '先解鎖封存，才能檢查手動輸出',
    externalOutputsUnlockBody:
      '只有目前封存工作階段保持解鎖時，手動輸出預覽才會載入。',
    externalOutputsManualOnlyTitle: '僅支援手動複製 / 匯出',
    externalOutputsManualOnlyBody:
      '這裡不會安裝小工具、發布本機 API，也不會儲存可重用的宿主產物。請先檢查載荷，再手動複製到你信任的本地介面。',
    externalOutputsTabEmbed: '嵌入卡片',
    externalOutputsTabWidget: '小工具快照',
    externalOutputsTabPublic: '公開快照',
    externalOutputsLoading: '正在載入手動輸出預覽',
    externalOutputsUnavailableTitle: '手動輸出暫時無法使用',
    externalOutputsUnavailableBody:
      'PathKeep 目前無法載入這組手動輸出預覽。等主介面完成重新整理後再試一次。',
    externalOutputsEmbedPreviewTitle: '嵌入卡片預覽',
    externalOutputsTrustedOnlyBadge: '僅限受信任宿主',
    externalOutputsHref: '載荷連結',
    externalOutputsOpenInsights: '打開洞察',
    externalOutputsEmbedEmpty: '這個範圍裡暫時沒有可用的嵌入卡片。',
    externalOutputsCardVisitsTitle: '造訪',
    externalOutputsCardSearchesTitle: '搜尋',
    externalOutputsCardOnThisDayTitle: '歷史今日 · {year}',
    externalOutputsCardTopSiteEyebrow: '常訪站點',
    externalOutputsCardRefindEyebrow: '反覆回訪',
    externalOutputsCardStableSourceEyebrow: '穩定來源',
    externalOutputsCardTotalVisitsBody: '這個智慧時間視窗內的總造訪次數。',
    externalOutputsCardTotalSearchesBody:
      '這個智慧時間視窗內觀察到的搜尋事件總數。',
    externalOutputsCardTopDomainBody:
      '{domain} 是這個時間視窗中最常造訪的網域之一。',
    externalOutputsCardRefindBody:
      '這個頁面在 {days} 天、{trails} 條軌跡中反覆出現。',
    externalOutputsCardSourceReference: '參考',
    externalOutputsCardSourceBody:
      '{domain} 經常作為{source}來源幫助收束瀏覽軌跡。',
    externalOutputsCardMostlyBrowsingBody: '主要在瀏覽 {domain}',
    externalOutputsJsonTitle: '原始 JSON 載荷',
    externalOutputsWidgetPreviewTitle: '小工具快照預覽',
    externalOutputsWindowLabel: '時間範圍：{start} → {end}',
    externalOutputsWidgetTrustedTitle: '需要受信任宿主審查',
    externalOutputsWidgetTrustedBody:
      '這份小工具快照仍包含僅限受信任宿主的卡片。請把它留在 PathKeep 控制的受信任宿主裡，不要把它當成公開匯出內容。',
    externalOutputsPublicPreviewTitle: '公開快照預覽',
    externalOutputsPublicRedactedTitle: '公開快照會保持去識別化',
    externalOutputsPublicRedactedBody:
      '這份載荷會刻意省略造訪 ID 和直接頁面 URL，離開 PathKeep 受信任介面時會更安全。',
    externalOutputsTopDomains: '常訪網域',
    externalOutputsSearchEngines: '搜尋引擎',
    externalOutputsNoSearchEngines: '這個時間範圍裡沒有可用的搜尋引擎活動。',
    externalOutputsDiscoveryTrend: '發現趨勢',
    externalOutputsNoDiscoveryTrend: '這個時間範圍裡沒有可用的發現趨勢點。',
    externalOutputsCopyFailed:
      '這份載荷無法直接複製，請改為手動從 JSON 區塊複製。',
    externalOutputsLocalHostTitle: '受信任本地宿主',
    externalOutputsLocalHostBadge: '僅限本地',
    externalOutputsLocalHostSummaryTitle: '可重用的瀏覽器片段',
    externalOutputsLocalHostSummaryBody:
      '在應用資料目錄下產生一個可直接開啟的本地瀏覽器片段。它會沿用上面手動預覽同一套共享瀏覽器範圍和本地時間視窗。',
    externalOutputsLocalHostLoading: '正在載入本地宿主預覽',
    externalOutputsLocalHostUnavailableTitle: '本地宿主預覽暫時無法使用',
    externalOutputsLocalHostUnavailableBody:
      'PathKeep 目前無法準備這組本地宿主預覽。等主介面完成重新整理後再試一次。',
    externalOutputsLocalHostPreviewTitle: '預覽',
    externalOutputsLocalHostPreviewBody:
      'PathKeep 會在 {path} 寫入或更新這個受信任的本地片段。先檢查生成檔案，再決定是否建立它。',
    externalOutputsLocalHostBoundaryTitle: '邊界說明',
    externalOutputsLocalHostWarningsTitle: '警告',
    externalOutputsLocalHostManualTitle: '手動檢查',
    externalOutputsLocalHostExecuteTitle: '建立或更新本地片段',
    externalOutputsLocalHostExecuteBody:
      '這個動作會把 index.html 和 bundle.json 寫入固定的本地宿主目錄。只要範圍、時間視窗或語言變了，就應該重新產生。',
    externalOutputsLocalHostCreateAction: '建立本地片段',
    externalOutputsLocalHostUpdateAction: '更新本地片段',
    externalOutputsLocalHostBuilding: '正在產生本地片段…',
    externalOutputsLocalHostBuilt:
      'PathKeep 已刷新受信任的本地片段。請在下方的驗證區繼續檢查。',
    externalOutputsLocalHostVerifyTitle: '驗證',
    externalOutputsLocalHostVerifyUnavailable:
      '這個範圍目前還沒有已安裝的受信任本地片段。',
    externalOutputsLocalHostScopeLabel: '範圍',
    externalOutputsLocalHostWindowLabel: '時間視窗',
    externalOutputsLocalHostGeneratedAtLabel: '產生時間',
    externalOutputsLocalHostEntryPathLabel: '入口檔案',
    externalOutputsLocalHostArtifactRootLabel: '產物目錄',
    externalOutputsLocalHostOpenAction: '開啟本地宿主',
    externalOutputsLocalHostCopyPathAction: '複製路徑',
    externalOutputsLocalHostBoundaryDeterministic:
      '這個本地宿主只使用確定性的 Core Intelligence 讀取模型。',
    externalOutputsLocalHostBoundaryTrusted:
      '僅限受信任宿主的卡片必須留在 PathKeep 控制的本地介面內。',
    externalOutputsLocalHostBoundaryPublic:
      '公開快照會保持去識別化，不包含造訪級 URL 或識別欄位。',
    externalOutputsLocalHostManualReview:
      '先檢查 index.html 與 bundle.json，再把這個資料夾交給其他受信任的本地工具。',
    externalOutputsLocalHostManualOpen:
      '從這個資料夾直接打開 index.html，在受信任的本地瀏覽器宿主裡檢視它。',
    externalOutputsLocalHostManualRebuild:
      '只要範圍、時間視窗或語言改變，就重新建立這個本地片段。',
    externalOutputsLocalHostWarningTrusted:
      '這個本地片段包含僅限受信任宿主的卡片，不能把它當成公開匯出。',
    externalOutputsLocalHostPurposeEntry:
      '可直接在本機瀏覽器開啟的 Core Intelligence 片段。',
    externalOutputsLocalHostPurposeBundle:
      '同一份本地宿主資料的機器可讀 JSON 包。',
    migrationTitle: '資料遷移',
    migrationIntro:
      '把整個 PathKeep 專案——設定、歷史紀錄、衍生資料、稽核帳本、原始快照與智能側鏈——搬到另一台機器，或還原一份先前匯出的資料。App Lock 密鑰與平台排程僅留在來源機器上。',
    migrationExportAction: '匯出資料包',
    migrationExportDescription:
      '把目前的專案打包成一個 .pathkeep 檔案，可以帶到另一台機器。',
    migrationExportingLabel: '匯出中…',
    migrationExportDialogTitle: '儲存 PathKeep 匯出資料包',
    migrationExportErrorTitle: '匯出失敗',
    migrationExportedTitle: '匯出完成',
    migrationExportedBody: '已將 {fileCount} 個檔案（{size}）寫入 {path}。',
    migrationImportAction: '匯入資料包…',
    migrationImportDescription:
      '把之前匯出的 .pathkeep 資料包還原到這台機器。會先預覽資料包內容，再確認覆寫。',
    migrationImportingLabel: '讀取資料包中…',
    migrationImportDialogTitle: '選擇 PathKeep 匯出資料包',
    migrationPreviewTitle: '匯入預覽',
    migrationPreviewErrorTitle: '無法讀取此資料包',
    migrationPreviewExportedAt: '匯出於',
    migrationPreviewAppVersion: '來源 App',
    migrationPreviewSchemaVersion: '封存 schema',
    migrationPreviewSchemaCurrent: '與目前版本一致',
    migrationPreviewSchemaWillMigrate: '將向前套用 {count} 次遷移',
    migrationPreviewArchiveMode: '封存模式',
    migrationPreviewFileCount: '內容',
    migrationPreviewOverwriteWarning:
      '這會替換這台機器上既有的封存。原先的專案會以 .bak-<時間戳> 為後綴保留在每個目錄旁邊，萬一匯入錯誤的資料包還可以救回。',
    migrationPreviewExclusionsLabel: '哪些資料會留在來源機器',
    migrationApplyErrorTitle: '匯入失敗',
    migrationConfirmAction: '確認匯入',
    migrationApplyingLabel: '匯入中…',
    migrationCancelAction: '取消',
    migrationAppliedTitle: '匯入完成',
    migrationAppliedBody:
      '封存現在是 schema v{finalSchemaVersion}。已套用的遷移：{migrationsApplied}。{bakNotice}',
    migrationAppliedNoMigrations: '無',
    migrationAppliedBakNotice: '原先的專案已以 .bak-<時間戳> 為後綴保留。',
  },
} as const
