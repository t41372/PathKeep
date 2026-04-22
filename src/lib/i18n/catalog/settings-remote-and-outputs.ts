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
    remoteBackup: 'CLOUD BACKUP',
    s3Compatible: 'S3-COMPATIBLE',
    remoteBackupSummary: 'Upload your archive to cloud storage',
    remoteBackupBody:
      'Preview what will upload, review the details, upload, then verify the backup can be restored.',
    remoteEnabled: 'Enable cloud backup',
    bucketLabel: 'Bucket',
    regionLabel: 'Region',
    endpointLabel: 'Custom endpoint',
    endpointPlaceholder: 'https://s3.example.com',
    prefixLabel: 'Path prefix',
    pathStyleLabel: 'Use path-style URLs',
    uploadAfterBackup: 'Auto-upload after each backup',
    saveRemoteSettings: 'Save',
    previewRemoteBackup: 'Preview upload',
    executeRemoteBackup: 'Upload now',
    verifyRemoteBackup: 'Verify backup',
    credentialsStatus: 'Credentials',
    credentialsSaved: 'Saved',
    credentialsMissing: 'Not saved yet',
    lastUploadedAt: 'Last upload',
    remoteNoUploadYet: 'No upload yet.',
    accessKeyId: 'Access key ID',
    secretAccessKey: 'Secret access key',
    storeRemoteCredentials: 'Save credentials',
    clearRemoteCredentials: 'Remove credentials',
    remotePme: 'UPLOAD WORKFLOW',
    savingRemoteSettings: 'Saving…',
    storingRemoteCredentials: 'Saving credentials…',
    clearingRemoteCredentials: 'Removing credentials…',
    previewingRemoteBackup: 'Generating preview…',
    executingRemoteBackup: 'Uploading…',
    verifyingRemoteBackup: 'Verifying…',
    previewBoundaryTitle: 'PREVIEW',
    previewBoundaryBody:
      'Review the file path, destination, and any warnings before anything is sent.',
    previewBoundaryReady:
      'Preview ready. Review the details below, then upload when ready.',
    manualBoundaryTitle: 'MANUAL UPLOAD',
    manualBoundaryBody:
      'Use this command to upload manually. Keep the restore steps with it.',
    previewCommand: 'Upload command',
    retentionGuidance:
      'Cleanup and retry are manual for now. Set up bucket lifecycle rules once you are comfortable with the backup format.',
    previewFirstTitle: 'Preview first',
    previewFirstBody:
      'Generate a preview before uploading or using the manual upload command.',
    executeBoundaryTitle: 'UPLOAD',
    executeBoundaryBody:
      "PathKeep will upload after you've reviewed the preview and saved your credentials.",
    executeMessage: 'Result',
    executeNotRunTitle: 'Not uploaded yet',
    executeNotRunBody:
      'Save your settings and credentials, then preview before uploading.',
    verifyBoundaryTitle: 'VERIFY',
    verifyBoundaryBody:
      'Checks that the uploaded backup is complete and can be restored.',
    bundlePath: 'File path',
    objectKey: 'Object key',
    uploadUrl: 'Destination',
    bundleVersion: 'Format version',
    restoreReady: 'Restorable',
    verifyNotRunTitle: 'Nothing to verify yet',
    verifyNotRunBody: "Upload a backup first so there's something to verify.",
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
  },
  'zh-CN': {
    remoteBackup: '云端备份',
    s3Compatible: 'S3 兼容',
    remoteBackupSummary: '将存档上传到云端存储',
    remoteBackupBody:
      '先预览会上传什么，再检查、上传，最后确认这份备份可以恢复。',
    remoteEnabled: '启用云端备份',
    bucketLabel: 'Bucket',
    regionLabel: 'Region',
    endpointLabel: '自定义地址',
    endpointPlaceholder: 'https://s3.example.com',
    prefixLabel: '路径前缀',
    pathStyleLabel: '使用 path-style URL',
    uploadAfterBackup: '每次备份后自动上传',
    saveRemoteSettings: '保存',
    previewRemoteBackup: '预览上传',
    executeRemoteBackup: '上传',
    verifyRemoteBackup: '验证备份',
    credentialsStatus: '凭证',
    credentialsSaved: '已保存',
    credentialsMissing: '未保存',
    lastUploadedAt: '上次上传',
    remoteNoUploadYet: '还没有上传过。',
    accessKeyId: 'Access key ID',
    secretAccessKey: 'Secret access key',
    storeRemoteCredentials: '保存凭证',
    clearRemoteCredentials: '删除凭证',
    remotePme: '上传流程',
    savingRemoteSettings: '保存中…',
    storingRemoteCredentials: '保存凭证…',
    clearingRemoteCredentials: '删除凭证…',
    previewingRemoteBackup: '生成预览…',
    executingRemoteBackup: '上传中…',
    verifyingRemoteBackup: '验证中…',
    previewBoundaryTitle: '预览',
    previewBoundaryBody: '上传前先检查文件路径、目标地址和注意事项。',
    previewBoundaryReady: '预览已生成。确认后点击上传。',
    manualBoundaryTitle: '手动上传',
    manualBoundaryBody: '可以用下方命令手动上传，记得把恢复步骤一起保存。',
    previewCommand: '上传命令',
    retentionGuidance:
      '目前清理和重试都需要手动操作。熟悉备份格式后，再设置自动清理规则。',
    previewFirstTitle: '请先预览',
    previewFirstBody: '先生成预览，再进行上传操作。',
    executeBoundaryTitle: '上传',
    executeBoundaryBody: '确认设置和凭证无误后再上传。',
    executeMessage: '结果',
    executeNotRunTitle: '还没有上传',
    executeNotRunBody: '先保存设置和凭证，然后预览确认后再上传。',
    verifyBoundaryTitle: '验证',
    verifyBoundaryBody: '验证上传的备份是否完整且可以恢复。',
    bundlePath: '文件路径',
    objectKey: 'Object key',
    uploadUrl: '目标地址',
    bundleVersion: '格式版本',
    restoreReady: '可恢复',
    verifyNotRunTitle: '没有可验证的内容',
    verifyNotRunBody: '先上传一次备份，才能验证。',
    externalOutputsTitle: '外部输出',
    externalOutputsManualBadge: '仅手动',
    externalOutputsSummaryTitle:
      '先检查 Core Intelligence 输出，再带到别处使用',
    externalOutputsSummaryBody:
      '在这里预览 embed cards、widget snapshot 和 public snapshot，然后把需要的 payload 手动复制到你信任的本地宿主。',
    externalOutputsScopedTitle: '沿用共享 profile 范围',
    externalOutputsScopedBody:
      '这些预览现在只会读取 {profile}。如果你想看 archive-wide 的输出 payload，请先清除 shell 顶部的共享 profile scope。',
    externalOutputsArchiveWideTitle: '当前是 archive-wide 预览',
    externalOutputsArchiveWideBody:
      '这些预览会读取整个当前可见存档。如果你想只看某个 profile 的输出，请先在 shell 里切换共享 profile scope。',
    externalOutputsNeedsArchiveTitle: '先创建存档，才能检查手动输出',
    externalOutputsNeedsArchiveBody:
      'PathKeep 需要先完成 archive 初始化，才能生成 embed cards、widget snapshots 和 public snapshots。',
    externalOutputsUnlockTitle: '先解锁存档，才能检查手动输出',
    externalOutputsUnlockBody:
      '只有当前 archive session 处于解锁状态时，手动输出预览才会加载。',
    externalOutputsManualOnlyTitle: '仅支持手动复制 / 导出',
    externalOutputsManualOnlyBody:
      '这里不会安装小组件、发布 localhost API，也不会保存可复用的宿主产物。请先检查 payload，再手动复制到你信任的本地 surface。',
    externalOutputsTabEmbed: 'Embed cards',
    externalOutputsTabWidget: 'Widget snapshot',
    externalOutputsTabPublic: 'Public snapshot',
    externalOutputsLoading: '正在加载手动输出预览',
    externalOutputsUnavailableTitle: '手动输出暂时不可用',
    externalOutputsUnavailableBody:
      'PathKeep 现在无法加载这组手动输出预览。等 shell 完成刷新后再试一次。',
    externalOutputsEmbedPreviewTitle: 'Embed card 预览',
    externalOutputsTrustedOnlyBadge: '仅限受信任宿主',
    externalOutputsHref: 'Payload href',
    externalOutputsOpenInsights: '打开洞察',
    externalOutputsEmbedEmpty: '这个范围里暂时没有可用的 embed cards。',
    externalOutputsJsonTitle: '原始 JSON payload',
    externalOutputsWidgetPreviewTitle: 'Widget snapshot 预览',
    externalOutputsWindowLabel: '时间范围：{start} → {end}',
    externalOutputsWidgetTrustedTitle: '需要受信任宿主审查',
    externalOutputsWidgetTrustedBody:
      '这个 widget snapshot 仍包含标记为 trusted-only 的卡片。请把它留在受信任的 PathKeep 控制宿主里，不要把它当成公开导出内容。',
    externalOutputsPublicPreviewTitle: 'Public snapshot 预览',
    externalOutputsPublicRedactedTitle: 'Public snapshot 会保持脱敏',
    externalOutputsPublicRedactedBody:
      '这个 payload 会刻意省略 visit ID 和直接页面 URL，这样在离开受信任的 PathKeep surface 时会更安全。',
    externalOutputsTopDomains: 'Top domains',
    externalOutputsSearchEngines: '搜索引擎',
    externalOutputsNoSearchEngines: '这个时间范围里没有可用的搜索引擎活动。',
    externalOutputsDiscoveryTrend: '发现趋势',
    externalOutputsNoDiscoveryTrend: '这个时间范围里没有可用的发现趋势点。',
    externalOutputsCopyFailed:
      '这份 payload 无法直接复制，请改为手动从 JSON 区块复制。',
    externalOutputsLocalHostTitle: '受信任本地宿主',
    externalOutputsLocalHostBadge: '仅限本地',
    externalOutputsLocalHostSummaryTitle: '可复用的浏览器片段',
    externalOutputsLocalHostSummaryBody:
      '在 app data 目录下生成一个可直接打开的本地浏览器片段。它会沿用上面手动预览同一套共享 profile 范围和本地时间窗口。',
    externalOutputsLocalHostLoading: '正在加载本地宿主预览',
    externalOutputsLocalHostUnavailableTitle: '本地宿主预览暂时不可用',
    externalOutputsLocalHostUnavailableBody:
      'PathKeep 现在无法准备这组本地宿主预览。等 shell 完成刷新后再试一次。',
    externalOutputsLocalHostPreviewTitle: '预览',
    externalOutputsLocalHostPreviewBody:
      'PathKeep 会在 {path} 写入或更新这个受信任的本地片段。先检查生成文件，再决定是否创建它。',
    externalOutputsLocalHostBoundaryTitle: '边界说明',
    externalOutputsLocalHostWarningsTitle: '警告',
    externalOutputsLocalHostManualTitle: '手动检查',
    externalOutputsLocalHostExecuteTitle: '创建或更新本地片段',
    externalOutputsLocalHostExecuteBody:
      '这个动作会把 index.html 和 bundle.json 写入固定的本地宿主目录。只要 scope、时间窗口或语言变了，就应该重新生成。',
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
      '这个本地宿主只使用 deterministic Core Intelligence read models。',
    externalOutputsLocalHostBoundaryTrusted:
      'trusted-only 卡片必须留在 PathKeep 控制的本地 surface 内。',
    externalOutputsLocalHostManualReview:
      '先检查 index.html 和 bundle.json，再把这个文件夹交给其他受信任的本地工具。',
    externalOutputsLocalHostManualOpen:
      '直接从这个文件夹打开 index.html，在受信任的本地浏览器宿主里查看它。',
    externalOutputsLocalHostManualRebuild:
      '只要 scope、时间窗口或语言发生变化，就重新创建这个本地片段。',
    externalOutputsLocalHostWarningTrusted:
      '这个本地片段包含 trusted-only 卡片，不能把它当成公开导出。',
    externalOutputsLocalHostPurposeEntry:
      '可直接在本机浏览器打开的 Core Intelligence 片段。',
    externalOutputsLocalHostPurposeBundle:
      '同一份本地宿主数据的机器可读 JSON bundle。',
  },
  'zh-TW': {
    remoteBackup: '雲端備份',
    s3Compatible: 'S3 相容',
    remoteBackupSummary: '將封存上傳到雲端儲存',
    remoteBackupBody:
      '先預覽會上傳什麼，再檢查、上傳，最後確認這份備份可以還原。',
    remoteEnabled: '啟用雲端備份',
    bucketLabel: 'Bucket',
    regionLabel: 'Region',
    endpointLabel: '自訂端點',
    endpointPlaceholder: 'https://s3.example.com',
    prefixLabel: '路徑前綴',
    pathStyleLabel: '使用 path-style URL',
    uploadAfterBackup: '每次備份後自動上傳',
    saveRemoteSettings: '儲存',
    previewRemoteBackup: '預覽上傳',
    executeRemoteBackup: '上傳',
    verifyRemoteBackup: '驗證備份',
    credentialsStatus: '憑證',
    credentialsSaved: '已儲存',
    credentialsMissing: '未儲存',
    lastUploadedAt: '上次上傳',
    remoteNoUploadYet: '還沒有上傳過。',
    accessKeyId: 'Access key ID',
    secretAccessKey: 'Secret access key',
    storeRemoteCredentials: '儲存憑證',
    clearRemoteCredentials: '移除憑證',
    remotePme: '上傳流程',
    savingRemoteSettings: '儲存中…',
    storingRemoteCredentials: '儲存憑證…',
    clearingRemoteCredentials: '移除憑證…',
    previewingRemoteBackup: '產生預覽…',
    executingRemoteBackup: '上傳中…',
    verifyingRemoteBackup: '驗證中…',
    previewBoundaryTitle: '預覽',
    previewBoundaryBody: '上傳前先檢查檔案路徑、目標位址和注意事項。',
    previewBoundaryReady: '預覽已產生。確認後點擊上傳。',
    manualBoundaryTitle: '手動上傳',
    manualBoundaryBody: '可以用下方指令手動上傳，記得把還原步驟一起保存。',
    previewCommand: '上傳指令',
    retentionGuidance:
      '目前清理和重試都需要手動操作。熟悉備份格式後，再設定自動清理規則。',
    previewFirstTitle: '請先預覽',
    previewFirstBody: '先產生預覽，再進行上傳操作。',
    executeBoundaryTitle: '上傳',
    executeBoundaryBody: '確認設定和憑證無誤後再上傳。',
    executeMessage: '結果',
    executeNotRunTitle: '還沒有上傳',
    executeNotRunBody: '先儲存設定和憑證，然後預覽確認後再上傳。',
    verifyBoundaryTitle: '驗證',
    verifyBoundaryBody: '驗證上傳的備份是否完整且可以還原。',
    bundlePath: '檔案路徑',
    objectKey: 'Object key',
    uploadUrl: '目標位址',
    bundleVersion: '格式版本',
    restoreReady: '可還原',
    verifyNotRunTitle: '沒有可驗證的內容',
    verifyNotRunBody: '先上傳一次備份，才能驗證。',
    externalOutputsTitle: '外部輸出',
    externalOutputsManualBadge: '僅手動',
    externalOutputsSummaryTitle:
      '先檢查 Core Intelligence 輸出，再帶到別處使用',
    externalOutputsSummaryBody:
      '在這裡預覽 embed cards、widget snapshot 和 public snapshot，然後把需要的 payload 手動複製到你信任的本地宿主。',
    externalOutputsScopedTitle: '沿用共享 profile 範圍',
    externalOutputsScopedBody:
      '這些預覽現在只會讀取 {profile}。如果你想看 archive-wide 的輸出 payload，請先清除 shell 頂部的共享 profile scope。',
    externalOutputsArchiveWideTitle: '目前是 archive-wide 預覽',
    externalOutputsArchiveWideBody:
      '這些預覽會讀取整個目前可見封存。如果你想只看某個 profile 的輸出，請先在 shell 裡切換共享 profile scope。',
    externalOutputsNeedsArchiveTitle: '先建立封存，才能檢查手動輸出',
    externalOutputsNeedsArchiveBody:
      'PathKeep 需要先完成 archive 初始化，才能產生 embed cards、widget snapshots 和 public snapshots。',
    externalOutputsUnlockTitle: '先解鎖封存，才能檢查手動輸出',
    externalOutputsUnlockBody:
      '只有目前 archive session 保持解鎖時，手動輸出預覽才會載入。',
    externalOutputsManualOnlyTitle: '僅支援手動複製 / 匯出',
    externalOutputsManualOnlyBody:
      '這裡不會安裝小工具、發布 localhost API，也不會儲存可重用的宿主產物。請先檢查 payload，再手動複製到你信任的本地 surface。',
    externalOutputsTabEmbed: 'Embed cards',
    externalOutputsTabWidget: 'Widget snapshot',
    externalOutputsTabPublic: 'Public snapshot',
    externalOutputsLoading: '正在載入手動輸出預覽',
    externalOutputsUnavailableTitle: '手動輸出暫時無法使用',
    externalOutputsUnavailableBody:
      'PathKeep 目前無法載入這組手動輸出預覽。等 shell 完成重新整理後再試一次。',
    externalOutputsEmbedPreviewTitle: 'Embed card 預覽',
    externalOutputsTrustedOnlyBadge: '僅限受信任宿主',
    externalOutputsHref: 'Payload href',
    externalOutputsOpenInsights: '打開洞察',
    externalOutputsEmbedEmpty: '這個範圍裡暫時沒有可用的 embed cards。',
    externalOutputsJsonTitle: '原始 JSON payload',
    externalOutputsWidgetPreviewTitle: 'Widget snapshot 預覽',
    externalOutputsWindowLabel: '時間範圍：{start} → {end}',
    externalOutputsWidgetTrustedTitle: '需要受信任宿主審查',
    externalOutputsWidgetTrustedBody:
      '這個 widget snapshot 仍包含標記為 trusted-only 的卡片。請把它留在受信任的 PathKeep 控制宿主裡，不要把它當成公開匯出內容。',
    externalOutputsPublicPreviewTitle: 'Public snapshot 預覽',
    externalOutputsPublicRedactedTitle: 'Public snapshot 會保持去識別化',
    externalOutputsPublicRedactedBody:
      '這個 payload 會刻意省略 visit ID 和直接頁面 URL，這樣在離開受信任的 PathKeep surface 時會更安全。',
    externalOutputsTopDomains: 'Top domains',
    externalOutputsSearchEngines: '搜尋引擎',
    externalOutputsNoSearchEngines: '這個時間範圍裡沒有可用的搜尋引擎活動。',
    externalOutputsDiscoveryTrend: '發現趨勢',
    externalOutputsNoDiscoveryTrend: '這個時間範圍裡沒有可用的發現趨勢點。',
    externalOutputsCopyFailed:
      '這份 payload 無法直接複製，請改為手動從 JSON 區塊複製。',
    externalOutputsLocalHostTitle: '受信任本地宿主',
    externalOutputsLocalHostBadge: '僅限本地',
    externalOutputsLocalHostSummaryTitle: '可重用的瀏覽器片段',
    externalOutputsLocalHostSummaryBody:
      '在 app data 目錄下產生一個可直接開啟的本地瀏覽器片段。它會沿用上面手動預覽同一套共享 profile 範圍和本地時間視窗。',
    externalOutputsLocalHostLoading: '正在載入本地宿主預覽',
    externalOutputsLocalHostUnavailableTitle: '本地宿主預覽暫時無法使用',
    externalOutputsLocalHostUnavailableBody:
      'PathKeep 目前無法準備這組本地宿主預覽。等 shell 完成重新整理後再試一次。',
    externalOutputsLocalHostPreviewTitle: '預覽',
    externalOutputsLocalHostPreviewBody:
      'PathKeep 會在 {path} 寫入或更新這個受信任的本地片段。先檢查生成檔案，再決定是否建立它。',
    externalOutputsLocalHostBoundaryTitle: '邊界說明',
    externalOutputsLocalHostWarningsTitle: '警告',
    externalOutputsLocalHostManualTitle: '手動檢查',
    externalOutputsLocalHostExecuteTitle: '建立或更新本地片段',
    externalOutputsLocalHostExecuteBody:
      '這個動作會把 index.html 和 bundle.json 寫入固定的本地宿主目錄。只要 scope、時間視窗或語言變了，就應該重新產生。',
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
      '這個本地宿主只使用 deterministic Core Intelligence read models。',
    externalOutputsLocalHostBoundaryTrusted:
      'trusted-only 卡片必須留在 PathKeep 控制的本地 surface 內。',
    externalOutputsLocalHostManualReview:
      '先檢查 index.html 與 bundle.json，再把這個資料夾交給其他受信任的本地工具。',
    externalOutputsLocalHostManualOpen:
      '從這個資料夾直接打開 index.html，在受信任的本地瀏覽器宿主裡檢視它。',
    externalOutputsLocalHostManualRebuild:
      '只要 scope、時間視窗或語言改變，就重新建立這個本地片段。',
    externalOutputsLocalHostWarningTrusted:
      '這個本地片段包含 trusted-only 卡片，不能把它當成公開匯出。',
    externalOutputsLocalHostPurposeEntry:
      '可直接在本機瀏覽器開啟的 Core Intelligence 片段。',
    externalOutputsLocalHostPurposeBundle:
      '同一份本地宿主資料的機器可讀 JSON bundle。',
  },
} as const
