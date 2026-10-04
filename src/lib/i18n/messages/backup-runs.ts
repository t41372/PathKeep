import { defineMessages } from '../define'

/** Backup → Recent runs → run detail sheet: tabs, files kept, safety copy. */
export const backupRuns = defineMessages({
  en: {
    tabs: {
      summary: 'Summary',
      files: 'Files',
      warnings_one: 'Warnings ({count})',
      warnings_other: 'Warnings ({count})',
    },
    manifest: 'Manifest',
    manifestHash: 'Fingerprint',
    noManifest: 'This run has no manifest.',
    noFiles: 'This run kept no other files.',
    kinds: { snapshot: 'Saved copy' },
    reasons: {
      'before-rekey': 'Copy from before the encryption change',
      'source-schema-changed': 'Copy kept because the browser’s format changed',
      'periodic-checkpoint': 'Periodic copy of the browser’s history file',
    },
    size: 'Size',
    safety: {
      title: 'Safety copy',
      body: 'PathKeep kept the archive as it was before the encryption change. It opens with the password the archive had then, or none if it was not encrypted.',
    },
    timezone: 'Time zone',
    dueOnly: 'Ran because a backup was due',
  },
  'zh-CN': {
    tabs: {
      summary: '摘要',
      files: '文件',
      warnings_one: '警告（{count}）',
      warnings_other: '警告（{count}）',
    },
    manifest: '清单',
    manifestHash: '指纹',
    noManifest: '这次运行没有清单。',
    noFiles: '这次运行没有保留其他文件。',
    kinds: { snapshot: '保存的副本' },
    reasons: {
      'before-rekey': '加密变更前的副本',
      'source-schema-changed': '浏览器格式变了，因此保留的副本',
      'periodic-checkpoint': '浏览器历史文件的定期副本',
    },
    size: '大小',
    safety: {
      title: '安全副本',
      body: 'PathKeep 保留了加密变更前的存档。打开它需要当时的密码；如果当时没有加密，则不需要密码。',
    },
    timezone: '时区',
    dueOnly: '因为到了备份时间而运行',
  },
  'zh-TW': {
    tabs: {
      summary: '摘要',
      files: '檔案',
      warnings_one: '警告（{count}）',
      warnings_other: '警告（{count}）',
    },
    manifest: '清單',
    manifestHash: '指紋',
    noManifest: '這次執行沒有清單。',
    noFiles: '這次執行沒有保留其他檔案。',
    kinds: { snapshot: '儲存的副本' },
    reasons: {
      'before-rekey': '加密變更前的副本',
      'source-schema-changed': '瀏覽器格式變了，因此保留的副本',
      'periodic-checkpoint': '瀏覽器歷史檔案的定期副本',
    },
    size: '大小',
    safety: {
      title: '安全副本',
      body: 'PathKeep 保留了加密變更前的存檔。打開它需要當時的密碼；如果當時沒有加密，則不需要密碼。',
    },
    timezone: '時區',
    dueOnly: '因為到了備份時間而執行',
  },
})
