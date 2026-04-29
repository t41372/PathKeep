/**
 * @file schedule.ts
 * @description Owns schedule and backup automation copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `schedule` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so schedule copy edits do not touch automation logic.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `schedule` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const scheduleNamespaceCatalog = {
  en: {
    loadingPreview: 'Loading schedule preview…',
    unavailableTitle: 'Schedule preview unavailable',
    unavailableBody: "Couldn't load the schedule preview right now.",
    backupSchedule: 'BACKUP SCHEDULE',
    platformBoundary: 'PLATFORM',
    statusBoundary: 'INSTALL STATE',
    configurationTitle: 'SCHEDULED BACKUP SETTINGS',
    installState: 'Status',
    interval: 'Backup trigger',
    verification: 'Schedule health check',
    intervalValue: 'Backup becomes due every {hours} hours',
    verificationValue:
      'PathKeep checks the installed schedule every {hours} hours',
    desiredInterval: 'Desired backup trigger',
    selectedIntervalValue: 'Back up when history is due every {hours} hours',
    intervalChipLabel: '{hours}h',
    intervalChangedTitle: 'Interval changed',
    intervalChangedBody:
      'Save this interval, then install or update the native schedule so the background task uses it.',
    mechanism: 'Method',
    lastTriggered: 'Last run',
    label: 'Name',
    profiles: 'Profiles',
    pmeTitle: 'MANAGE SCHEDULE',
    previewBoundary: 'PREVIEW',
    previewBody:
      'Review the schedule file and install steps before applying. You can undo this later.',
    noGeneratedFiles: 'Schedule files are only available in the desktop app.',
    openLatestAudit: 'View audit log',
    executeRun: 'APPLY',
    fileArtifact: 'FILE',
    executeBody:
      'Apply or remove the schedule after reviewing the preview above.',
    applyCommand: 'INSTALL STEP {index}',
    rollbackCommand: 'UNINSTALL STEP {index}',
    saveInterval: 'Save interval',
    saveAndInstallSchedule: 'Save & install schedule',
    saveAndUpdateSchedule: 'Save & update schedule',
    updateInstalledSchedule: 'Update installed schedule',
    installCanonicalSchedule: 'Install current schedule',
    installFromCurrentSettings: 'Install from current settings',
    applySchedule: 'Install schedule',
    removeSchedule: 'Remove schedule',
    removeInstalledSchedule: 'Remove installed schedule',
    openSchedulerAudit: 'View audit log',
    initializeArchiveFirst:
      'Set up the archive first, then come back to install the schedule.',
    installedDescription: 'Schedule is installed and up to date.',
    mismatchDescription:
      "Schedule is installed but doesn't match the current settings. Review and re-apply.",
    permissionWarningDescription:
      "Couldn't check the installed schedule files. Check file permissions.",
    legacyInstallDescription:
      'An older schedule is still installed. Review it before applying the new one.',
    manualReviewDescription:
      "On this platform, you'll need to install the schedule manually. Follow the steps below.",
    notInstalledDescription: 'No schedule installed yet.',
    installedBadge: 'Installed',
    attentionBadge: 'Needs attention',
    manualReviewBadge: 'Manual setup',
    notInstalledBadge: 'Not installed',
  },
  'zh-CN': {
    loadingPreview: '加载定时备份预览…',
    unavailableTitle: '定时备份预览不可用',
    unavailableBody: '无法加载定时备份预览。',
    backupSchedule: '定时备份',
    platformBoundary: '平台',
    statusBoundary: '安装状态',
    configurationTitle: '定时备份设置',
    installState: '状态',
    interval: '备份触发',
    verification: '定时备份健康检查',
    intervalValue: '每 {hours} 小时到期一次备份',
    verificationValue: '每 {hours} 小时检查已安装的定时备份',
    desiredInterval: '目标备份触发',
    selectedIntervalValue: '每 {hours} 小时检查是否需要备份',
    intervalChipLabel: '{hours} 小时',
    intervalChangedTitle: '间隔已修改',
    intervalChangedBody:
      '请先保存这个间隔，再安装或更新系统定时任务，让后台任务使用新的设置。',
    mechanism: '方式',
    lastTriggered: '上次运行',
    label: '名称',
    profiles: '浏览器',
    pmeTitle: '管理定时备份',
    previewBoundary: '预览',
    previewBody: '安装前先查看配置文件和安装步骤，安装后可以卸载。',
    noGeneratedFiles: '配置文件只在桌面应用中生成。',
    openLatestAudit: '查看日志',
    executeRun: '安装',
    fileArtifact: '文件',
    executeBody: '确认预览内容后再安装或卸载定时备份。',
    applyCommand: '安装步骤 {index}',
    rollbackCommand: '卸载步骤 {index}',
    saveInterval: '保存间隔',
    saveAndInstallSchedule: '保存并安装定时备份',
    saveAndUpdateSchedule: '保存并更新定时备份',
    updateInstalledSchedule: '更新已安装任务',
    installCanonicalSchedule: '安装当前定时备份',
    installFromCurrentSettings: '按当前设置安装',
    applySchedule: '安装定时备份',
    removeSchedule: '卸载定时备份',
    removeInstalledSchedule: '卸载已安装任务',
    openSchedulerAudit: '查看日志',
    initializeArchiveFirst: '请先完成初始设置，然后回来配置定时备份。',
    installedDescription: '定时备份已安装，配置与当前设置一致。',
    mismatchDescription: '已安装但配置与当前设置不一致，建议重新安装。',
    permissionWarningDescription: '无法检查已安装的配置文件，请检查文件权限。',
    legacyInstallDescription: '发现旧版配置，建议检查后再重新安装。',
    manualReviewDescription: '在此平台上需要手动安装，请按照下方步骤操作。',
    notInstalledDescription: '还没有安装定时备份。',
    installedBadge: '已安装',
    attentionBadge: '需要处理',
    manualReviewBadge: '手动安装',
    notInstalledBadge: '未安装',
  },
  'zh-TW': {
    loadingPreview: '載入定時備份預覽…',
    unavailableTitle: '定時備份預覽無法使用',
    unavailableBody: '無法載入定時備份預覽。',
    backupSchedule: '定時備份',
    platformBoundary: '平台',
    statusBoundary: '安裝狀態',
    configurationTitle: '定時備份設定',
    installState: '狀態',
    interval: '備份觸發',
    verification: '定時備份健康檢查',
    intervalValue: '每 {hours} 小時到期一次備份',
    verificationValue: '每 {hours} 小時檢查已安裝的定時備份',
    desiredInterval: '目標備份觸發',
    selectedIntervalValue: '每 {hours} 小時檢查是否需要備份',
    intervalChipLabel: '{hours} 小時',
    intervalChangedTitle: '間隔已修改',
    intervalChangedBody:
      '請先儲存這個間隔，再安裝或更新系統排程，讓背景任務使用新的設定。',
    mechanism: '方式',
    lastTriggered: '上次執行',
    label: '名稱',
    profiles: '瀏覽器',
    pmeTitle: '管理定時備份',
    previewBoundary: '預覽',
    previewBody: '安裝前先查看設定檔和安裝步驟，安裝後可以移除。',
    noGeneratedFiles: '設定檔只在桌面應用程式中產生。',
    openLatestAudit: '查看日誌',
    executeRun: '安裝',
    fileArtifact: '檔案',
    executeBody: '確認預覽內容後再安裝或移除定時備份。',
    applyCommand: '安裝步驟 {index}',
    rollbackCommand: '移除步驟 {index}',
    saveInterval: '儲存間隔',
    saveAndInstallSchedule: '儲存並安裝定時備份',
    saveAndUpdateSchedule: '儲存並更新定時備份',
    updateInstalledSchedule: '更新已安裝排程',
    installCanonicalSchedule: '安裝目前定時備份',
    installFromCurrentSettings: '依目前設定安裝',
    applySchedule: '安裝定時備份',
    removeSchedule: '移除定時備份',
    removeInstalledSchedule: '移除已安裝排程',
    openSchedulerAudit: '查看日誌',
    initializeArchiveFirst: '請先完成初始設定，再回來設定定時備份。',
    installedDescription: '定時備份已安裝，設定與目前一致。',
    mismatchDescription: '已安裝但設定與目前不一致，建議重新安裝。',
    permissionWarningDescription: '無法檢查已安裝的設定檔，請確認檔案權限。',
    legacyInstallDescription: '發現舊版設定，建議檢查後再重新安裝。',
    manualReviewDescription: '此平台需要手動安裝，請依照下方步驟操作。',
    notInstalledDescription: '還沒有安裝定時備份。',
    installedBadge: '已安裝',
    attentionBadge: '需要處理',
    manualReviewBadge: '手動安裝',
    notInstalledBadge: '未安裝',
  },
} as const
