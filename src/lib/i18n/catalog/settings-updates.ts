/**
 * @file settings-updates.ts
 * @description Defines the settings translation owner for manual app update copy.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide update-related settings copy for en, zh-CN, and zh-TW.
 * - Keep update copy aligned across locales without reintroducing a broad settings owner.
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
 * Keeps the update namespace subsection aligned across shipping locales.
 */
export const settingsUpdatesNamespace = {
  en: {
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
    updateChecking: 'Checking for updates...',
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
    updateChecking: '正在检查更新...',
    updateDownloadAndInstall: '下载并安装',
    updateRestartNow: '立即重启',
    updateOpenReleasePage: '打开发布页',
    updateAvailableBody: '已发现 PathKeep {version}。安装前请先查看下方说明。',
    updateUpToDateBody: '当前构建已经是最新可用版本。',
    updateUnsupportedBody:
      '这个界面只在桌面应用里可用。浏览器预览版只能打开发布页面。',
  },
  'zh-TW': {
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
    updateChecking: '正在檢查更新...',
    updateDownloadAndInstall: '下載並安裝',
    updateRestartNow: '立即重新啟動',
    updateOpenReleasePage: '開啟發布頁面',
    updateAvailableBody: '已發現 PathKeep {version}。安裝前請先查看下方說明。',
    updateUpToDateBody: '目前建置已經是最新可用版本。',
    updateUnsupportedBody:
      '這個介面只在桌面應用程式中可用。瀏覽器預覽版只能開啟發布頁面。',
  },
} as const
