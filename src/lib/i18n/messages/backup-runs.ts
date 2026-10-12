import { defineMessages } from '../define'

/**
 * Backup → Recent runs → run detail sheet: tabs, files kept, safety copy, and
 * one sentence per recorded warning or note code (see `run-warnings.tsx`).
 */
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
    warnings: {
      'safari-full-disk-access-skip':
        'Skipped {profile}: PathKeep can’t read Safari’s history without Full Disk Access. Grant it, and the next backup includes Safari.',
      'profile-history-unreadable-skip':
        'Skipped {profile}: its history file is missing or can’t be read.',
      'profile-not-detected-skip':
        'Skipped {profile}: it’s no longer on this computer. What earlier backups saved from it stays in the archive.',
      'staging-fallback-recovered-copy':
        '{profile} was in use, so PathKeep read a copy of its history file instead of the live one.',
      'git-history-skipped':
        'This run’s records were saved, but adding them to the Git history failed.',
      'intelligence-refresh-failed':
        'Insights couldn’t start updating with this backup’s visits.',
      'ai-autoindex-enqueue-failed':
        'New pages couldn’t be queued for semantic search.',
      'ai-autoindex-provider-not-ready':
        'New pages weren’t queued for semantic search because its model isn’t ready.',
      'enrichment-reoffer-failed':
        'Pages waiting for their content to be fetched couldn’t be queued.',
      'og-refetch-failed':
        'Retrying link previews that failed before didn’t work.',
      'og-prefetch-failed': 'Link previews for new pages couldn’t be fetched.',
      'og-cleanup-failed': 'Cleaning up the link preview cache failed.',
      'review-record-failed':
        'Everything was saved, but the record of this run couldn’t be written.',
      'skipped-missing-timestamp_one':
        'Skipped {count} entry in the import file because it has no time.',
      'skipped-missing-timestamp_other':
        'Skipped {count} entries in the import file because they have no time.',
      'browser-skipped-missing-url-row_one':
        'Skipped {count} visit in the import file because its address is missing.',
      'browser-skipped-missing-url-row_other':
        'Skipped {count} visits in the import file because their address is missing.',
      'parse-failed': 'Part of the import couldn’t be read.',
      'no-importable-files':
        'The import had no history files PathKeep could read.',
      parser: 'The import file reported a problem.',
      other: 'Recorded during this run:',
    },
    notes: {
      title: 'Also during this run',
      'profile-search-terms-captured_one':
        'Saved {count} search typed in {profile}.',
      'profile-search-terms-captured_other':
        'Saved {count} searches typed in {profile}.',
      'og-prefetch-summary_one':
        'Link previews: fetched {succeeded} of {count} new page.',
      'og-prefetch-summary_other':
        'Link previews: fetched {succeeded} of {count} new pages.',
      'og-refetch-summary_one':
        'Link previews: retried {count} that failed before; {succeeded} worked this time.',
      'og-refetch-summary_other':
        'Link previews: retried {count} that failed before; {succeeded} worked this time.',
      'og-cleanup-summary_one':
        'Link preview cache: removed {count} old entry and freed {bytes}.',
      'og-cleanup-summary_other':
        'Link preview cache: removed {count} old entries and freed {bytes}.',
      'ai-autoindex-queued-while-paused':
        'New pages are queued for semantic search. They’ll be indexed when background work resumes.',
    },
    actions: {
      aiSettings: 'Go to AI settings',
      backgroundSettings: 'Go to Background work',
    },
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
    warnings: {
      'safari-full-disk-access-skip':
        '已跳过 {profile}：没有完全磁盘访问权限，PathKeep 读不到 Safari 的历史记录。授权后，下次备份就会包含 Safari。',
      'profile-history-unreadable-skip':
        '已跳过 {profile}：它的历史文件不存在或无法读取。',
      'profile-not-detected-skip':
        '已跳过 {profile}：这台电脑上已经找不到它。之前备份的内容仍在存档中。',
      'staging-fallback-recovered-copy':
        '{profile} 正在使用中，因此 PathKeep 读取的是历史文件的副本，而不是正在使用的文件。',
      'git-history-skipped': '这次运行的记录已保存，但未能写入 Git 历史。',
      'intelligence-refresh-failed': '洞察没能开始用这次备份的访问记录更新。',
      'ai-autoindex-enqueue-failed': '新页面没能加入语义搜索的索引队列。',
      'ai-autoindex-provider-not-ready':
        '语义搜索的模型还没准备好，因此新页面没有加入索引队列。',
      'enrichment-reoffer-failed': '等待抓取内容的页面没能加入队列。',
      'og-refetch-failed': '重试之前失败的链接预览时出错。',
      'og-prefetch-failed': '没能获取新页面的链接预览。',
      'og-cleanup-failed': '清理链接预览缓存失败。',
      'review-record-failed': '内容都已保存，但这次执行的记录没能写入。',
      'skipped-missing-timestamp_one':
        '跳过了导入文件中的 {count} 条记录，因为它们没有时间。',
      'skipped-missing-timestamp_other':
        '跳过了导入文件中的 {count} 条记录，因为它们没有时间。',
      'browser-skipped-missing-url-row_one':
        '跳过了导入文件中的 {count} 次访问，因为缺少网址。',
      'browser-skipped-missing-url-row_other':
        '跳过了导入文件中的 {count} 次访问，因为缺少网址。',
      'parse-failed': '导入的部分内容无法读取。',
      'no-importable-files': '导入中没有 PathKeep 能读取的历史文件。',
      parser: '导入文件报告了一个问题。',
      other: '这次运行记录了：',
    },
    notes: {
      title: '这次运行还做了',
      'profile-search-terms-captured_one':
        '保存了在 {profile} 中输入的 {count} 次搜索。',
      'profile-search-terms-captured_other':
        '保存了在 {profile} 中输入的 {count} 次搜索。',
      'og-prefetch-summary_one':
        '链接预览：{count} 个新页面中获取了 {succeeded} 个。',
      'og-prefetch-summary_other':
        '链接预览：{count} 个新页面中获取了 {succeeded} 个。',
      'og-refetch-summary_one':
        '链接预览：重试了 {count} 个之前失败的页面，这次成功 {succeeded} 个。',
      'og-refetch-summary_other':
        '链接预览：重试了 {count} 个之前失败的页面，这次成功 {succeeded} 个。',
      'og-cleanup-summary_one':
        '链接预览缓存：移除了 {count} 条旧记录，释放了 {bytes}。',
      'og-cleanup-summary_other':
        '链接预览缓存：移除了 {count} 条旧记录，释放了 {bytes}。',
      'ai-autoindex-queued-while-paused':
        '新页面已加入语义搜索的索引队列，后台任务恢复后就会建立索引。',
    },
    actions: {
      aiSettings: '前往 AI 设置',
      backgroundSettings: '前往后台任务',
    },
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
    warnings: {
      'safari-full-disk-access-skip':
        '已略過 {profile}：沒有完整磁碟取用權限，PathKeep 讀不到 Safari 的歷史記錄。授權後，下次備份就會包含 Safari。',
      'profile-history-unreadable-skip':
        '已略過 {profile}：它的歷史檔案不存在或無法讀取。',
      'profile-not-detected-skip':
        '已略過 {profile}：這台電腦上已經找不到它。之前備份的內容仍在存檔中。',
      'staging-fallback-recovered-copy':
        '{profile} 正在使用中，因此 PathKeep 讀取的是歷史檔案的副本，而不是正在使用的檔案。',
      'git-history-skipped': '這次執行的記錄已儲存，但未能寫入 Git 歷史。',
      'intelligence-refresh-failed': '洞察沒能開始用這次備份的造訪記錄更新。',
      'ai-autoindex-enqueue-failed': '新頁面沒能加入語意搜尋的索引佇列。',
      'ai-autoindex-provider-not-ready':
        '語意搜尋的模型還沒準備好，因此新頁面沒有加入索引佇列。',
      'enrichment-reoffer-failed': '等待抓取內容的頁面沒能加入佇列。',
      'og-refetch-failed': '重試之前失敗的連結預覽時出錯。',
      'og-prefetch-failed': '沒能取得新頁面的連結預覽。',
      'og-cleanup-failed': '清理連結預覽快取失敗。',
      'review-record-failed': '內容都已儲存，但這次執行的記錄沒能寫入。',
      'skipped-missing-timestamp_one':
        '略過了匯入檔案中的 {count} 筆記錄，因為它們沒有時間。',
      'skipped-missing-timestamp_other':
        '略過了匯入檔案中的 {count} 筆記錄，因為它們沒有時間。',
      'browser-skipped-missing-url-row_one':
        '略過了匯入檔案中的 {count} 次造訪，因為缺少網址。',
      'browser-skipped-missing-url-row_other':
        '略過了匯入檔案中的 {count} 次造訪，因為缺少網址。',
      'parse-failed': '匯入的部分內容無法讀取。',
      'no-importable-files': '匯入中沒有 PathKeep 能讀取的歷史檔案。',
      parser: '匯入檔案回報了一個問題。',
      other: '這次執行記錄了：',
    },
    notes: {
      title: '這次執行還做了',
      'profile-search-terms-captured_one':
        '儲存了在 {profile} 中輸入的 {count} 次搜尋。',
      'profile-search-terms-captured_other':
        '儲存了在 {profile} 中輸入的 {count} 次搜尋。',
      'og-prefetch-summary_one':
        '連結預覽：{count} 個新頁面中取得了 {succeeded} 個。',
      'og-prefetch-summary_other':
        '連結預覽：{count} 個新頁面中取得了 {succeeded} 個。',
      'og-refetch-summary_one':
        '連結預覽：重試了 {count} 個之前失敗的頁面，這次成功 {succeeded} 個。',
      'og-refetch-summary_other':
        '連結預覽：重試了 {count} 個之前失敗的頁面，這次成功 {succeeded} 個。',
      'og-cleanup-summary_one':
        '連結預覽快取：移除了 {count} 筆舊記錄，釋放了 {bytes}。',
      'og-cleanup-summary_other':
        '連結預覽快取：移除了 {count} 筆舊記錄，釋放了 {bytes}。',
      'ai-autoindex-queued-while-paused':
        '新頁面已加入語意搜尋的索引佇列，背景工作恢復後就會建立索引。',
    },
    actions: {
      aiSettings: '前往 AI 設定',
      backgroundSettings: '前往背景工作',
    },
  },
})
