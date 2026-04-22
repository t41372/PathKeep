/**
 * @file platform.ts
 * @description Owns platform and environment diagnostics copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `platform` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so platform wording stays isolated from host capability detection.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `platform` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const platformNamespaceCatalog = {
  en: {
    macosLabel: 'macOS LaunchAgent',
    windowsLabel: 'Windows Task Scheduler',
    linuxLabel: 'Linux systemd timer',
    macosSummary:
      'On macOS, PathKeep can install a LaunchAgent to run backups automatically.',
    windowsSummary:
      'On Windows, PathKeep uses Task Scheduler. Review the generated file before applying.',
    linuxSummary:
      'On Linux, PathKeep uses a systemd user timer. You may need to install it manually.',
    safariAccessTitle: 'Safari needs Full Disk Access',
    safariAccessBody:
      'Go to System Settings → Privacy & Security → Full Disk Access and add PathKeep to read Safari history.',
    keyringTitle: 'System keychain not available',
    keyringBody:
      "You can still use encryption, but you'll need to enter your password each time. Auto-unlock requires a supported system keychain.",
    schedulerMismatchTitle: 'Schedule needs review',
    schedulerMismatchBody:
      "The installed schedule doesn't match your current settings. Review and re-apply it.",
  },
  'zh-CN': {
    macosLabel: 'macOS LaunchAgent',
    windowsLabel: 'Windows 任务计划',
    linuxLabel: 'Linux systemd 定时器',
    macosSummary: '在 macOS 上，PathKeep 可以自动安装 LaunchAgent 来定时备份。',
    windowsSummary:
      '在 Windows 上，PathKeep 使用任务计划来定时备份。安装前请检查配置文件。',
    linuxSummary:
      '在 Linux 上，PathKeep 使用 systemd 定时器。可能需要手动安装。',
    safariAccessTitle: 'Safari 需要完全磁盘访问权限',
    safariAccessBody:
      '打开系统设置 → 隐私与安全性 → 完全磁盘访问权限，添加 PathKeep 以读取 Safari 历史记录。',
    keyringTitle: '系统钥匙串不可用',
    keyringBody:
      '仍然可以使用加密模式，但每次启动都需要手动输入密码。有可用的钥匙串后才能自动解锁。',
    schedulerMismatchTitle: '定时备份需要检查',
    schedulerMismatchBody: '已安装的配置与当前设置不一致，请重新检查并安装。',
  },
  'zh-TW': {
    macosLabel: 'macOS LaunchAgent',
    windowsLabel: 'Windows 工作排程器',
    linuxLabel: 'Linux systemd 計時器',
    macosSummary: '在 macOS 上，PathKeep 可以自動安裝 LaunchAgent 來定時備份。',
    windowsSummary:
      '在 Windows 上，PathKeep 使用工作排程器來定時備份。安裝前請檢查設定檔。',
    linuxSummary:
      '在 Linux 上，PathKeep 使用 systemd 計時器。可能需要手動安裝。',
    safariAccessTitle: 'Safari 需要完整磁碟取用權限',
    safariAccessBody:
      '前往系統設定 → 隱私權與安全性 → 完整磁碟取用權限，加入 PathKeep 以讀取 Safari 歷史紀錄。',
    keyringTitle: '系統鑰匙圈無法使用',
    keyringBody:
      '仍然可以使用加密模式，但每次啟動都需要手動輸入密碼。有可用的鑰匙圈後才能自動解鎖。',
    schedulerMismatchTitle: '定時備份需要檢查',
    schedulerMismatchBody: '已安裝的設定與目前不一致，請重新檢查並安裝。',
  },
} as const
