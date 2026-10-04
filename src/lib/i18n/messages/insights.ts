import { defineMessages } from '../define'

export const insights = defineMessages({
  en: {
    title: 'Insights',
    subtitle: 'Computed from your local archive. Nothing leaves this computer.',
    range: {
      label: 'Time range',
      d7: '7d',
      d30: '30d',
      d90: '90d',
      y1: '1y',
    },
    kpi: {
      visits: 'Page visits',
      domains: 'Distinct sites',
      searches: 'Searches',
      activeTime: 'Active time',
      vsPrevious: 'Compared with the previous {days} days',
      hours: '{value} h',
      minutes: '{value} min',
    },
    daily: {
      title: 'Daily activity',
      pages: 'Pages',
      searches: 'Searches',
    },
    topSites: {
      title: 'Top sites',
      empty: 'No visits in this range.',
    },
    rhythm: {
      title: 'Rhythm',
      subtitle: 'Visits by hour of the day, across the week',
      cell: '{day} {hour}:00 · {visits}',
      weekdays: 'Mon,Tue,Wed,Thu,Fri,Sat,Sun',
    },
    searches: {
      title: 'Frequent searches',
      empty: 'No searches in this range.',
    },
    refind: {
      title: 'Pages you keep reopening',
      times: '{count}×',
      days_one: 'on {count} day',
      days_other: 'on {count} days',
      empty: 'Pages you come back to on different days will show up here.',
    },
    stale: 'Updating after the last backup…',
    notReady:
      'Insights are calculated after a backup. Run one and they will fill in.',
  },
  'zh-CN': {
    title: '洞察',
    subtitle: '全部来自本机存档，不经过网络',
    range: {
      label: '时间范围',
      d7: '7 天',
      d30: '30 天',
      d90: '90 天',
      y1: '1 年',
    },
    kpi: {
      visits: '页面浏览',
      domains: '不同网站',
      searches: '搜索次数',
      activeTime: '活跃时间',
      vsPrevious: '与前 {days} 天相比',
      hours: '{value} 小时',
      minutes: '{value} 分钟',
    },
    daily: {
      title: '每日活动',
      pages: '页面',
      searches: '搜索',
    },
    topSites: {
      title: '最常浏览',
      empty: '这段时间没有浏览记录。',
    },
    rhythm: {
      title: '浏览节奏',
      subtitle: '一周内每小时的浏览量',
      cell: '周{day} {hour} 点 · {visits}',
      weekdays: '一,二,三,四,五,六,日',
    },
    searches: {
      title: '常用搜索词',
      empty: '这段时间没有搜索。',
    },
    refind: {
      title: '反复打开的页面',
      times: '{count} 次',
      days_one: '分布在 {count} 天',
      days_other: '分布在 {count} 天',
      empty: '在不同日子反复打开的页面会出现在这里。',
    },
    stale: '正在根据上次备份更新…',
    notReady: '洞察会在备份之后计算。做一次备份就会出现。',
  },
  'zh-TW': {
    title: '洞察',
    subtitle: '全部來自本機存檔，不經過網路',
    range: {
      label: '時間範圍',
      d7: '7 天',
      d30: '30 天',
      d90: '90 天',
      y1: '1 年',
    },
    kpi: {
      visits: '頁面瀏覽',
      domains: '不同網站',
      searches: '搜尋次數',
      activeTime: '活躍時間',
      vsPrevious: '與前 {days} 天相比',
      hours: '{value} 小時',
      minutes: '{value} 分鐘',
    },
    daily: {
      title: '每日活動',
      pages: '頁面',
      searches: '搜尋',
    },
    topSites: {
      title: '最常瀏覽',
      empty: '這段時間沒有瀏覽記錄。',
    },
    rhythm: {
      title: '瀏覽節奏',
      subtitle: '一週內每小時的瀏覽量',
      cell: '週{day} {hour} 點 · {visits}',
      weekdays: '一,二,三,四,五,六,日',
    },
    searches: {
      title: '常用搜尋詞',
      empty: '這段時間沒有搜尋。',
    },
    refind: {
      title: '反覆打開的頁面',
      times: '{count} 次',
      days_one: '分布在 {count} 天',
      days_other: '分布在 {count} 天',
      empty: '在不同日子反覆打開的頁面會出現在這裡。',
    },
    stale: '正在根據上次備份更新…',
    notReady: '洞察會在備份之後計算。做一次備份就會出現。',
  },
})
