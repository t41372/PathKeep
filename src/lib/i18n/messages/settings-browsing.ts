import { defineMessages } from '../define'

/**
 * Settings › Browsing history (`data-spaces.md` §8): the two features that
 * contact websites, link previews' two options, the preview images kept,
 * exporting the history, and the one row General keeps to point here.
 */
export const settingsBrowsing = defineMessages({
  en: {
    backupNote:
      'Which browser profiles to back up, and importing history from elsewhere, are on the backup page of Browsing history, not here.',
    openBackup: 'Open the backup page',
    open: 'Browsing history settings',
    exportGroup: 'Export',
    off: 'Browsing history is off, so it has no settings here. Turn it on in Manage data types.',
    online: {
      title: 'Online',
      note: 'These two contact websites. Everything else stays on this computer.',
      elsewhere:
        'Only link previews and page summaries, in the settings of Browsing history, contact websites. Everything else stays on this computer.',
    },
    previews: {
      title: 'Link previews',
      description:
        'Download the preview image each page offers. The site sees a request from your computer.',
      options: 'Link preview options',
    },
    summaries: {
      title: 'Page summaries',
      description:
        'Fetch a short summary of pages you visited, so search can find them by what they say. Sites never get your cookies or accounts.',
      loadFailed: 'Could not load this setting.',
    },
    images: {
      title: 'Link preview images',
      description_one: '{count} image, {size}',
      description_other: '{count} images, {size}',
      statsFailed: 'Could not measure the preview images.',
      action: 'Clear…',
      confirmTitle: 'Delete all link preview images?',
      confirmBody_one:
        '{count} image ({size}) is deleted. PathKeep downloads previews again only if link previews are on.',
      confirmBody_other:
        '{count} images ({size}) are deleted. PathKeep downloads previews again only if link previews are on.',
      cleared: 'Freed {size}.',
      clearFailed: 'Could not delete the preview images',
    },
    export: {
      title: 'Export history',
      description: 'Save your whole history as one file',
      format: 'Export format',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: 'Plain text',
        jsonl: 'JSON Lines',
      },
      action: 'Export…',
      stop: 'Stop',
      stopping: 'Stopping…',
      cancelled: 'Export stopped. No file was saved.',
      progressLabel: 'Export progress',
      progress: '{written} of about {total} visits · {size}',
      progressNoTotal: '{written} visits · {size}',
      finishing: 'Saving the file…',
      leftUnderMinute: 'Less than a minute left',
      left_one: 'About {count} minute left',
      left_other: 'About {count} minutes left',
      done_one: 'Exported {count} visit.',
      done_other: 'Exported {count} visits.',
      failed: 'Export failed',
    },
  },
  'zh-CN': {
    backupNote:
      '要备份哪些浏览器配置文件、从别处导入记录，都在浏览记录的备份页，不在这里。',
    openBackup: '打开备份页',
    open: '浏览记录设置',
    exportGroup: '导出',
    off: '浏览记录已关闭，所以这里没有它的设置。可以在“管理数据类型”里打开。',
    online: {
      title: '联网',
      note: '只有这两项会访问网站，其他一切都留在这台电脑上。',
      elsewhere:
        '只有浏览记录设置里的链接预览和页面摘要会访问网站，其他一切都留在这台电脑上。',
    },
    previews: {
      title: '链接预览',
      description: '下载每个页面提供的预览图。网站会看到来自你电脑的请求。',
      options: '链接预览的选项',
    },
    summaries: {
      title: '页面摘要',
      description:
        '抓取你看过的页面的简短摘要，让搜索能按内容找到它们。网站不会拿到你的 Cookie 或账号。',
      loadFailed: '无法加载这项设置。',
    },
    images: {
      title: '链接预览图',
      description_one: '{count} 张图片，{size}',
      description_other: '{count} 张图片，{size}',
      statsFailed: '无法计算预览图占用的空间。',
      action: '清除…',
      confirmTitle: '要删除所有链接预览图吗？',
      confirmBody_one:
        '将删除 {count} 张图片（{size}）。只有开启链接预览时，PathKeep 才会重新下载。',
      confirmBody_other:
        '将删除 {count} 张图片（{size}）。只有开启链接预览时，PathKeep 才会重新下载。',
      cleared: '已释放 {size}。',
      clearFailed: '无法删除预览图',
    },
    export: {
      title: '导出历史',
      description: '把你的全部历史保存为一个文件',
      format: '导出格式',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: '纯文本',
        jsonl: 'JSON Lines',
      },
      action: '导出…',
      stop: '停止',
      stopping: '正在停止…',
      cancelled: '导出已停止，没有保存文件。',
      progressLabel: '导出进度',
      progress: '已写入 {written} / 约 {total} 条记录 · {size}',
      progressNoTotal: '已写入 {written} 条记录 · {size}',
      finishing: '正在保存文件…',
      leftUnderMinute: '剩余不到一分钟',
      left_one: '大约还要 {count} 分钟',
      left_other: '大约还要 {count} 分钟',
      done_one: '已导出 {count} 条记录。',
      done_other: '已导出 {count} 条记录。',
      failed: '导出失败',
    },
  },
  'zh-TW': {
    backupNote:
      '要備份哪些瀏覽器設定檔、從別處匯入紀錄，都在瀏覽紀錄的備份頁，不在這裡。',
    openBackup: '打開備份頁',
    open: '瀏覽紀錄設定',
    exportGroup: '匯出',
    off: '瀏覽紀錄已關閉，所以這裡沒有它的設定。可以在「管理資料類型」裡打開。',
    online: {
      title: '連網',
      note: '只有這兩項會連線到網站，其他一切都留在這台電腦上。',
      elsewhere:
        '只有瀏覽紀錄設定裡的連結預覽和頁面摘要會連線到網站，其他一切都留在這台電腦上。',
    },
    previews: {
      title: '連結預覽',
      description: '下載每個頁面提供的預覽圖。網站會看到來自你電腦的請求。',
      options: '連結預覽的選項',
    },
    summaries: {
      title: '頁面摘要',
      description:
        '抓取你看過的頁面的簡短摘要，讓搜尋能依內容找到它們。網站不會拿到你的 Cookie 或帳號。',
      loadFailed: '無法載入這項設定。',
    },
    images: {
      title: '連結預覽圖',
      description_one: '{count} 張圖片，{size}',
      description_other: '{count} 張圖片，{size}',
      statsFailed: '無法計算預覽圖佔用的空間。',
      action: '清除…',
      confirmTitle: '要刪除所有連結預覽圖嗎？',
      confirmBody_one:
        '將刪除 {count} 張圖片（{size}）。只有開啟連結預覽時，PathKeep 才會重新下載。',
      confirmBody_other:
        '將刪除 {count} 張圖片（{size}）。只有開啟連結預覽時，PathKeep 才會重新下載。',
      cleared: '已釋放 {size}。',
      clearFailed: '無法刪除預覽圖',
    },
    export: {
      title: '匯出歷史',
      description: '把你的全部歷史存成一個檔案',
      format: '匯出格式',
      formats: {
        html: 'HTML',
        markdown: 'Markdown',
        text: '純文字',
        jsonl: 'JSON Lines',
      },
      action: '匯出…',
      stop: '停止',
      stopping: '正在停止…',
      cancelled: '匯出已停止，沒有儲存檔案。',
      progressLabel: '匯出進度',
      progress: '已寫入 {written} / 約 {total} 筆記錄 · {size}',
      progressNoTotal: '已寫入 {written} 筆記錄 · {size}',
      finishing: '正在儲存檔案…',
      leftUnderMinute: '剩不到一分鐘',
      left_one: '大約還要 {count} 分鐘',
      left_other: '大約還要 {count} 分鐘',
      done_one: '已匯出 {count} 筆記錄。',
      done_other: '已匯出 {count} 筆記錄。',
      failed: '匯出失敗',
    },
  },
})
