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
      'That password did not unlock this archive. Retype it, or use the one saved in your keychain.',
    encryptedArchiveNeedsPasswordWarning:
      'Unlock this encrypted archive with its current password before reviewing history or audit data.',
    rememberKeyNeedsKeychainWarning:
      'PathKeep is set to remember this archive password, but no system keychain is available on this machine.',
    rememberedKeyMissingWarning:
      'This archive is encrypted, but its password is not currently saved in the system keychain.',
    unlockArchive: 'Unlock',
    useKeyring: 'Use saved password',
    lockArchive: 'Lock',
    unlockGateTitle: 'Archive locked',
    unlockGateBody:
      'This archive is encrypted. Enter your password to unlock this session.',
    unlockGateBackupRetryBody:
      'The last backup could not run — the archive was locked. Unlock it to retry.',
    cantUnlockRecover: "Can't unlock? Recover from a snapshot",
    cantUnlockRecoverAria:
      "Can't unlock — recover from a saved snapshot instead",
    recoverTitle: 'Restore from a snapshot',
    recoverBody:
      'Locked out? Your history is safe in your saved snapshots — restore one to get back in.',
    backToUnlock: 'Back to unlock',
    backToUnlockAria: 'Go back to the unlock form',
    rememberOnThisDevice: 'Remember on this device',
    rememberOnThisDeviceNamed: 'Remember on this device ({backend})',
    keychainSectionTitle: 'ARCHIVE PASSWORD',
    keychainToggleLabel: 'Remember password on this device',
    keychainToggleHelp:
      'Saves your archive password to the system keychain. Future sessions unlock automatically without a prompt.',
    keychainStatusStored: 'Stored in {backend}',
    keychainStatusNotStored: 'Not stored',
    keychainClearFailedTitle: 'The saved password was not removed',
    keychainClearFailed:
      'PathKeep could not remove the archive password from this machine\u2019s keychain, so the setting was left unchanged. Try again, or remove the PathKeep entry from the system keychain yourself.',
    keychainStatusUnavailable: 'System keychain unavailable',
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
    rekeyArchiveLockedWarning:
      'The archive is currently locked. Unlock it before applying this change.',
    rekeyNewKeyRequiredWarning:
      'Switching to encrypted needs a new password before the change can be applied.',
    rekeySameModeRewriteWarning:
      'The new mode matches the current mode, so the archive will still be rewritten as a password rotation or validation pass.',
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
      '这个密码打不开这个存档。请重新输入，或改用系统钥匙串里保存的那个密码。',
    encryptedArchiveNeedsPasswordWarning:
      '请先用当前密码解锁这个加密存档，再查看历史记录或审计数据。',
    rememberKeyNeedsKeychainWarning:
      'PathKeep 已设置为记住这个存档密码，但这台机器目前没有可用的系统钥匙串。',
    rememberedKeyMissingWarning:
      '这个存档已加密，但它的密码目前还没有保存在系统钥匙串里。',
    unlockArchive: '解锁',
    useKeyring: '使用已保存的密码',
    lockArchive: '锁定',
    unlockGateTitle: '存档已锁定',
    unlockGateBody: '这个存档已加密，请输入密码解锁当前会话。',
    unlockGateBackupRetryBody: '上次备份因存档锁定而中止，解锁后将自动重试。',
    cantUnlockRecover: '无法解锁？从快照恢复',
    cantUnlockRecoverAria: '无法解锁——改为从已保存的快照恢复',
    recoverTitle: '从快照恢复',
    recoverBody:
      '被锁在外面了？你的历史记录安全地保存在快照里——恢复其中一个即可重新进入。',
    backToUnlock: '返回解锁',
    backToUnlockAria: '返回解锁表单',
    rememberOnThisDevice: '在此设备上记住',
    rememberOnThisDeviceNamed: '在此设备上记住（{backend}）',
    keychainSectionTitle: '存档密码',
    keychainToggleLabel: '在此设备上记住密码',
    keychainToggleHelp: '将密码保存到系统钥匙串，以后启动时无需再次输入。',
    keychainStatusStored: '已保存到 {backend}',
    keychainStatusNotStored: '未保存',
    keychainClearFailedTitle: '没有移除已保存的密码',
    keychainClearFailed:
      'PathKeep 无法把存档密码从这台设备的钥匙串中移除，因此这项设置保持原样。请重试，或自己在系统钥匙串里删除 PathKeep 的条目。',
    keychainStatusUnavailable: '系统钥匙串不可用',
    rekeyTitle: '修改加密',
    previewBeforeExecute: '先预览变更',
    targetMode: '新模式',
    newDatabaseKey: '新密码',
    newDatabaseKeyPlaceholder: '输入新密码',
    newDatabaseKeyRequired: '请输入新密码。',
    storeNewKey: '修改后保存新密码到钥匙串',
    previewRekey: '预览变更',
    executeRekey: '确认修改',
    rekeyConfirmLabel: '输入「confirm」以切换为明文模式',
    rekeyConfirmPlaceholder: 'confirm',
    rekeyArchiveLockedWarning:
      '存档目前处于锁定状态。请先解锁，再应用这项修改。',
    rekeyNewKeyRequiredWarning:
      '切换到加密模式需要先设置新密码，然后才能应用修改。',
    rekeySameModeRewriteWarning:
      '新模式与当前模式相同，存档仍会整体重写，相当于一次密码轮换或校验。',
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
      '這個密碼打不開這個封存。請重新輸入，或改用系統鑰匙圈裡儲存的那組密碼。',
    encryptedArchiveNeedsPasswordWarning:
      '請先用目前密碼解鎖這個加密封存，再查看歷史記錄或稽核資料。',
    rememberKeyNeedsKeychainWarning:
      'PathKeep 已設定成記住這個封存密碼，但這台機器目前沒有可用的系統鑰匙圈。',
    rememberedKeyMissingWarning:
      '這個封存已加密，但它的密碼目前還沒有保存在系統鑰匙圈裡。',
    unlockArchive: '解鎖',
    useKeyring: '使用已儲存的密碼',
    lockArchive: '鎖定',
    unlockGateTitle: '封存已鎖定',
    unlockGateBody: '此封存已加密，請輸入密碼解鎖目前工作階段。',
    unlockGateBackupRetryBody: '上次備份因封存鎖定而中止，解鎖後將自動重試。',
    cantUnlockRecover: '無法解鎖？從快照還原',
    cantUnlockRecoverAria: '無法解鎖——改為從已儲存的快照還原',
    recoverTitle: '從快照還原',
    recoverBody:
      '被鎖在外面了？你的歷史記錄安全地保存在快照裡——還原其中一個即可重新進入。',
    backToUnlock: '返回解鎖',
    backToUnlockAria: '返回解鎖表單',
    rememberOnThisDevice: '在此裝置上記住',
    rememberOnThisDeviceNamed: '在此裝置上記住（{backend}）',
    keychainSectionTitle: '封存密碼',
    keychainToggleLabel: '在此裝置上記住密碼',
    keychainToggleHelp: '將密碼儲存到系統鑰匙圈，之後啟動時不需再次輸入。',
    keychainStatusStored: '已儲存至 {backend}',
    keychainStatusNotStored: '尚未儲存',
    keychainClearFailedTitle: '沒有移除已儲存的密碼',
    keychainClearFailed:
      'PathKeep 無法把封存密碼從這台裝置的鑰匙圈中移除，因此這項設定保持原樣。請重試，或自己在系統鑰匙圈裡刪除 PathKeep 的項目。',
    keychainStatusUnavailable: '系統鑰匙圈不可用',
    rekeyTitle: '修改加密',
    previewBeforeExecute: '先預覽變更',
    targetMode: '新模式',
    newDatabaseKey: '新密碼',
    newDatabaseKeyPlaceholder: '輸入新密碼',
    newDatabaseKeyRequired: '請輸入新密碼。',
    storeNewKey: '修改後儲存新密碼到鑰匙圈',
    previewRekey: '預覽變更',
    executeRekey: '確認修改',
    rekeyConfirmLabel: '輸入「confirm」以切換為明文模式',
    rekeyConfirmPlaceholder: 'confirm',
    rekeyArchiveLockedWarning:
      '封存目前處於鎖定狀態。請先解鎖，再套用這項修改。',
    rekeyNewKeyRequiredWarning:
      '切換到加密模式需要先設定新密碼，然後才能套用修改。',
    rekeySameModeRewriteWarning:
      '新模式與目前模式相同，封存仍會整體重寫，相當於一次密碼輪換或驗證。',
    mode: '模式',
    snapshot: '備份',
    temporaryDatabase: '暫存',
  },
} as const
