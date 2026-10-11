import { defineMessages } from '../define'

export const home = defineMessages({
  en: {
    greetingMorning: 'Good morning',
    greetingAfternoon: 'Good afternoon',
    greetingEvening: 'Good evening',
    title: '{greeting}',
    alerts: {
      label: 'Needs your attention',
      openBackup: 'Open Backup',
      openSecurity: 'Open Security settings',
      scheduleFallback: 'Automatic backup needs attention',
      fullDiskAccess: {
        title: 'Safari’s history can’t be read',
        detail:
          'macOS keeps Safari’s history from PathKeep until you give it Full Disk Access. Until then Safari is not backed up.',
      },
      discoveryError: {
        title: 'PathKeep couldn’t look for your browsers',
        detail:
          'The browsers on this computer could not be listed, so new history may be missed.',
      },
      keychainUnavailable: {
        title: 'The keychain isn’t available',
        detail:
          'The archive password can’t be saved, so PathKeep asks for it every time it starts, and automatic backups can’t run without you.',
      },
      keyMissing: {
        title: 'The archive password is no longer in the keychain',
        detail:
          'PathKeep will ask for it the next time it starts, and automatic backups can’t open the archive until it is saved again.',
      },
      passwordRisk: {
        title: 'Only you have the archive password',
        detail:
          'The archive is encrypted and its password isn’t in the keychain. If the password is lost, nothing can open the archive, PathKeep included.',
        noKeychain:
          'The archive is encrypted, and this computer has no keychain to save its password in. If the password is lost, nothing can open the archive, PathKeep included. Keep it in a password manager, or written down somewhere safe.',
        keep: 'Save it in the keychain',
        dismiss: 'I keep it safe',
      },
    },
    stats: {
      saved: 'Saved visits',
      savedSub_one: '{count} new this week',
      savedSub_other: '{count} new this week',
      span: 'Time span',
      spanYearsMonths: '{years} yr {months} mo',
      spanMonths_one: '{count} month',
      spanMonths_other: '{count} months',
      spanDays_one: '{count} day',
      spanDays_other: '{count} days',
      spanSince: 'Since {date}',
      size: 'Archive size',
      encrypted: 'Encrypted',
      notEncrypted: 'Not encrypted',
      sources: 'Sources',
      sourcesValue_one: '{count} browser',
      sourcesValue_other: '{count} browsers',
      sourcesPaused_one: '{count} paused',
      sourcesPaused_other: '{count} paused',
      sourcesAllOn: 'All backing up',
    },
    building: {
      first: 'Working out your Insights for the first time…',
      update: 'Updating your Insights…',
    },
    trend: {
      title: 'Last 30 days',
      subtitle: 'Visits per day, all browsers',
      visits: 'Visits',
      vsPrevious: 'vs. the 30 days before',
    },
    onThisDay: {
      title: 'On this day',
      subtitle: 'What you were reading on {date}',
      meta: '{visits} · {domains}',
      empty:
        'Nothing from this date in earlier years yet. Come back next year.',
    },
    year: {
      title: 'Your year',
      subtitle: 'Each square is a day. Click one to see that day.',
      previous: 'Previous year',
      next: 'Next year',
      summary: '{total} visits in {year}',
      busiest: 'busiest: {date}',
      cell: '{date} · {visits}',
      cellEmpty: '{date} · no visits',
    },
    threads: {
      title: 'Things you keep coming back to',
      all: 'All',
      meta: '{days} · {visits}',
      days_one: '{count} day',
      days_other: '{count} days',
      empty: 'Topics you return to over several days will show up here.',
    },
    sources: {
      title: 'Sources',
      manage: 'Manage',
      paused: 'Paused',
      empty: 'No browsers are being backed up yet.',
    },
  },
  'zh-CN': {
    greetingMorning: '早上好',
    greetingAfternoon: '下午好',
    greetingEvening: '晚上好',
    title: '{greeting}，这是你走过的路',
    alerts: {
      label: '需要你处理',
      openBackup: '打开备份',
      openSecurity: '打开安全设置',
      scheduleFallback: '自动备份需要处理',
      fullDiskAccess: {
        title: '读不到 Safari 的历史',
        detail:
          '在你给 PathKeep 完全磁盘访问权限之前，macOS 不让它读 Safari 的历史，这期间 Safari 不会被备份。',
      },
      discoveryError: {
        title: 'PathKeep 找不到你的浏览器',
        detail: '没能列出这台电脑上的浏览器，新的历史可能会漏掉。',
      },
      keychainUnavailable: {
        title: '钥匙串不可用',
        detail:
          '存不了存档的密码，所以 PathKeep 每次启动都要问，自动备份也没法在你不在时运行。',
      },
      keyMissing: {
        title: '存档的密码已经不在钥匙串里',
        detail:
          'PathKeep 下次启动会问密码，自动备份在重新存好之前也打不开存档。',
      },
      passwordRisk: {
        title: '存档的密码只有你知道',
        detail:
          '存档已加密，密码不在钥匙串里。密码一旦丢了，谁都打不开存档，PathKeep 也不行。',
        noKeychain:
          '存档已加密，这台电脑上也没有能存密码的钥匙串。密码一旦丢了，谁都打不开存档，PathKeep 也不行。请把它存在密码管理器里，或写下来放在安全的地方。',
        keep: '存到钥匙串',
        dismiss: '我有妥善保管',
      },
    },
    stats: {
      saved: '已保存记录',
      savedSub_one: '本周新增 {count}',
      savedSub_other: '本周新增 {count}',
      span: '时间跨度',
      spanYearsMonths: '{years} 年 {months} 个月',
      spanMonths_one: '{count} 个月',
      spanMonths_other: '{count} 个月',
      spanDays_one: '{count} 天',
      spanDays_other: '{count} 天',
      spanSince: '最早 {date}',
      size: '存档大小',
      encrypted: '已加密',
      notEncrypted: '未加密',
      sources: '来源',
      sourcesValue_one: '{count} 个浏览器',
      sourcesValue_other: '{count} 个浏览器',
      sourcesPaused_one: '{count} 个已暂停',
      sourcesPaused_other: '{count} 个已暂停',
      sourcesAllOn: '全部在备份',
    },
    building: {
      first: '正在第一次整理你的洞察…',
      update: '正在更新你的洞察…',
    },
    trend: {
      title: '近 30 天浏览',
      subtitle: '每日浏览次数，所有浏览器',
      visits: '浏览',
      vsPrevious: '与前 30 天相比',
    },
    onThisDay: {
      title: '往年今日',
      subtitle: '{date}，你在看什么',
      meta: '{visits} · {domains}',
      empty: '往年的今天还没有记录。明年再来看看。',
    },
    year: {
      title: '一年的浏览',
      subtitle: '每格是一天，点一下查看那天',
      previous: '上一年',
      next: '下一年',
      summary: '{year} 年共 {total} 次浏览',
      busiest: '最活跃：{date}',
      cell: '{date} · {visits}',
      cellEmpty: '{date} · 没有浏览',
    },
    threads: {
      title: '反复回来看的主题',
      all: '全部',
      meta: '{days} · {visits}',
      days_one: '{count} 天',
      days_other: '{count} 天',
      empty: '连续几天都在看的主题会出现在这里。',
    },
    sources: {
      title: '来源',
      manage: '管理',
      paused: '已暂停',
      empty: '还没有在备份任何浏览器。',
    },
  },
  'zh-TW': {
    greetingMorning: '早安',
    greetingAfternoon: '午安',
    greetingEvening: '晚安',
    title: '{greeting}，這是你走過的路',
    alerts: {
      label: '需要你處理',
      openBackup: '打開備份',
      openSecurity: '打開安全性設定',
      scheduleFallback: '自動備份需要處理',
      fullDiskAccess: {
        title: '讀不到 Safari 的歷史',
        detail:
          '在你給 PathKeep 完整磁碟取用權限之前，macOS 不讓它讀 Safari 的歷史，這段期間 Safari 不會被備份。',
      },
      discoveryError: {
        title: 'PathKeep 找不到你的瀏覽器',
        detail: '沒能列出這台電腦上的瀏覽器，新的歷史可能會漏掉。',
      },
      keychainUnavailable: {
        title: '鑰匙圈無法使用',
        detail:
          '存不了存檔的密碼，所以 PathKeep 每次啟動都要問，自動備份也沒辦法在你不在時執行。',
      },
      keyMissing: {
        title: '存檔的密碼已經不在鑰匙圈裡',
        detail:
          'PathKeep 下次啟動會問密碼，自動備份在重新存好之前也打不開存檔。',
      },
      passwordRisk: {
        title: '存檔的密碼只有你知道',
        detail:
          '存檔已加密，密碼不在鑰匙圈裡。密碼一旦遺失，誰都打不開存檔，PathKeep 也不行。',
        noKeychain:
          '存檔已加密，這台電腦上也沒有能存密碼的鑰匙圈。密碼一旦遺失，誰都打不開存檔，PathKeep 也不行。請把它存在密碼管理器裡，或寫下來放在安全的地方。',
        keep: '存到鑰匙圈',
        dismiss: '我有妥善保管',
      },
    },
    stats: {
      saved: '已保存記錄',
      savedSub_one: '本週新增 {count}',
      savedSub_other: '本週新增 {count}',
      span: '時間跨度',
      spanYearsMonths: '{years} 年 {months} 個月',
      spanMonths_one: '{count} 個月',
      spanMonths_other: '{count} 個月',
      spanDays_one: '{count} 天',
      spanDays_other: '{count} 天',
      spanSince: '最早 {date}',
      size: '存檔大小',
      encrypted: '已加密',
      notEncrypted: '未加密',
      sources: '來源',
      sourcesValue_one: '{count} 個瀏覽器',
      sourcesValue_other: '{count} 個瀏覽器',
      sourcesPaused_one: '{count} 個已暫停',
      sourcesPaused_other: '{count} 個已暫停',
      sourcesAllOn: '全部在備份',
    },
    building: {
      first: '正在第一次整理你的洞察…',
      update: '正在更新你的洞察…',
    },
    trend: {
      title: '近 30 天瀏覽',
      subtitle: '每日瀏覽次數，所有瀏覽器',
      visits: '瀏覽',
      vsPrevious: '與前 30 天相比',
    },
    onThisDay: {
      title: '往年今日',
      subtitle: '{date}，你在看什麼',
      meta: '{visits} · {domains}',
      empty: '往年的今天還沒有記錄。明年再來看看。',
    },
    year: {
      title: '一年的瀏覽',
      subtitle: '每格是一天，點一下查看那天',
      previous: '上一年',
      next: '下一年',
      summary: '{year} 年共 {total} 次瀏覽',
      busiest: '最活躍：{date}',
      cell: '{date} · {visits}',
      cellEmpty: '{date} · 沒有瀏覽',
    },
    threads: {
      title: '反覆回來看的主題',
      all: '全部',
      meta: '{days} · {visits}',
      days_one: '{count} 天',
      days_other: '{count} 天',
      empty: '連續幾天都在看的主題會出現在這裡。',
    },
    sources: {
      title: '來源',
      manage: '管理',
      paused: '已暫停',
      empty: '還沒有在備份任何瀏覽器。',
    },
  },
})
