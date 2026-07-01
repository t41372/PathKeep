/**
 * @file recovery.ts
 * @description Owns all copy for the archive recovery screen and Settings → Restore from snapshot section.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `recovery` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Cover every visible state: loading, empty, error, confirm, restoring, success, and all aria-labels.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so recovery wording stays isolated from archive logic.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `recovery` namespace payload for the shipped locales.
 *
 * All keys must appear in all three locales with no raw English in zh-CN or zh-TW.
 */
export const recoveryNamespaceCatalog = {
  en: {
    // Screen header
    eyebrow: 'PathKeep',
    title: 'Restore from snapshot',
    bodyOne:
      'Your archive needs attention. PathKeep found 1 verified snapshot — restoring it will replace the current broken state. Your broken archive is automatically quarantined (moved, not deleted) so nothing is lost.',
    bodyMany:
      'Your archive needs attention. PathKeep found {count} verified snapshots — restoring one will replace the current broken state. Your broken archive is automatically quarantined (moved, not deleted) so nothing is lost.',
    bodyNoSnapshots:
      'Your archive needs attention and no verified snapshots are available.',
    bodyUnverifiedOnly:
      'Your archive needs attention. No verified snapshot is available, but older snapshots can still be tried.',

    // Source operation labels — must match backend KNOWN_OPS: rekey | reconcile | import | periodic | unknown
    sourceOp: {
      rekey: 'Encryption change',
      reconcile: 'Encryption maintenance',
      import: 'Archive import',
      periodic: 'Periodic snapshot',
      unknown: 'Automatic snapshot',
    },

    // Snapshot card
    snapshotDate: 'Created {date}',
    snapshotDateUnknown: 'Date unknown',
    snapshotSize: '{size}',
    verifiedBadge: 'Verified',
    notVerifiedBadge: 'Not verified',
    encryptedNeedsKeyBadge: 'Encrypted · needs your key',

    // Archive-key entry (encrypted snapshot confirm step)
    keyFieldLabel: 'Archive key',
    keyFieldPlaceholder: 'Enter your archive key',
    keyFieldHint:
      'This snapshot is encrypted. Enter your archive key so PathKeep can verify and restore it. A wrong key fails safely — nothing is changed.',

    // Actions
    restoreThis: 'Restore this',
    restoreThisAria: 'Restore from this snapshot',
    seeAll: 'See all snapshots',
    seeAllAria: 'Show all available snapshots',
    hideAll: 'Hide snapshot list',
    hideAllAria: 'Collapse snapshot list',

    // Confirm step
    confirmTitle: 'Replace archive and restore?',
    confirmBody:
      'This will replace your live archive with the snapshot from {date}. Your current broken state will be quarantined (moved to a safe location — not deleted) so you can retrieve it if needed.',
    confirmBodyDateUnknown:
      'This will replace your live archive with the selected snapshot. Your current broken state will be quarantined (moved to a safe location — not deleted) so you can retrieve it if needed.',
    restoreNow: 'Restore now',
    restoreNowAria: 'Confirm and restore from this snapshot',
    cancelRestore: 'Cancel',
    cancelRestoreAria: 'Cancel restore and go back',

    // Restoring state
    restoring: 'Restoring…',

    // Success
    restoreSuccess: 'Restore complete. Your archive is ready.',

    // Failure
    restoreError: 'Restore failed',
    restoreErrorDetail: 'Detail: {detail}',
    retry: 'Try another snapshot',
    retryAria: 'Dismiss error and pick another snapshot to restore',

    // Empty state
    emptyTitle: 'No restore points yet',
    emptyBody:
      'PathKeep captures a verified snapshot before each whole-archive rewrite (encryption change / re-key / whole-archive import).',
    emptyReassurance:
      'Your raw data files have been moved to quarantine — not deleted — and can be recovered manually or with support.',
    revealLogs: 'Reveal logs',
    revealLogsAria: 'Open the PathKeep logs directory in Finder',

    // Error state (list load)
    loadError: 'Failed to load snapshots',
    loadErrorAria: 'Error loading snapshot list',
    loadRetry: 'Retry',
    loadRetryAria: 'Retry loading snapshots',

    // Loading state (list load)
    loadingSnapshots: 'Loading snapshots…',
    loadingSnapshotsAria: 'Loading available snapshots',

    // Settings section
    sectionTitle: 'RESTORE FROM SNAPSHOT',
    sectionDescription:
      'Roll back to a verified full-archive snapshot taken before a whole-archive rewrite. The current state is quarantined (moved, not deleted) before the restore runs.',
  },
  'zh-CN': {
    eyebrow: 'PathKeep',
    title: '从快照恢复',
    bodyOne:
      '你的存档需要处理。PathKeep 找到了 1 个已验证的快照——恢复它将替换当前损坏的存档。损坏的存档会自动隔离（移走，不删除），数据不会丢失。',
    bodyMany:
      '你的存档需要处理。PathKeep 找到了 {count} 个已验证的快照——恢复其中一个将替换当前损坏的存档。损坏的存档会自动隔离（移走，不删除），数据不会丢失。',
    bodyNoSnapshots: '你的存档需要处理，但没有可用的已验证快照。',
    bodyUnverifiedOnly:
      '你的存档需要处理。目前没有已验证的快照，但可以尝试使用较早的快照恢复。',

    sourceOp: {
      rekey: '修改加密',
      reconcile: '加密维护',
      import: '存档导入',
      periodic: '定期快照',
      unknown: '自动快照',
    },

    snapshotDate: '创建于 {date}',
    snapshotDateUnknown: '日期未知',
    snapshotSize: '{size}',
    verifiedBadge: '已验证',
    notVerifiedBadge: '未验证',
    encryptedNeedsKeyBadge: '已加密 · 需要你的密钥',

    keyFieldLabel: '存档密钥',
    keyFieldPlaceholder: '输入你的存档密钥',
    keyFieldHint:
      '此快照已加密。请输入你的存档密钥，PathKeep 才能验证并恢复它。密钥错误会安全失败，不会改动任何数据。',

    restoreThis: '恢复此快照',
    restoreThisAria: '从此快照恢复',
    seeAll: '查看所有快照',
    seeAllAria: '显示所有可用快照',
    hideAll: '收起快照列表',
    hideAllAria: '折叠快照列表',

    confirmTitle: '替换存档并恢复？',
    confirmBody:
      '此操作将用 {date} 的快照替换当前的存档。当前损坏的存档将被隔离（移到安全位置，不删除），如需可随时取回。',
    confirmBodyDateUnknown:
      '此操作将用所选快照替换当前的存档。当前损坏的存档将被隔离（移到安全位置，不删除），如需可随时取回。',
    restoreNow: '立即恢复',
    restoreNowAria: '确认并从此快照恢复',
    cancelRestore: '取消',
    cancelRestoreAria: '取消恢复，返回',

    restoring: '恢复中…',

    restoreSuccess: '恢复完成，存档已就绪。',

    restoreError: '恢复失败',
    restoreErrorDetail: '详情：{detail}',
    retry: '换一个快照',
    retryAria: '关闭错误提示，选择其他快照',

    emptyTitle: '暂无还原点',
    emptyBody:
      'PathKeep 在每次全量存档改写前（修改加密 / 重新加密 / 全量导入）会自动保存一个已验证快照。',
    emptyReassurance:
      '你的原始数据文件已移至隔离区（未删除），可手动恢复或联系支持获取帮助。',
    revealLogs: '查看日志',
    revealLogsAria: '在访达中打开 PathKeep 日志目录',

    loadError: '加载快照失败',
    loadErrorAria: '加载快照列表出错',
    loadRetry: '重试',
    loadRetryAria: '重新加载快照',

    loadingSnapshots: '加载快照中…',
    loadingSnapshotsAria: '正在加载可用快照',

    sectionTitle: '从快照恢复',
    sectionDescription:
      '回滚到全量存档改写前保存的已验证快照。恢复前，当前状态会被隔离（移走，不删除）。',
  },
  'zh-TW': {
    eyebrow: 'PathKeep',
    title: '從快照還原',
    bodyOne:
      '你的封存需要處理。PathKeep 找到了 1 個已驗證的快照——還原它將取代目前損壞的封存。損壞的封存會自動隔離（移走，不刪除），資料不會遺失。',
    bodyMany:
      '你的封存需要處理。PathKeep 找到了 {count} 個已驗證的快照——還原其中一個將取代目前損壞的封存。損壞的封存會自動隔離（移走，不刪除），資料不會遺失。',
    bodyNoSnapshots: '你的封存需要處理，但沒有可用的已驗證快照。',
    bodyUnverifiedOnly:
      '你的封存需要處理。目前沒有已驗證的快照，但可以嘗試使用較早的快照還原。',

    sourceOp: {
      rekey: '修改加密',
      reconcile: '加密維護',
      import: '封存匯入',
      periodic: '定期快照',
      unknown: '自動快照',
    },

    snapshotDate: '建立於 {date}',
    snapshotDateUnknown: '日期不明',
    snapshotSize: '{size}',
    verifiedBadge: '已驗證',
    notVerifiedBadge: '未驗證',
    encryptedNeedsKeyBadge: '已加密 · 需要你的密鑰',

    keyFieldLabel: '封存密鑰',
    keyFieldPlaceholder: '輸入你的封存密鑰',
    keyFieldHint:
      '此快照已加密。請輸入你的封存密鑰，PathKeep 才能驗證並還原它。密鑰錯誤會安全失敗，不會更動任何資料。',

    restoreThis: '還原此快照',
    restoreThisAria: '從此快照還原',
    seeAll: '查看所有快照',
    seeAllAria: '顯示所有可用快照',
    hideAll: '收起快照列表',
    hideAllAria: '折疊快照列表',

    confirmTitle: '取代封存並還原？',
    confirmBody:
      '此操作將以 {date} 的快照取代目前的封存。目前損壞的封存將被隔離（移至安全位置，不刪除），必要時可取回。',
    confirmBodyDateUnknown:
      '此操作將以所選快照取代目前的封存。目前損壞的封存將被隔離（移至安全位置，不刪除），必要時可取回。',
    restoreNow: '立即還原',
    restoreNowAria: '確認並從此快照還原',
    cancelRestore: '取消',
    cancelRestoreAria: '取消還原，返回',

    restoring: '還原中…',

    restoreSuccess: '還原完成，封存已就緒。',

    restoreError: '還原失敗',
    restoreErrorDetail: '詳情：{detail}',
    retry: '換一個快照',
    retryAria: '關閉錯誤提示，選擇其他快照',

    emptyTitle: '尚無還原點',
    emptyBody:
      'PathKeep 在每次全量封存改寫前（修改加密 / 重新加密 / 全量匯入）會自動儲存一個已驗證快照。',
    emptyReassurance:
      '你的原始資料檔案已移至隔離區（未刪除），可手動還原或聯絡支援取得協助。',
    revealLogs: '查看日誌',
    revealLogsAria: '在 Finder 中開啟 PathKeep 日誌目錄',

    loadError: '載入快照失敗',
    loadErrorAria: '載入快照列表出錯',
    loadRetry: '重試',
    loadRetryAria: '重新載入快照',

    loadingSnapshots: '載入快照中…',
    loadingSnapshotsAria: '正在載入可用快照',

    sectionTitle: '從快照還原',
    sectionDescription:
      '回滾至全量封存改寫前儲存的已驗證快照。還原前，目前狀態會被隔離（移走，不刪除）。',
  },
} as const
