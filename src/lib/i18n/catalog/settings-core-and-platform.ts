/**
 * @file settings-core-and-platform.ts
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
export const settingsCoreAndPlatformNamespace = {
  en: {
    loadingSettings: 'Loading settings…',
    loadingModules: 'Loading…',
    archiveUnlockTitle: 'Unlock the archive before reviewing settings',
    archiveUnlockBody:
      'PathKeep can still inspect scheduler and keyring posture, but the rest of Settings needs the archive session key. Open Security to unlock it first.',
    unavailableTitle: 'Settings are temporarily unavailable',
    unavailableBody:
      'PathKeep could not load the current settings review surface. Try refreshing after the shell finishes reloading.',
    browserProfiles: 'BROWSER PROFILES',
    browserProfilesBody:
      'Select which browsers to include in your archive. History will only be backed up from checked profiles.',
    aiProvider: 'AI PROVIDER',
    aiProviderBody:
      'Connect a local or self-hosted AI provider to chat with your history and turn on smart search. Optional and off by default — keyword search and Core Intelligence work without it.',
    optional: 'OPTIONAL',
    savedConfirmation: 'Saved',
    groupCore: 'CORE',
    groupLookFeel: 'LOOK & FEEL',
    groupDataSources: 'DATA SOURCES',
    groupData: 'DATA',
    groupDisplay: 'DISPLAY',
    groupDataUpdates: 'DATA & UPDATES',
    groupSecurityAccess: 'SECURITY & ACCESS',
    groupIntelligence: 'INTELLIGENCE',
    groupBackupSync: 'BACKUP & SYNC',
    groupPlatform: 'PLATFORM',
    groupPrivacyAccess: 'PRIVACY & ACCESS',
    groupMaintenance: 'UPDATES & CLEANUP',
    groupDerivedData: 'DERIVED DATA',
    groupDiagnostics: 'DIAGNOSTICS',
    groupExternalOutputs: 'EXTERNAL OUTPUTS',
    groupGeneratedArtifacts: 'GENERATED ARTIFACTS',
    externalReviewBadge: 'REVIEW ONLY',
    general: 'GENERAL',
    generalDescription: 'Language and Explorer performance preferences.',
    paperHeaderEyebrow: 'Preferences',
    paperHeaderTitle: 'Settle the page before you read.',
    paperHeaderSubtitle:
      'Your reading environment and archive connections. Every change saves automatically.',
    paperJumpLabel: 'Jump to',
    // ── Paper-redesign appearance ──
    appearanceTitle: 'Appearance',
    appearanceIntro:
      'Light or darkroom, the typography of a book, and how much paper materiality you want under the ink.',
    appearanceTheme: 'Theme',
    appearanceThemeLight: 'Paper · light',
    appearanceThemeDark: 'Darkroom · dark',
    appearanceFonts: 'Typography',
    appearanceFontsHelp:
      'Bundled fonts ship offline; system fonts skip the bundle and use what your OS already provides. CJK always falls back to system fonts either way.',
    appearanceFontBundled: 'Bundled · Newsreader + JetBrains Mono',
    appearanceFontBundledHint: 'Default · ~400 KB of Latin subsets',
    appearanceFontSystem: 'System fonts only',
    appearanceFontSystemHint: 'Georgia / SF Mono / system sans',
    appearanceDensity: 'Density',
    appearanceDensityComfortable: 'Comfortable',
    appearanceDensityCompact: 'Compact',
    appearancePaperTexture: 'Paper materiality',
    appearancePaperTextureHelp:
      'Subtle paper noise overlay and the dark-mode darkroom vignette. Turn off for a flatter surface.',
    appearancePaperOn: 'Texture on',
    appearancePaperOff: 'Texture off',
    appearanceClockLabel: 'Time format',
    appearanceClockHelp:
      'Used everywhere except sparkline / chart axes, which always show 24-hour for compactness.',
    appearanceClock12h: '12-hour (3:14 PM)',
    appearanceClock24h: '24-hour (15:14)',
    // ── Link previews (og:image cache) ──
    linkPreviewsTitle: 'Link previews',
    linkPreviewsIntro:
      'Card-mode Browse fetches each page’s og:image preview when one is available. Bytes are cached locally and deduplicated; nothing is sent off-machine besides the GET to the page itself.',
    linkPreviewsFetchToggleLabel: 'Fetch link previews from the web',
    linkPreviewsFetchToggleHint:
      'When off, card mode falls back to the favicon or the domain swatch. Already-cached previews still render.',
    linkPreviewsFetchOn: 'Fetching on',
    linkPreviewsFetchOff: 'Fetching off',
    linkPreviewsFetchModeLabel: 'Fetch policy',
    linkPreviewsFetchModeHint:
      'Background uses the per-backup tick to pre-warm new visits + retry transient failures. On-demand only fetches when you scroll a card into view. Off disables all fetching while keeping the cache intact.',
    linkPreviewsFetchModeOff: 'Off',
    linkPreviewsFetchModeOffHint: 'No fetching anywhere.',
    linkPreviewsFetchModeOnDemand: 'On demand',
    linkPreviewsFetchModeOnDemandHint:
      'Fetches only when a card scrolls into view.',
    linkPreviewsFetchModeBackground: 'Background',
    linkPreviewsFetchModeBackgroundHint:
      'On-demand + per-backup pre-warm + daily retry. Recommended.',
    linkPreviewsBudgetsLabel: 'Per-backup budgets',
    linkPreviewsBudgetsHint:
      'Caps how many URLs the daily retry pass and the new-visit pre-warm sweep enqueue at most. Lower values keep outbound HTTP bounded; zero disables that pass.',
    linkPreviewsDailyRefetchBudgetLabel: 'Retry budget / day',
    linkPreviewsPrefetchBudgetLabel: 'Pre-warm budget / backup',
    linkPreviewsRebuildAction: 'Rebuild now ({budget})',
    linkPreviewsRebuildHint:
      'Sweeps up to {budget} of the most recently visited URLs without a cached preview (worker hard-caps any single pass at {cap}).',
    linkPreviewsRebuildSummary: 'Enqueued {enqueued}, succeeded {succeeded}.',
    linkPreviewsStatsLabel: 'Cache footprint',
    linkPreviewsStatsRows: '{rows} rows · {blobs} blobs · {bytes}',
    linkPreviewsStatsEmpty: 'No previews cached yet.',
    linkPreviewsCoverageLabel: 'Preview coverage',
    linkPreviewsCoverageValue:
      '{percent}% of pages have a preview image ({withImage} of {eligible})',
    linkPreviewsCoverageEmpty: 'No pages to measure yet.',
    linkPreviewsCoverageLoading: 'Measuring coverage…',
    linkPreviewsCoverageRate: 'Of {checked} pages checked, {rate}% had one.',
    linkPreviewsCoverageError: 'Couldn’t measure coverage.',
    linkPreviewsCoverageNotFetched:
      'No previews fetched yet — they’re fetched in the background as you browse.',
    linkPreviewsCleanupLabel: 'Maintenance',
    linkPreviewsRunCleanupAction: 'Run cleanup now',
    linkPreviewsClearAllAction: 'Clear all link previews',
    linkPreviewsClearConfirm:
      'Delete every cached link preview? This is reversible only by re-fetching each page.',
    linkPreviewsCleanupSummary:
      'Deleted {rows} rows, {blobs} blobs, reclaimed {bytes}.',
    linkPreviewsBlocklistLabel: 'Domain blocklist',
    linkPreviewsBlocklistHint:
      'One host per line — every page on these domains skips og:image fetching. Lines starting with # are treated as comments.',
    linkPreviewsBlocklistPlaceholder:
      'example.com\nbanking.example.org\n# private corporate intranet\nintranet.example.local',
    linkPreviewsCleanupModeLabel: 'Eviction policy',
    linkPreviewsCleanupModeHint:
      'Off keeps every preview forever; pick another mode to let PathKeep prune the cache automatically on the daily maintenance tick.',
    linkPreviewsCleanupModeOff: 'Off',
    linkPreviewsCleanupModeOffHint: 'Cache grows unbounded.',
    linkPreviewsCleanupModeTimeTtl: 'Age',
    linkPreviewsCleanupModeTimeTtlHint: 'Drop rows older than the limit.',
    linkPreviewsCleanupModeSizeCap: 'Size cap',
    linkPreviewsCleanupModeSizeCapHint:
      'Drop the oldest rows until the total fits.',
    linkPreviewsCleanupModeLru: 'LRU',
    linkPreviewsCleanupModeLruHint:
      'Drop the least-recently-shown rows until the total fits.',
    linkPreviewsMaxAgeDaysLabel: 'Maximum age',
    linkPreviewsMaxAgeDaysUnit: 'days',
    linkPreviewsMaxBytesLabel: 'Maximum total',
    linkPreviewsMaxBytesUnit: 'MB',
    openMaintenanceBody:
      'Updates, cleanup, derived-data rebuilds, remote backup workflows, diagnostics, and platform troubleshooting.',
    maintenanceTitle: 'Maintenance',
    maintenanceBody:
      'Run advanced upkeep here. Preview destructive cleanup, follow update state, rebuild derived data, and review support diagnostics without crowding daily preferences.',
    maintenanceUnavailableTitle: 'Maintenance is temporarily unavailable',
    integrationsTitle: 'Integrations',
    integrationsBody:
      'Review payloads and generated files before using them in trusted local tools. Raw JSON and code are bounded inside review panels.',
    integrationsUnavailableTitle: 'Integrations are temporarily unavailable',
    openJobsBody:
      'Background Jobs remains the canonical place for runtime progress, retries, cancellation, and logs.',
    backToSettingsBody:
      'Return to everyday preferences such as language, browsers, privacy, AI providers, and saved backup configuration.',
    diagnosticsTitle: 'Support diagnostics',
    diagnosticsBody:
      'Use these paths and build details when checking logs, crash reports, or local support artifacts.',
    retentionTitle: 'RETENTION & CLEANUP',
    retentionDescription:
      'Review which local artifacts can be removed now. PathKeep only prunes them when you explicitly run this action.',
    retentionSelected: '{size} selected',
    retentionUnlockTitle: 'Unlock the archive before pruning',
    retentionUnlockBody:
      'Snapshot pruning updates the audit ledger as it deletes files, so PathKeep needs the archive unlocked first.',
    retentionSnapshots: 'Saved snapshots',
    retentionExports: 'Local exports',
    retentionStaging: 'Staging files',
    retentionQuarantine: 'Quarantined files',
    retentionItems: 'items',
    retentionLoadingTitle: 'Loading retention preview',
    retentionRefresh: 'Refresh preview',
    retentionExecute: 'Prune selected',
    retentionDeletedBytes: 'Deleted {size} of local artifacts.',
    retentionDeletedFiles: 'Removed {count} files or directories.',
    retentionOpenAudit: 'Open prune review',
    retentionNothingSelected:
      'Select at least one retention bucket before pruning.',
    retentionSnapshotPruneWarning:
      'Snapshot pruning removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.',
    retentionExportPruneWarning:
      'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.',
    appLock: 'APP LOCK',
    appLockBoundaryTitle: 'Session-only protection',
    appLockBoundaryBody:
      'App Lock protects the desktop UI session only. Archive encryption remains a separate at-rest control, and shared profile scope stays a filter, not a separate partition.',
    appLockEnabled: 'Enable App Lock',
    appLockStatus: 'Status',
    appLockStatusLocked: 'Locked',
    appLockStatusUnlocked: 'Unlocked',
    appLockIdleTimeout: 'Idle timeout',
    appLockMinutes: '{count} minutes',
    appLockBiometric: 'Allow biometric unlock when available',
    appLockTouchId: 'Allow Touch ID unlock when available',
    appLockBiometricUnavailable:
      'Biometric unlock is not wired into this build yet, so passcode unlock stays required.',
    appLockTouchIdUnavailable:
      'Touch ID is unavailable on this Mac right now, so passcode unlock stays required.',
    appLockTouchIdAvailable:
      'Touch ID is available on this Mac and can unlock the current PathKeep session.',
    appLockRecoveryHint: 'Recovery hint',
    appLockRecoveryHintPlaceholder:
      'Optional reminder shown on the lock screen',
    appLockPasscode: 'Passcode',
    appLockPasscodePlaceholder: 'Set or replace the current app lock passcode',
    appLockSetPasscode: 'Save passcode',
    appLockUpdatePasscode: 'Update passcode',
    appLockSavingPasscode: 'Saving passcode…',
    appLockClearPasscode: 'Clear passcode',
    appLockClearingPasscode: 'Clearing passcode…',
    appLockLockNow: 'Lock now',
    appLockLockingNow: 'Locking now…',
    appLockNeedsPasscodeTitle: 'Set a passcode first',
    appLockNeedsPasscodeBody:
      'Save an app lock passcode before enabling the lock. PathKeep does not treat an unchecked toggle as security.',
    appLockConfigPath: 'Config path',
    appLockLastUnlocked: 'Last unlocked',
    archiveSecurity: 'ARCHIVE KEY',
    baseUrlLabel: 'Base URL',
    embeddingModelLabel: 'Embedding model',
    llmModelLabel: 'Chat model',
    apiKeyLabel: 'API key',
    interfaceLanguage: 'Language',
    currentLanguage: 'Current',
    explorerBackgroundPrefetchPages: 'Explorer background prefetch',
    explorerBackgroundPrefetchDisabled: 'Disabled',
    explorerBackgroundPrefetchOption: '{count} pages per side',
    explorerBackgroundPrefetchBody:
      'After the current Explorer page appears, PathKeep can warm nearby pages in the background. Higher values make next and previous navigation feel more immediate, but they also add more background reads.',
    dataDirectory: 'Data folder',
    archiveDatabase: 'Archive database',
    auditRepository: 'Audit logs',
    logsDirectory: 'Logs folder',
    revealLogsButton: 'Reveal logs in Finder',
    revealLogsAriaLabel: 'Open the PathKeep logs folder in the file manager',
    crashReports: 'Crash reports',
    openDirectory: 'Open folder',
    openCrashReport: 'Open crash report',
    mcpServer: 'MCP Server',
    version: 'App version',
    gitCommit: 'Git hash',
    latestCrashTitle: 'Recent crash report detected',
    latestCrashBody: '{source} recorded at {time}. Latest message: {message}',
    latestCrashSourceRust: 'Rust panic',
    latestCrashSourceFrontend: 'Frontend crash',
    latestCrashClearTitle: 'No recent crash report',
    latestCrashClearBody:
      'PathKeep has not recorded a recent panic or uncaught frontend failure in this data directory.',
  },
  'zh-CN': {
    loadingSettings: '加载设置…',
    loadingModules: '加载中…',
    archiveUnlockTitle: '请先解锁存档再检查设置',
    archiveUnlockBody:
      'PathKeep 仍然可以检查定时备份和钥匙串状态，但其余设置需要先提供存档会话密钥。请先到安全页面解锁。',
    unavailableTitle: '设置暂时不可用',
    unavailableBody:
      'PathKeep 当前无法加载这组设置检查界面。等主界面刷新完成后再试一次。',
    browserProfiles: '浏览器',
    browserProfilesBody: '选择要备份的浏览器。只有勾选的浏览器会被纳入存档。',
    aiProvider: 'AI 服务',
    aiProviderBody:
      '连接一个本地或自托管的 AI 服务，即可与你的历史对话并开启智能搜索。可选功能，默认关闭——关键词搜索和确定性智能分析无需它也能使用。',
    optional: '可选',
    savedConfirmation: '已保存',
    groupCore: '核心',
    groupLookFeel: '外观与感觉',
    groupDataSources: '数据来源',
    groupData: '数据',
    groupDisplay: '显示',
    groupDataUpdates: '数据与更新',
    groupSecurityAccess: '安全与访问',
    groupIntelligence: '智能',
    groupBackupSync: '备份与同步',
    groupPlatform: '平台',
    groupPrivacyAccess: '隐私与访问',
    groupMaintenance: '更新与清理',
    groupDerivedData: '派生数据',
    groupDiagnostics: '诊断',
    groupExternalOutputs: '外部输出',
    groupGeneratedArtifacts: '生成产物',
    externalReviewBadge: '仅复核',
    general: '通用',
    generalDescription: '语言和 Explorer 性能偏好。',
    paperHeaderEyebrow: '偏好设置',
    paperHeaderTitle: '在阅读之前,先安顿好这一页。',
    paperHeaderSubtitle: '您的阅读环境与档案连接设置。所有更改都会自动保存。',
    paperJumpLabel: '跳转到',
    appearanceTitle: '外观',
    appearanceIntro:
      '选择纸面（亮色）还是暗房（深色），调整书一般的字体，并决定要不要在墨水之下保留纸张的质感。',
    appearanceTheme: '主题',
    appearanceThemeLight: '纸面 · 亮色',
    appearanceThemeDark: '暗房 · 深色',
    appearanceFonts: '字体',
    appearanceFontsHelp:
      '内置字体支持离线运行；系统字体跳过内置包，直接使用系统已有的字体。中日韩文字始终回退到系统字体。',
    appearanceFontBundled: '内置 · Newsreader + JetBrains Mono',
    appearanceFontBundledHint: '默认 · 约 400 KB Latin 子集',
    appearanceFontSystem: '仅使用系统字体',
    appearanceFontSystemHint: 'Georgia / SF Mono / 系统无衬线',
    appearanceDensity: '密度',
    appearanceDensityComfortable: '舒适',
    appearanceDensityCompact: '紧凑',
    appearancePaperTexture: '纸张质感',
    appearancePaperTextureHelp:
      '极轻的纸纹遮罩和深色模式下的暗房光晕。关闭即可得到纯平表面。',
    appearancePaperOn: '已开启',
    appearancePaperOff: '已关闭',
    appearanceClockLabel: '时间格式',
    appearanceClockHelp:
      '除了图表轴线之外，所有时间均按此格式显示。图表始终用 24 小时制以保持紧凑。',
    appearanceClock12h: '12 小时制（下午 3:14）',
    appearanceClock24h: '24 小时制（15:14）',
    // ── 链接预览（og:image 缓存）──
    linkPreviewsTitle: '链接预览',
    linkPreviewsIntro:
      '卡片模式的 Browse 会在能取到 og:image 时抓取页面预览。字节缓存在本地、内容相同则只存一份；除了访问页面本身的 GET 之外，不会把数据传出本机。',
    linkPreviewsFetchToggleLabel: '从网络抓取链接预览',
    linkPreviewsFetchToggleHint:
      '关闭后，卡片模式会退回 favicon 或域名色块；已经缓存的预览依然会显示。',
    linkPreviewsFetchOn: '抓取已开启',
    linkPreviewsFetchOff: '抓取已关闭',
    linkPreviewsFetchModeLabel: '抓取策略',
    linkPreviewsFetchModeHint:
      '"后台" 模式：每次备份后扫描新访问 URL 预抓 + 重试暂时性失败。"按需" 仅在你滚到卡片时才抓。"关闭" 暂停抓取但保留缓存。',
    linkPreviewsFetchModeOff: '关闭',
    linkPreviewsFetchModeOffHint: '完全不抓取。',
    linkPreviewsFetchModeOnDemand: '按需',
    linkPreviewsFetchModeOnDemandHint: '只在卡片滚入视口时抓取。',
    linkPreviewsFetchModeBackground: '后台',
    linkPreviewsFetchModeBackgroundHint:
      '按需 + 每次备份预抓 + 每日重试。推荐。',
    linkPreviewsBudgetsLabel: '每次备份预算',
    linkPreviewsBudgetsHint:
      '限制每日重试和新访问预抓单次入队的 URL 数量上限，避免短时间内大量对外请求。设为 0 即停用该项。',
    linkPreviewsDailyRefetchBudgetLabel: '每日重试上限',
    linkPreviewsPrefetchBudgetLabel: '每次备份预抓上限',
    linkPreviewsRebuildAction: '立即重建 ({budget})',
    linkPreviewsRebuildHint:
      '扫描最近访问且尚未有预览的 URL，最多 {budget} 条（worker 单次硬上限 {cap}）。',
    linkPreviewsRebuildSummary: '入队 {enqueued} 条，成功 {succeeded} 条。',
    linkPreviewsStatsLabel: '缓存大小',
    linkPreviewsStatsRows: '{rows} 行 · {blobs} 个文件 · {bytes}',
    linkPreviewsStatsEmpty: '尚未缓存任何预览。',
    linkPreviewsCoverageLabel: '图片覆盖率',
    linkPreviewsCoverageValue:
      '{percent}% 的页面有预览图（{withImage} / {eligible}）',
    linkPreviewsCoverageEmpty: '暂无可统计的页面。',
    linkPreviewsCoverageLoading: '正在统计覆盖率…',
    linkPreviewsCoverageRate: '已检查 {checked} 个页面，{rate}% 有预览图。',
    linkPreviewsCoverageError: '无法统计覆盖率。',
    linkPreviewsCoverageNotFetched: '尚未抓取预览图，浏览时会在后台自动抓取。',
    linkPreviewsCleanupLabel: '维护',
    linkPreviewsRunCleanupAction: '立即清理',
    linkPreviewsClearAllAction: '清空所有链接预览',
    linkPreviewsClearConfirm:
      '删除所有缓存的链接预览？只能通过重新抓取每个页面恢复。',
    linkPreviewsCleanupSummary:
      '已删除 {rows} 行、{blobs} 个文件，释放 {bytes}。',
    linkPreviewsBlocklistLabel: '域名屏蔽列表',
    linkPreviewsBlocklistHint:
      '一行一个域名，列表内域名的页面将不抓取链接预览。以 # 开头的行视为注释。',
    linkPreviewsBlocklistPlaceholder:
      'example.com\nbanking.example.org\n# 公司内网\nintranet.example.local',
    linkPreviewsCleanupModeLabel: '清理策略',
    linkPreviewsCleanupModeHint:
      '"关闭" 保留全部预览；选择其他模式时 PathKeep 会在每日维护时自动按规则清理。',
    linkPreviewsCleanupModeOff: '关闭',
    linkPreviewsCleanupModeOffHint: '缓存不限制增长。',
    linkPreviewsCleanupModeTimeTtl: '按时长',
    linkPreviewsCleanupModeTimeTtlHint: '删除早于阈值的行。',
    linkPreviewsCleanupModeSizeCap: '按总量',
    linkPreviewsCleanupModeSizeCapHint:
      '按抓取时间最早的优先删除，直到总量低于阈值。',
    linkPreviewsCleanupModeLru: 'LRU',
    linkPreviewsCleanupModeLruHint:
      '按最近一次显示的时间最旧的优先删除，直到总量低于阈值。',
    linkPreviewsMaxAgeDaysLabel: '最大保留',
    linkPreviewsMaxAgeDaysUnit: '天',
    linkPreviewsMaxBytesLabel: '总量上限',
    linkPreviewsMaxBytesUnit: 'MB',
    openMaintenanceBody:
      '更新、清理、派生数据重建、远程备份流程、诊断和平台排障。',
    maintenanceTitle: '维护',
    maintenanceBody:
      '高级维护操作集中在这里。先预览破坏性清理，跟踪更新状态，重建派生数据，并查看支持诊断，而不挤占日常偏好设置。',
    maintenanceUnavailableTitle: '维护暂时不可用',
    integrationsTitle: '集成',
    integrationsBody:
      '在把载荷和生成文件交给受信任本地工具前，先在这里检查。原始 JSON 和代码会限制在可滚动的复核面板里。',
    integrationsUnavailableTitle: '集成暂时不可用',
    openJobsBody: '后台任务仍然负责运行进度、重试、取消和日志。',
    backToSettingsBody:
      '回到语言、浏览器、隐私、AI 服务和已保存备份配置等日常偏好。',
    diagnosticsTitle: '支持诊断',
    diagnosticsBody:
      '检查日志、崩溃报告或本地支持产物时，可以使用这些路径和构建信息。',
    retentionTitle: '保留与清理',
    retentionDescription:
      '先检查哪些本地文件现在可以清理。PathKeep 只有在你明确执行时才会删除这些工件。',
    retentionSelected: '已选择 {size}',
    retentionUnlockTitle: '清理前请先解锁存档',
    retentionUnlockBody:
      '删除快照时还要同步更新审计记录，所以 PathKeep 需要先解锁存档数据库。',
    retentionSnapshots: '保存的快照',
    retentionExports: '本地导出',
    retentionStaging: '暂存文件',
    retentionQuarantine: '隔离文件',
    retentionItems: '项',
    retentionLoadingTitle: '正在加载清理预览',
    retentionRefresh: '刷新预览',
    retentionExecute: '清理所选内容',
    retentionDeletedBytes: '已删除 {size} 的本地工件。',
    retentionDeletedFiles: '已移除 {count} 个文件或目录。',
    retentionOpenAudit: '打开清理复核',
    retentionNothingSelected: '请至少选择一个可清理项。',
    retentionSnapshotPruneWarning:
      '清理快照会移除以后在审计页可用的恢复检查点。Manifest 和运行摘要会保留。',
    retentionExportPruneWarning:
      '清理导出只会删除 PathKeep 数据目录下的本地文件，远端对象不会变化。',
    appLock: '应用锁',
    appLockBoundaryTitle: '仅保护当前会话',
    appLockBoundaryBody:
      '应用锁只保护桌面 UI 会话。归档加密仍然是独立的静态数据保护层，共享浏览器范围也仍只是筛选条件，不是单独分区。',
    appLockEnabled: '启用应用锁',
    appLockStatus: '状态',
    appLockStatusLocked: '已锁定',
    appLockStatusUnlocked: '已解锁',
    appLockIdleTimeout: '闲置超时',
    appLockMinutes: '{count} 分钟',
    appLockBiometric: '在可用时允许生物识别解锁',
    appLockTouchId: '在可用时允许 Touch ID 解锁',
    appLockBiometricUnavailable:
      '当前构建尚未接入生物识别，所以仍然必须使用密码解锁。',
    appLockTouchIdUnavailable:
      '这台 Mac 当前无法使用 Touch ID，所以仍然必须使用密码解锁。',
    appLockTouchIdAvailable:
      '这台 Mac 可以使用 Touch ID 解锁当前 PathKeep 会话。',
    appLockRecoveryHint: '恢复提示',
    appLockRecoveryHintPlaceholder: '锁定页面上显示的可选提示',
    appLockPasscode: '密码',
    appLockPasscodePlaceholder: '设置或替换当前应用锁密码',
    appLockSetPasscode: '保存密码',
    appLockUpdatePasscode: '更新密码',
    appLockSavingPasscode: '正在保存密码…',
    appLockClearPasscode: '清除密码',
    appLockClearingPasscode: '正在清除密码…',
    appLockLockNow: '立即锁定',
    appLockLockingNow: '正在锁定…',
    appLockNeedsPasscodeTitle: '请先设置密码',
    appLockNeedsPasscodeBody:
      '启用应用锁前，必须先保存密码。PathKeep 不会把一个未配置完成的开关当成真实安全保护。',
    appLockConfigPath: '配置路径',
    appLockLastUnlocked: '上次解锁',
    archiveSecurity: '存档密钥',
    baseUrlLabel: 'Base URL',
    embeddingModelLabel: '向量模型',
    llmModelLabel: '对话模型',
    apiKeyLabel: 'API 密钥',
    interfaceLanguage: '语言',
    currentLanguage: '当前',
    explorerBackgroundPrefetchPages: 'Explorer 后台预取',
    explorerBackgroundPrefetchDisabled: '关闭',
    explorerBackgroundPrefetchOption: '每侧 {count} 页',
    explorerBackgroundPrefetchBody:
      '当前 Explorer 页面显示出来后，PathKeep 可以在后台预热附近页面。数值越高，上一页和下一页切换会更顺，但也会增加后台读取。',
    dataDirectory: '数据文件夹',
    archiveDatabase: '存档数据库',
    auditRepository: '审计日志',
    logsDirectory: '日志文件夹',
    revealLogsButton: '在访达中显示日志',
    revealLogsAriaLabel: '在文件管理器中打开 PathKeep 日志文件夹',
    crashReports: '崩溃报告',
    openDirectory: '打开文件夹',
    openCrashReport: '打开崩溃报告',
    mcpServer: 'MCP 服务',
    version: '应用版本',
    gitCommit: 'Git 哈希',
    latestCrashTitle: '检测到最近的崩溃报告',
    latestCrashBody: '{source} 记录于 {time}。最新消息：{message}',
    latestCrashSourceRust: 'Rust panic',
    latestCrashSourceFrontend: '前端崩溃',
    latestCrashClearTitle: '最近没有崩溃报告',
    latestCrashClearBody:
      'PathKeep 在当前数据目录里还没有记录到最近的 panic 或未捕获前端故障。',
  },
  'zh-TW': {
    loadingSettings: '載入設定…',
    loadingModules: '載入中…',
    archiveUnlockTitle: '請先解鎖封存再檢查設定',
    archiveUnlockBody:
      'PathKeep 仍然可以檢查定時備份和鑰匙圈狀態，但其餘設定需要先提供封存會話金鑰。請先到安全頁面解鎖。',
    unavailableTitle: '設定暫時無法使用',
    unavailableBody:
      'PathKeep 目前無法載入這組設定檢查畫面。等主介面刷新完成後再試一次。',
    browserProfiles: '瀏覽器',
    browserProfilesBody: '選擇要備份的瀏覽器。只有勾選的瀏覽器會被納入封存。',
    aiProvider: 'AI 服務',
    aiProviderBody:
      '連接一個本地或自架的 AI 服務，即可與你的歷史對話並開啟智慧搜尋。選用功能，預設關閉——關鍵字搜尋和確定性智慧分析不需要它也能使用。',
    optional: '可選',
    savedConfirmation: '已儲存',
    groupCore: '核心',
    groupLookFeel: '外觀與感受',
    groupDataSources: '資料來源',
    groupData: '資料',
    groupDisplay: '顯示',
    groupDataUpdates: '資料與更新',
    groupSecurityAccess: '安全與存取',
    groupIntelligence: '智慧',
    groupBackupSync: '備份與同步',
    groupPlatform: '平台',
    groupPrivacyAccess: '隱私與存取',
    groupMaintenance: '更新與清理',
    groupDerivedData: '派生資料',
    groupDiagnostics: '診斷',
    groupExternalOutputs: '外部輸出',
    groupGeneratedArtifacts: '生成產物',
    externalReviewBadge: '僅複核',
    general: '一般',
    generalDescription: '語言和 Explorer 效能偏好。',
    paperHeaderEyebrow: '偏好設定',
    paperHeaderTitle: '在閱讀之前，先安頓好這一頁。',
    paperHeaderSubtitle: '您的閱讀環境與檔案連接設定。所有變更都會自動儲存。',
    paperJumpLabel: '跳轉到',
    appearanceTitle: '外觀',
    appearanceIntro:
      '選擇紙面（亮色）或暗房（深色），調整像書一般的字體，再決定要不要在墨水之下保留紙張的質感。',
    appearanceTheme: '主題',
    appearanceThemeLight: '紙面 · 亮色',
    appearanceThemeDark: '暗房 · 深色',
    appearanceFonts: '字體',
    appearanceFontsHelp:
      '內建字體支援離線執行；系統字體會略過內建包，直接使用系統已有的字體。中日韓文字始終回退到系統字體。',
    appearanceFontBundled: '內建 · Newsreader + JetBrains Mono',
    appearanceFontBundledHint: '預設 · 約 400 KB Latin 子集',
    appearanceFontSystem: '僅使用系統字體',
    appearanceFontSystemHint: 'Georgia / SF Mono / 系統無襯線',
    appearanceDensity: '密度',
    appearanceDensityComfortable: '舒適',
    appearanceDensityCompact: '緊湊',
    appearancePaperTexture: '紙張質感',
    appearancePaperTextureHelp:
      '極輕的紙紋遮罩與深色模式下的暗房光暈。關閉即可得到純平表面。',
    appearancePaperOn: '已開啟',
    appearancePaperOff: '已關閉',
    appearanceClockLabel: '時間格式',
    appearanceClockHelp:
      '除了圖表軸線之外，所有時間均依此格式顯示。圖表保留 24 小時制以維持緊湊。',
    appearanceClock12h: '12 小時制（下午 3:14）',
    appearanceClock24h: '24 小時制（15:14）',
    // ── 連結預覽（og:image 快取）──
    linkPreviewsTitle: '連結預覽',
    linkPreviewsIntro:
      '卡片模式的 Browse 會在能取到 og:image 時擷取頁面預覽。位元組快取在本機、內容相同就只存一份；除了訪問頁面本身的 GET 之外，不會把資料傳出本機。',
    linkPreviewsFetchToggleLabel: '從網路擷取連結預覽',
    linkPreviewsFetchToggleHint:
      '關閉之後，卡片模式會退回 favicon 或網域色塊；已經快取的預覽仍會顯示。',
    linkPreviewsFetchOn: '擷取已開啟',
    linkPreviewsFetchOff: '擷取已關閉',
    linkPreviewsFetchModeLabel: '擷取策略',
    linkPreviewsFetchModeHint:
      '「背景」模式：每次備份完掃描新訪問 URL 預抓 + 重試暫時性失敗。「按需」只在你滑到卡片時才抓。「關閉」暫停擷取但保留快取。',
    linkPreviewsFetchModeOff: '關閉',
    linkPreviewsFetchModeOffHint: '完全不擷取。',
    linkPreviewsFetchModeOnDemand: '按需',
    linkPreviewsFetchModeOnDemandHint: '只在卡片滑入視口時擷取。',
    linkPreviewsFetchModeBackground: '背景',
    linkPreviewsFetchModeBackgroundHint:
      '按需 + 每次備份預抓 + 每日重試。推薦。',
    linkPreviewsBudgetsLabel: '每次備份預算',
    linkPreviewsBudgetsHint:
      '限制每日重試和新訪問預抓單次入佇列的 URL 數量上限，避免短時間內大量對外請求。設為 0 即停用該項。',
    linkPreviewsDailyRefetchBudgetLabel: '每日重試上限',
    linkPreviewsPrefetchBudgetLabel: '每次備份預抓上限',
    linkPreviewsRebuildAction: '立即重建 ({budget})',
    linkPreviewsRebuildHint:
      '掃描最近訪問且尚未有預覽的 URL，最多 {budget} 條（worker 單次硬上限 {cap}）。',
    linkPreviewsRebuildSummary: '入佇列 {enqueued} 條，成功 {succeeded} 條。',
    linkPreviewsStatsLabel: '快取大小',
    linkPreviewsStatsRows: '{rows} 列 · {blobs} 個檔案 · {bytes}',
    linkPreviewsStatsEmpty: '尚未快取任何預覽。',
    linkPreviewsCoverageLabel: '圖片覆蓋率',
    linkPreviewsCoverageValue:
      '{percent}% 的頁面有預覽圖（{withImage} / {eligible}）',
    linkPreviewsCoverageEmpty: '暫無可統計的頁面。',
    linkPreviewsCoverageLoading: '正在統計覆蓋率…',
    linkPreviewsCoverageRate: '已檢查 {checked} 個頁面，{rate}% 有預覽圖。',
    linkPreviewsCoverageError: '無法統計覆蓋率。',
    linkPreviewsCoverageNotFetched: '尚未擷取預覽圖，瀏覽時會在背景自動擷取。',
    linkPreviewsCleanupLabel: '維護',
    linkPreviewsRunCleanupAction: '立即清理',
    linkPreviewsClearAllAction: '清空所有連結預覽',
    linkPreviewsClearConfirm:
      '刪除所有快取的連結預覽？只能透過重新擷取每個頁面恢復。',
    linkPreviewsCleanupSummary:
      '已刪除 {rows} 列、{blobs} 個檔案，釋放 {bytes}。',
    linkPreviewsBlocklistLabel: '網域封鎖列表',
    linkPreviewsBlocklistHint:
      '一行一個網域，名單內網域的頁面不會抓取連結預覽。以 # 開頭的行視為註解。',
    linkPreviewsBlocklistPlaceholder:
      'example.com\nbanking.example.org\n# 公司內網\nintranet.example.local',
    linkPreviewsCleanupModeLabel: '清理策略',
    linkPreviewsCleanupModeHint:
      '「關閉」保留全部預覽；選擇其他模式時 PathKeep 會在每日維護時依規則自動清理。',
    linkPreviewsCleanupModeOff: '關閉',
    linkPreviewsCleanupModeOffHint: '快取不限增長。',
    linkPreviewsCleanupModeTimeTtl: '按時長',
    linkPreviewsCleanupModeTimeTtlHint: '刪除早於閾值的列。',
    linkPreviewsCleanupModeSizeCap: '按總量',
    linkPreviewsCleanupModeSizeCapHint:
      '依擷取時間最早優先刪除，直到總量低於閾值。',
    linkPreviewsCleanupModeLru: 'LRU',
    linkPreviewsCleanupModeLruHint:
      '依最近一次顯示時間最久遠優先刪除，直到總量低於閾值。',
    linkPreviewsMaxAgeDaysLabel: '最長保留',
    linkPreviewsMaxAgeDaysUnit: '天',
    linkPreviewsMaxBytesLabel: '總量上限',
    linkPreviewsMaxBytesUnit: 'MB',
    openMaintenanceBody:
      '更新、清理、派生資料重建、遠端備份流程、診斷和平台排障。',
    maintenanceTitle: '維護',
    maintenanceBody:
      '進階維護操作集中在這裡。先預覽破壞性清理，追蹤更新狀態，重建派生資料，並查看支援診斷，而不擠占日常偏好設定。',
    maintenanceUnavailableTitle: '維護暫時無法使用',
    integrationsTitle: '整合',
    integrationsBody:
      '在把載荷和生成檔案交給受信任本地工具前，先在這裡檢查。原始 JSON 和程式碼會限制在可捲動的複核面板裡。',
    integrationsUnavailableTitle: '整合暫時無法使用',
    openJobsBody: '背景工作仍然負責執行進度、重試、取消和日誌。',
    backToSettingsBody:
      '回到語言、瀏覽器、隱私、AI 服務和已保存備份設定等日常偏好。',
    diagnosticsTitle: '支援診斷',
    diagnosticsBody:
      '檢查日誌、崩潰報告或本地支援產物時，可以使用這些路徑和建置資訊。',
    retentionTitle: '保留與清理',
    retentionDescription:
      '先檢查哪些本地檔案現在可以清理。PathKeep 只有在你明確執行時才會刪除這些工件。',
    retentionSelected: '已選擇 {size}',
    retentionUnlockTitle: '清理前請先解鎖封存',
    retentionUnlockBody:
      '刪除快照時也要同步更新稽核記錄，所以 PathKeep 需要先解鎖封存資料庫。',
    retentionSnapshots: '保存的快照',
    retentionExports: '本地匯出',
    retentionStaging: '暫存檔案',
    retentionQuarantine: '隔離檔案',
    retentionItems: '項',
    retentionLoadingTitle: '正在載入清理預覽',
    retentionRefresh: '重新整理預覽',
    retentionExecute: '清理所選內容',
    retentionDeletedBytes: '已刪除 {size} 的本地工件。',
    retentionDeletedFiles: '已移除 {count} 個檔案或目錄。',
    retentionOpenAudit: '打開清理複核',
    retentionNothingSelected: '請至少選擇一個可清理項目。',
    retentionSnapshotPruneWarning:
      '清理快照會移除未來在稽核頁可用的復原檢查點。Manifest 與執行摘要會保留。',
    retentionExportPruneWarning:
      '清理匯出只會刪除 PathKeep 資料目錄下的本地檔案，遠端物件不會變更。',
    appLock: '應用鎖',
    appLockBoundaryTitle: '僅保護目前會話',
    appLockBoundaryBody:
      '應用鎖只保護桌面 UI 會話。封存加密仍然是獨立的靜態資料保護層，共享瀏覽器範圍也仍只是篩選條件，不是獨立分區。',
    appLockEnabled: '啟用應用鎖',
    appLockStatus: '狀態',
    appLockStatusLocked: '已鎖定',
    appLockStatusUnlocked: '已解鎖',
    appLockIdleTimeout: '閒置逾時',
    appLockMinutes: '{count} 分鐘',
    appLockBiometric: '可用時允許生物辨識解鎖',
    appLockTouchId: '可用時允許 Touch ID 解鎖',
    appLockBiometricUnavailable:
      '目前建置尚未接上生物辨識，所以仍然必須使用密碼解鎖。',
    appLockTouchIdUnavailable:
      '這台 Mac 目前無法使用 Touch ID，所以仍然必須使用密碼解鎖。',
    appLockTouchIdAvailable:
      '這台 Mac 可以使用 Touch ID 解鎖目前的 PathKeep 會話。',
    appLockRecoveryHint: '恢復提示',
    appLockRecoveryHintPlaceholder: '鎖定畫面上顯示的可選提示',
    appLockPasscode: '密碼',
    appLockPasscodePlaceholder: '設定或替換目前的應用鎖密碼',
    appLockSetPasscode: '儲存密碼',
    appLockUpdatePasscode: '更新密碼',
    appLockSavingPasscode: '正在儲存密碼…',
    appLockClearPasscode: '清除密碼',
    appLockClearingPasscode: '正在清除密碼…',
    appLockLockNow: '立即鎖定',
    appLockLockingNow: '正在鎖定…',
    appLockNeedsPasscodeTitle: '請先設定密碼',
    appLockNeedsPasscodeBody:
      '啟用應用鎖前，必須先儲存密碼。PathKeep 不會把一個尚未設定完成的開關當成真正的安全保護。',
    appLockConfigPath: '設定路徑',
    appLockLastUnlocked: '上次解鎖',
    archiveSecurity: '封存密鑰',
    baseUrlLabel: 'Base URL',
    embeddingModelLabel: '向量模型',
    llmModelLabel: '對話模型',
    apiKeyLabel: 'API 金鑰',
    interfaceLanguage: '語言',
    currentLanguage: '目前',
    explorerBackgroundPrefetchPages: 'Explorer 背景預取',
    explorerBackgroundPrefetchDisabled: '關閉',
    explorerBackgroundPrefetchOption: '每側 {count} 頁',
    explorerBackgroundPrefetchBody:
      '目前的 Explorer 頁面顯示出來後，PathKeep 可以在背景預熱附近頁面。數值越高，上一頁和下一頁切換會更順，但也會增加背景讀取。',
    dataDirectory: '資料夾',
    archiveDatabase: '封存資料庫',
    auditRepository: '稽核日誌',
    logsDirectory: '日誌資料夾',
    revealLogsButton: '在 Finder 中顯示日誌',
    revealLogsAriaLabel: '在檔案管理器中開啟 PathKeep 日誌資料夾',
    crashReports: '崩潰報告',
    openDirectory: '開啟資料夾',
    openCrashReport: '開啟崩潰報告',
    mcpServer: 'MCP 服務',
    version: '應用版本',
    gitCommit: 'Git 雜湊',
    latestCrashTitle: '偵測到最近的崩潰報告',
    latestCrashBody: '{source} 記錄於 {time}。最新訊息：{message}',
    latestCrashSourceRust: 'Rust panic',
    latestCrashSourceFrontend: '前端崩潰',
    latestCrashClearTitle: '最近沒有崩潰報告',
    latestCrashClearBody:
      'PathKeep 在目前資料目錄裡尚未記錄到最近的 panic 或未攔截前端故障。',
  },
} as const
