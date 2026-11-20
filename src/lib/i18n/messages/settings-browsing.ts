import { defineMessages } from '../define'

/**
 * Settings › Browsing history (`data-spaces.md` §8): the two features that
 * contact websites, link previews' two options, and the one row General
 * keeps to point here.
 */
export const settingsBrowsing = defineMessages({
  en: {
    backupNote:
      'Which browser profiles to back up, and importing history from elsewhere, are on the backup page of Browsing history, not here.',
    openBackup: 'Open the backup page',
    open: 'Browsing history settings',
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
  },
  'zh-CN': {
    backupNote:
      '要备份哪些浏览器配置文件、从别处导入记录，都在浏览记录的备份页，不在这里。',
    openBackup: '打开备份页',
    open: '浏览记录设置',
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
  },
  'zh-TW': {
    backupNote:
      '要備份哪些瀏覽器設定檔、從別處匯入紀錄，都在瀏覽紀錄的備份頁，不在這裡。',
    openBackup: '打開備份頁',
    open: '瀏覽紀錄設定',
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
  },
})
