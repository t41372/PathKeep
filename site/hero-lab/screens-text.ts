/**
 * The words on the app screenshots in the page's other languages. The
 * screens are drawn in Traditional Chinese; `capture-screens.ts` swaps every
 * piece of text for its entry here before it takes the English and
 * Simplified Chinese pictures, and stops on any piece without one.
 *
 * Keys are the text as the screen holds it, one text node, trimmed. An
 * entry marked `app:` carries the app's own wording from that message in
 * `src/lib/i18n/messages`; the rest are written for these pictures, in the
 * same voice. English entries bring the spaces Chinese runs without.
 */

export interface Words {
  en: string
  'zh-CN': string
}

export const words: Record<string, Words> = {
  // The navigation and the space switcher, on every screen.
  // The switcher shows a space's short name.
  瀏覽紀錄: { en: 'Browsing', 'zh-CN': '浏览记录' }, // app: spaces.browsing.shortName
  首頁: { en: 'Home', 'zh-CN': '首页' }, // app: shell.nav.home
  紀錄: { en: 'History', 'zh-CN': '记录' }, // app: shell.nav.history
  洞察: { en: 'Insights', 'zh-CN': '洞察' }, // app: insights.title
  備份: { en: 'Backup', 'zh-CN': '备份' }, // app: backup.title
  助理: { en: 'Assistant', 'zh-CN': '助理' }, // app: ask.export.assistant
  設定: { en: 'Settings', 'zh-CN': '设置' }, // app: settings.title
  專案: { en: 'Projects', 'zh-CN': '项目' },
  搜尋與指令: { en: 'Search and commands', 'zh-CN': '搜索与指令' },
  '跳到頁面、對話或專案': {
    en: 'Jump to pages, conversations or projects',
    'zh-CN': '跳到页面、对话或项目',
  },

  // Browsing history, Home.
  '2026 年 11 月 13 日 星期五': {
    en: 'Friday, November 13, 2026',
    'zh-CN': '2026 年 11 月 13 日 星期五',
  },
  午安: { en: 'Good afternoon', 'zh-CN': '下午好' }, // app: home.greetingAfternoon
  '已保存 182 萬筆瀏覽紀錄，從 2019 年 1 月開始；這週新增 8,412 筆。': {
    en: '1.82 million visits saved since January 2019; 8,412 new this week.',
    'zh-CN':
      '已保存 182 万条浏览记录，从 2019 年 1 月开始；本周新增 8,412 条。',
  },
  '上次備份 今天 15:02': {
    en: 'Last backup today at 15:02',
    'zh-CN': '上次备份 今天 15:02',
  },
  立即備份: { en: 'Back up now', 'zh-CN': '立即备份' }, // app: shell.palette.backupNow
  '最近 30 天': { en: 'Last 30 days', 'zh-CN': '最近 30 天' }, // English as the app's 30-day card on Home
  '每天的造訪，共 11,877 筆': {
    en: 'Visits per day, 11,877 in all',
    'zh-CN': '每天的浏览，共 11,877 次',
  },
  往年今日: { en: 'On this day', 'zh-CN': '往年今日' }, // app: home.onThisDay.title
  '以前的 11 月 13 日，你在看什麼': {
    en: 'What you were reading on November 13 in other years',
    'zh-CN': '以前的 11 月 13 日，你在看什么',
  },
  '2025年11月13日': { en: 'Nov 13, 2025', 'zh-CN': '2025年11月13日' },
  '2024年11月13日': { en: 'Nov 13, 2024', 'zh-CN': '2024年11月13日' },
  '頁，最多在': { en: 'pages, mostly on', 'zh-CN': '页，最多在' },
  'github.com、docs.rs': {
    en: 'github.com, docs.rs',
    'zh-CN': 'github.com、docs.rs',
  },
  'www.youtube.com、zh.wikipedia.org': {
    en: 'www.youtube.com, zh.wikipedia.org',
    'zh-CN': 'www.youtube.com、zh.wikipedia.org',
  },
  '2026 閱讀清單': { en: '2026 reading list', 'zh-CN': '2026 阅读清单' },
  這一年: { en: 'Your year', 'zh-CN': '这一年' }, // English as the app's year card on Home
  每天瀏覽的頁數: { en: 'Pages viewed each day', 'zh-CN': '每天浏览的页数' },
  '2026 年共 40,997 頁': {
    en: '40,997 pages in 2026',
    'zh-CN': '2026 年共 40,997 页',
  },
  少: { en: 'Less', 'zh-CN': '少' }, // app: common.less
  多: { en: 'More', 'zh-CN': '多' }, // app: common.more

  // Agent conversations, Insights.
  全部時間: { en: 'all time', 'zh-CN': '全部时间' }, // app: insights.groups.all
  '（': { en: ' (', 'zh-CN': '（' },
  '）': { en: ')', 'zh-CN': '）' },
  '7 天': { en: '7d', 'zh-CN': '7 天' }, // app: insights.range.d7
  '30 天': { en: '30d', 'zh-CN': '30 天' }, // app: insights.range.d30
  '90 天': { en: '90d', 'zh-CN': '90 天' }, // app: insights.range.d90
  一年: { en: '1y', 'zh-CN': '一年' },
  全部: { en: 'All', 'zh-CN': '全部' }, // app: common.all
  篩選: { en: 'Filter', 'zh-CN': '筛选' },
  'Agent 工作時間': { en: 'Hours agents worked', 'zh-CN': 'Agent 工作时间' },
  '1,632 小時': { en: '1,632 hours', 'zh-CN': '1,632 小时' },
  '不眠不休 68 天': { en: '68 days nonstop', 'zh-CN': '不眠不休 68 天' },
  分開加總: { en: 'Summed separately', 'zh-CN': '分开加总' },
  '2,215 小時': { en: '2,215 hours', 'zh-CN': '2,215 小时' },
  '平均同時開 1.4 個': {
    en: '1.4 at a time on average',
    'zh-CN': '平均同时开 1.4 个',
  },
  你在場的時間: { en: 'Hours you were there', 'zh-CN': '你在场的时间' },
  '1,778 小時': { en: '1,778 hours', 'zh-CN': '1,778 小时' },
  '不眠不休 74 天': { en: '74 days nonstop', 'zh-CN': '不眠不休 74 天' },
  '估算：你在跟 Agent 來回的時段': {
    en: 'Estimated: the stretches you spent going back and forth with an agent',
    'zh-CN': '估算：你在跟 Agent 来回的时段',
  },
  '117 億': { en: '11.7 billion', 'zh-CN': '117 亿' },
  段對話有: { en: 'conversations have ', 'zh-CN': '段对话有' },
  用量: { en: 'usage', 'zh-CN': '用量' },
  '費用（按 API 牌價）': {
    en: 'Cost (at API list prices)',
    'zh-CN': '费用（按 API 牌价）',
  },
  '工具記下 US$2,228 · 估算 US$10,382': {
    en: 'Recorded by tools US$2,228 · estimated US$10,382',
    'zh-CN': '工具记下 US$2,228 · 估算 US$10,382',
  },
  對話: { en: 'Conversations', 'zh-CN': '对话' }, // app: ask.list.label
  '另有 9,948 個 subagent · 297 個專案': {
    en: 'plus 9,948 subagents · 297 projects',
    'zh-CN': '另有 9,948 个 subagent · 297 个项目',
  },
  '每天的 Agent 工作時間': {
    en: 'Hours agents worked, day by day',
    'zh-CN': '每天的 Agent 工作时间',
  },
  '一年裡的每一天，同時開的只算一次。不受上面的時間範圍影響': {
    en: 'Every day of the year, with overlapping sessions counted once. The range above does not apply.',
    'zh-CN': '一年里的每一天，同时开的只算一次。不受上面的时间范围影响',
  },
  '2026 年': { en: '2026', 'zh-CN': '2026 年' },
  'Agent 工作': { en: 'Agents worked', 'zh-CN': 'Agent 工作' },
  '1,024 小時': { en: '1,024 hours', 'zh-CN': '1,024 小时' },
  同時開的只算一次: {
    en: 'overlaps counted once',
    'zh-CN': '同时开的只算一次',
  },
  有對話的日子: { en: 'Days with conversations', 'zh-CN': '有对话的日子' },
  '288 天': { en: '288 days', 'zh-CN': '288 天' },
  '今年到今天 317 天': {
    en: 'of 317 so far this year',
    'zh-CN': '今年到今天 317 天',
  },
  每天平均: { en: 'Daily average', 'zh-CN': '每天平均' },
  '3.6 小時': { en: '3.6 hours', 'zh-CN': '3.6 小时' },
  只算有對話的日子: {
    en: 'on days with conversations',
    'zh-CN': '只算有对话的日子',
  },
  最長連續: { en: 'Longest streak', 'zh-CN': '最长连续' },
  '52 天': { en: '52 days', 'zh-CN': '52 天' },
  '9月17日 – 11月7日': { en: 'Sep 17 – Nov 7', 'zh-CN': '9月17日 – 11月7日' },
  最忙的一天: { en: 'Busiest day', 'zh-CN': '最忙的一天' },
  '23 小時': { en: '23 hours', 'zh-CN': '23 小时' },
  '5月21日': { en: 'May 21', 'zh-CN': '5月21日' },
  最忙的月份: { en: 'Busiest month', 'zh-CN': '最忙的月份' },
  '5 月': { en: 'May', 'zh-CN': '5 月' },
  '157 小時': { en: '157 hours', 'zh-CN': '157 小时' },
  'Agent 工作的時間': { en: 'When agents worked', 'zh-CN': 'Agent 工作的时间' },
  '全部時間每月，單位：小時': {
    en: 'Each month, all time, in hours',
    'zh-CN': '全部时间每月，单位：小时',
  },
  時間: { en: 'Hours', 'zh-CN': '时间' },
  'Agent 工作時間（同時開的只算一次）': {
    en: 'Hours agents worked (overlaps counted once)',
    'zh-CN': 'Agent 工作时间（同时开的只算一次）',
  },
  '分開加總（同時開的各算各的）': {
    en: 'Summed separately (each session counted)',
    'zh-CN': '分开加总（同时开的各算各的）',
  },
  依專案: { en: 'By project', 'zh-CN': '按项目' },
  '全部時間，Agent 花最多時間的專案（分開加總）': {
    en: 'All time, the projects agents spent the most hours on (summed separately)',
    'zh-CN': '全部时间，Agent 花最多时间的项目（分开加总）',
  },
  個: { en: '', 'zh-CN': '个' },
  '150 段 · 7.84 億 token': {
    en: '150 conversations · 784 million tokens',
    'zh-CN': '150 段 · 7.84 亿 token',
  },
  '95 段 · 2.33 億 token': {
    en: '95 conversations · 233 million tokens',
    'zh-CN': '95 段 · 2.33 亿 token',
  },
  '53 段 · 4.82 億 token': {
    en: '53 conversations · 482 million tokens',
    'zh-CN': '53 段 · 4.82 亿 token',
  },
  '80 段 · 3.17 億 token': {
    en: '80 conversations · 317 million tokens',
    'zh-CN': '80 段 · 3.17 亿 token',
  },
  '358 小時': { en: '358 hours', 'zh-CN': '358 小时' },
  '160 小時': { en: '160 hours', 'zh-CN': '160 小时' },
  '91 小時': { en: '91 hours', 'zh-CN': '91 小时' },

  // Agent conversations, History.
  '今天 · 11月13日週五': {
    en: 'Today · Fri, Nov 13',
    'zh-CN': '今天 · 11月13日周五',
  },
  '昨天 · 11月12日週四': {
    en: 'Yesterday · Thu, Nov 12',
    'zh-CN': '昨天 · 11月12日周四',
  },
  '11月11日週三': { en: 'Wed, Nov 11', 'zh-CN': '11月11日周三' },
  段: { en: '', 'zh-CN': '段' },
  '把這個資料夾裡的收據整理成表格，依類別加總，金額對不上的標出來': {
    en: 'Put the receipts in this folder into a table, total them by category and flag the amounts that don’t add up',
    'zh-CN': '把这个文件夹里的收据整理成表格，按类别加总，金额对不上的标出来',
  },
  採訪逐字稿: { en: 'interview-transcripts', 'zh-CN': '采访逐字稿' },
  清掉沒用到的依賴: {
    en: 'Remove the dependencies we don’t use',
    'zh-CN': '清掉没用到的依赖',
  },
  '把 Tauri 升到最新版，列出 breaking changes 跟我們要改的地方，先': {
    en: 'Upgrade Tauri to the latest version, list the breaking changes and what we have to change, then',
    'zh-CN': '把 Tauri 升到最新版，列出 breaking changes 跟我们要改的地方，先',
  },
  '修 dark mode 對比不足': {
    en: 'Fix the weak contrast in dark mode',
    'zh-CN': '修 dark mode 对比不足',
  },
  選一段對話來看: {
    en: 'Pick a conversation to read',
    'zh-CN': '选一段对话来看',
  },
  切換對話: { en: 'Switch conversations', 'zh-CN': '切换对话' },
  搜尋全部對話: { en: 'Search all conversations', 'zh-CN': '搜索全部对话' },
  跳到任何地方: { en: 'Jump anywhere', 'zh-CN': '跳到任何地方' },
  '搜尋所有對話…': {
    en: 'Search all conversations…',
    'zh-CN': '搜索所有对话…',
  },

  // The browsing records page.
  日期: { en: 'Date', 'zh-CN': '日期' },
  所有瀏覽器: { en: 'All browsers', 'zh-CN': '所有浏览器' }, // app: history.browser.all
  時間線: { en: 'Timeline', 'zh-CN': '时间线' }, // app: history.views.timeline
  網站: { en: 'Sites', 'zh-CN': '网站' }, // app: history.views.sites
  已加星: { en: 'Starred', 'zh-CN': '已加星' }, // app: history.views.starred
  筆: { en: 'visits', 'zh-CN': '笔' },
  '時刻表與票價 | 台灣高鐵': {
    en: 'Timetable and fares | Taiwan High Speed Rail',
    'zh-CN': '时刻表与票价 | 台湾高铁',
  },
  'sqlcipher rekey progress - Google 搜尋': {
    en: 'sqlcipher rekey progress - Google Search',
    'zh-CN': 'sqlcipher rekey progress - Google 搜索',
  },
  '搜尋標題、網址、筆記': {
    en: 'Search titles, URLs, notes',
    'zh-CN': '搜索标题、网址、笔记',
  },
}

/**
 * Pieces that read differently inside one phrase than on their own, keyed
 * by the whole phrase as its element holds it (spaces collapsed).
 */
export const inPhrase: Record<string, Record<string, Words>> = {
  '1,757 / 1,942 段對話有用量紀錄': {
    紀錄: { en: ' records', 'zh-CN': '记录' },
  },
}
