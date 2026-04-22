/**
 * @file security.ts
 * @description Owns security, encryption, and unlock flow copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `security` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so security wording stays isolated from credential and unlock logic.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `security` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const securityNamespaceCatalog = {
  en: {
    loadingPosture: 'Loading security status…',
    unavailableTitle: 'Security status unavailable',
    unavailableBody:
      "Couldn't load the current encryption and keychain status.",
    initFirstAction: 'Set up archive first',
    notInitializedTitle: 'No archive yet',
    notInitializedBody:
      'Complete the setup first. Security options will be available after your first backup.',
    encryptionStatus: 'ENCRYPTION',
    archiveIs: 'Archive is {mode}',
    encryptedDetail: 'Encrypted at rest — password required to access',
    plaintextDetail: "Not encrypted — relies on your system's disk encryption",
    keyring: 'Keychain',
    sessionStatus: 'Current session',
    sessionUnlocked: 'Unlocked',
    sessionLocked: 'Locked — unlock to browse history and view audit logs',
    lastBackup: 'Last backup',
    lastRekey: 'Last rekey',
    stronghold: 'Secure storage',
    archivePath: 'Archive location',
    lastRekeySnapshot: 'Last rekey snapshot',
    openLastRekeyAudit: 'Open last rekey review',
    passwordLossTitle: 'No password = no data.',
    passwordLossBody:
      'There is no way to recover a forgotten password. Write it down somewhere safe before making changes.',
    unlockKeyringTitle: 'UNLOCK',
    sessionActive: 'Unlocked',
    needsUnlock: 'Locked',
    currentDatabaseKey: 'PASSWORD',
    currentDatabaseKeyPlaceholder: 'Enter your password',
    currentDatabaseKeyRequired: 'Enter your current password.',
    archiveUnlockFailed:
      'That key did not unlock this archive. Check the password or saved key, then try again.',
    encryptedArchiveNeedsPasswordWarning:
      'Unlock this encrypted archive with its current password before reviewing history or audit data.',
    rememberKeyNeedsKeychainWarning:
      'PathKeep is set to remember this archive password, but no system keychain is available on this machine.',
    rememberedKeyMissingWarning:
      'This archive is encrypted, but its password is not currently saved in the system keychain.',
    unlockArchive: 'Unlock',
    useKeyring: 'Use saved password',
    lockArchive: 'Lock',
    storeInKeyring: 'Save to keychain',
    clearKeyring: 'Remove from keychain',
    keyringConvenience:
      'Saving the password to your system keychain lets you skip typing it each time. Your data still stays local.',
    rekeyTitle: 'CHANGE ENCRYPTION',
    previewBeforeExecute: 'Preview changes first',
    targetMode: 'NEW MODE',
    newDatabaseKey: 'NEW PASSWORD',
    newDatabaseKeyPlaceholder: 'Enter a new password',
    newDatabaseKeyRequired: 'Enter a new password.',
    storeNewKey: 'Save the new password to the system keychain',
    previewRekey: 'Preview changes',
    executeRekey: 'Apply changes',
    rekeyConfirmLabel: 'Type "confirm" to switch to plaintext',
    rekeyConfirmPlaceholder: 'confirm',
    mode: 'MODE',
    snapshot: 'BACKUP',
    temporaryDatabase: 'TEMP',
  },
  'zh-CN': {
    loadingPosture: '加载安全状态…',
    unavailableTitle: '安全状态暂不可用',
    unavailableBody: '无法加载当前的加密和钥匙串状态。',
    initFirstAction: '先完成设置',
    notInitializedTitle: '还没有设置存档',
    notInitializedBody: '完成初始设置后，安全选项才可用。',
    encryptionStatus: '加密状态',
    archiveIs: '存档为{mode}',
    encryptedDetail: '数据已加密，访问前需要输入密码',
    plaintextDetail: '未加密，安全性取决于系统磁盘加密',
    keyring: '钥匙串',
    sessionStatus: '当前状态',
    sessionUnlocked: '已解锁',
    sessionLocked: '已锁定 — 解锁后才能浏览历史和查看日志',
    lastBackup: '上次备份',
    lastRekey: '上次修改加密',
    stronghold: '安全存储',
    archivePath: '存档位置',
    lastRekeySnapshot: '上次修改加密快照',
    openLastRekeyAudit: '打开上次修改加密复核',
    passwordLossTitle: '忘记密码 = 丢失数据',
    passwordLossBody:
      '没有密码恢复功能。修改加密设置前，请确保密码保存在安全的地方。',
    unlockKeyringTitle: '解锁',
    sessionActive: '已解锁',
    needsUnlock: '已锁定',
    currentDatabaseKey: '密码',
    currentDatabaseKeyPlaceholder: '输入密码',
    currentDatabaseKeyRequired: '请输入当前密码。',
    archiveUnlockFailed:
      '这把钥匙还不能解锁这个存档。请检查密码或已保存的钥匙后再试一次。',
    encryptedArchiveNeedsPasswordWarning:
      '请先用当前密码解锁这个加密存档，再查看历史记录或审计数据。',
    rememberKeyNeedsKeychainWarning:
      'PathKeep 已设置为记住这个存档密码，但这台机器目前没有可用的系统钥匙串。',
    rememberedKeyMissingWarning:
      '这个存档已加密，但它的密码目前还没有保存在系统钥匙串里。',
    unlockArchive: '解锁',
    useKeyring: '使用已保存的密码',
    lockArchive: '锁定',
    storeInKeyring: '保存到钥匙串',
    clearKeyring: '从钥匙串删除',
    keyringConvenience:
      '保存到系统钥匙串可以免去每次输入密码，数据仍然只存在你的设备上。',
    rekeyTitle: '修改加密',
    previewBeforeExecute: '先预览变更',
    targetMode: '新模式',
    newDatabaseKey: '新密码',
    newDatabaseKeyPlaceholder: '输入新密码',
    newDatabaseKeyRequired: '请输入新密码。',
    storeNewKey: '修改后保存新密码到钥匙串',
    previewRekey: '预览变更',
    executeRekey: '确认修改',
    rekeyConfirmLabel: '输入 "confirm" 以切换为明文模式',
    rekeyConfirmPlaceholder: 'confirm',
    mode: '模式',
    snapshot: '备份',
    temporaryDatabase: '临时',
  },
  'zh-TW': {
    loadingPosture: '載入安全狀態…',
    unavailableTitle: '安全狀態暫時無法使用',
    unavailableBody: '無法載入目前的加密和鑰匙圈狀態。',
    initFirstAction: '先完成設定',
    notInitializedTitle: '還沒有設定封存',
    notInitializedBody: '完成初始設定後，安全選項才可使用。',
    encryptionStatus: '加密狀態',
    archiveIs: '封存為{mode}',
    encryptedDetail: '資料已加密，存取前需要輸入密碼',
    plaintextDetail: '未加密，安全性取決於系統磁碟加密',
    keyring: '鑰匙圈',
    sessionStatus: '目前狀態',
    sessionUnlocked: '已解鎖',
    sessionLocked: '已鎖定 — 解鎖後才能瀏覽歷史和查看日誌',
    lastBackup: '上次備份',
    lastRekey: '上次修改加密',
    stronghold: '安全儲存',
    archivePath: '封存位置',
    lastRekeySnapshot: '上次修改加密快照',
    openLastRekeyAudit: '打開上次修改加密複核',
    passwordLossTitle: '忘記密碼 = 遺失資料',
    passwordLossBody:
      '沒有密碼復原功能。修改加密設定前，請確保密碼記在安全的地方。',
    unlockKeyringTitle: '解鎖',
    sessionActive: '已解鎖',
    needsUnlock: '已鎖定',
    currentDatabaseKey: '密碼',
    currentDatabaseKeyPlaceholder: '輸入密碼',
    currentDatabaseKeyRequired: '請輸入目前密碼。',
    archiveUnlockFailed:
      '這把鑰匙還不能解鎖這個封存。請檢查密碼或已儲存的鑰匙後再試一次。',
    encryptedArchiveNeedsPasswordWarning:
      '請先用目前密碼解鎖這個加密封存，再查看歷史記錄或稽核資料。',
    rememberKeyNeedsKeychainWarning:
      'PathKeep 已設定成記住這個封存密碼，但這台機器目前沒有可用的系統鑰匙圈。',
    rememberedKeyMissingWarning:
      '這個封存已加密，但它的密碼目前還沒有保存在系統鑰匙圈裡。',
    unlockArchive: '解鎖',
    useKeyring: '使用已儲存的密碼',
    lockArchive: '鎖定',
    storeInKeyring: '儲存到鑰匙圈',
    clearKeyring: '從鑰匙圈移除',
    keyringConvenience:
      '儲存到系統鑰匙圈可以免去每次輸入密碼，資料仍然只在你的裝置上。',
    rekeyTitle: '修改加密',
    previewBeforeExecute: '先預覽變更',
    targetMode: '新模式',
    newDatabaseKey: '新密碼',
    newDatabaseKeyPlaceholder: '輸入新密碼',
    newDatabaseKeyRequired: '請輸入新密碼。',
    storeNewKey: '修改後儲存新密碼到鑰匙圈',
    previewRekey: '預覽變更',
    executeRekey: '確認修改',
    rekeyConfirmLabel: '輸入 "confirm" 以切換為明文模式',
    rekeyConfirmPlaceholder: 'confirm',
    mode: '模式',
    snapshot: '備份',
    temporaryDatabase: '暫存',
  },
} as const
