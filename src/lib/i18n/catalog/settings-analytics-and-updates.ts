/**
 * @file settings-analytics-and-updates.ts
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
export const settingsAnalyticsAndUpdatesNamespace = {
  en: {
    analyticsTitle: 'ANALYTICS',
    analyticsBoundaryTitle: 'Opt-in frontend analytics only',
    analyticsBoundaryBody:
      'PathKeep only sends coarse frontend events after you opt in. It never includes archive content, URLs, search queries, profile IDs, run IDs, prompts, or filesystem paths.',
    analyticsEndpointMissingTitle: 'Analytics endpoint not configured',
    analyticsEndpointMissingBody:
      'This desktop build has no first-party analytics endpoint configured, so enabling consent would still keep event delivery off.',
    analyticsEnabled: 'Allow coarse frontend analytics',
    analyticsEndpoint: 'Endpoint',
    analyticsConsentGrantedAt: 'Consent granted',
    analyticsStatusBody:
      'Analytics only runs in packaged desktop production builds, and only after explicit opt-in.',
    analyticsSave: 'Save analytics consent',
    analyticsSaving: 'Saving analytics consent…',
    updateTitle: 'APP UPDATES',
    updateBoundaryTitle: 'Manual check and install',
    updateBoundaryBody:
      'Check the available version, read the notes, and install when you are ready. PathKeep restarts after the update.',
    updateCurrentVersion: 'Current version',
    updateLatestVersion: 'Latest available',
    updatePublishedAt: 'Published',
    updateCheckedAt: 'Last checked',
    updateProgress: 'Downloaded {downloaded} of {total}.',
    updateReleaseNotes: 'Release notes',
    updateCheckNow: 'Check now',
    updateChecking: 'Checking for updates…',
    updateDownloadAndInstall: 'Download and install',
    updateRestartNow: 'Restart now',
    updateOpenReleasePage: 'Open release page',
    updateAvailableBody:
      'PathKeep {version} is available. Review the notes below before installing.',
    updateUpToDateBody:
      'This build is already on the latest available release.',
    updateUnsupportedBody:
      'This surface only works in the desktop app. Browser preview can open the release page instead.',
  },
  'zh-CN': {
    analyticsTitle: '分析',
    analyticsBoundaryTitle: '仅在同意后发送前端分析',
    analyticsBoundaryBody:
      '只有在你明确同意后，PathKeep 才会发送粗粒度前端事件。它绝不会包含存档内容、URL、搜索词、profile ID、run ID、提示词或文件路径。',
    analyticsEndpointMissingTitle: '未配置分析端点',
    analyticsEndpointMissingBody:
      '当前桌面构建没有配置第一方分析端点，所以即使打开同意，事件发送也仍然保持关闭。',
    analyticsEnabled: '允许粗粒度前端分析',
    analyticsEndpoint: '端点',
    analyticsConsentGrantedAt: '同意时间',
    analyticsStatusBody:
      '分析只会在打包后的桌面生产构建中运行，而且必须先明确同意。',
    analyticsSave: '保存分析同意',
    analyticsSaving: '正在保存分析同意…',
    updateTitle: '应用更新',
    updateBoundaryTitle: '手动检查并安装',
    updateBoundaryBody:
      '先查看可用版本和说明，准备好后再安装。PathKeep 会在更新后重启。',
    updateCurrentVersion: '当前版本',
    updateLatestVersion: '最新可用版本',
    updatePublishedAt: '发布时间',
    updateCheckedAt: '上次检查',
    updateProgress: '已下载 {downloaded} / {total}。',
    updateReleaseNotes: '发行说明',
    updateCheckNow: '立即检查',
    updateChecking: '正在检查更新…',
    updateDownloadAndInstall: '下载并安装',
    updateRestartNow: '立即重启',
    updateOpenReleasePage: '打开发布页',
    updateAvailableBody: '已发现 PathKeep {version}。安装前请先查看下方说明。',
    updateUpToDateBody: '当前构建已经是最新可用版本。',
    updateUnsupportedBody:
      '这个界面只在桌面应用里可用。浏览器预览版只能打开发布页面。',
  },
  'zh-TW': {
    analyticsTitle: '分析',
    analyticsBoundaryTitle: '僅在同意後傳送前端分析',
    analyticsBoundaryBody:
      '只有在你明確同意後，PathKeep 才會傳送粗粒度前端事件。它絕不包含封存內容、URL、搜尋詞、profile ID、run ID、提示詞或檔案路徑。',
    analyticsEndpointMissingTitle: '尚未設定分析端點',
    analyticsEndpointMissingBody:
      '目前桌面建置沒有設定第一方分析端點，所以即使開啟同意，事件傳送也仍然維持關閉。',
    analyticsEnabled: '允許粗粒度前端分析',
    analyticsEndpoint: '端點',
    analyticsConsentGrantedAt: '同意時間',
    analyticsStatusBody:
      '分析只會在打包後的桌面正式建置中運作，而且必須先明確同意。',
    analyticsSave: '儲存分析同意',
    analyticsSaving: '正在儲存分析同意…',
    updateTitle: '應用更新',
    updateBoundaryTitle: '手動檢查並安裝',
    updateBoundaryBody:
      '先查看可用版本和說明，準備好後再安裝。PathKeep 會在更新後重新啟動。',
    updateCurrentVersion: '目前版本',
    updateLatestVersion: '最新可用版本',
    updatePublishedAt: '發布時間',
    updateCheckedAt: '上次檢查',
    updateProgress: '已下載 {downloaded} / {total}。',
    updateReleaseNotes: '發行說明',
    updateCheckNow: '立即檢查',
    updateChecking: '正在檢查更新…',
    updateDownloadAndInstall: '下載並安裝',
    updateRestartNow: '立即重新啟動',
    updateOpenReleasePage: '開啟發布頁',
    updateAvailableBody: '已發現 PathKeep {version}。安裝前請先查看下方說明。',
    updateUpToDateBody: '目前建置已經是最新可用版本。',
    updateUnsupportedBody:
      '這個介面只在桌面應用程式中可用。瀏覽器預覽版只能開啟發布頁面。',
  },
} as const
