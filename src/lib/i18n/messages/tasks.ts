import { defineMessages } from '../define'

/**
 * The background tasks page (`#/tasks`): its header, the four groups and
 * their rows (browsing history's work and Agent conversations' backups
 * side by side), the queues below them, the row in Settings that leads
 * there, and the automatic-backup row on each space's backup page.
 * Conversation backups keep their own words (`agentTasks`); the queues
 * theirs (`settingsBackground`).
 */
export const tasks = defineMessages({
  en: {
    title: 'Background tasks',
    subtitle: {
      keepsGoing_one: '{count} running. Closing the window doesn’t stop it.',
      keepsGoing_other:
        '{count} running. Closing the window doesn’t stop them.',
      running_one: '{count} running.',
      running_other: '{count} running.',
      idle: 'Nothing is running now.',
    },
    schedule: 'Schedule and background work',
    groups: {
      attention: 'Needs a look',
      running: 'Running',
      waiting: 'Waiting',
      today: 'Done today',
      noneRunning: 'None.',
      noneRunningNext: 'None. Next scheduled backup: {time}.',
      noneRunningOff: 'None. Automatic backup is off.',
      noneToday: 'Nothing has finished today yet.',
      more: 'Show more',
      loading: 'Loading tasks…',
      agentUnavailable:
        'Saved Agent conversations can’t be opened right now, so their backups aren’t listed here. Browsing history is not affected.',
    },
    owner: {
      whole: 'Whole archive',
    },
    row: {
      progress: '{done} of {total}',
      done_one: '{count} done so far',
      done_other: '{count} done so far',
      youPaused: 'You paused it',
    },
    browsing: {
      backup: 'Back up browsing history',
      job: '{queue} · {name}',
      other: 'Browsing history: {code}',
      state: {
        partial: 'Didn’t finish everything',
        interrupted: 'Stopped before the end',
        other: '“{code}”',
      },
    },
    actions: {
      pause: 'Pause',
      pauseLabel: 'Pause “{name}”',
      resume: 'Continue',
      resumeLabel: 'Continue “{name}”',
      retry: 'Try again',
      retryLabel: 'Try “{name}” again',
      cancel: 'Cancel',
      cancelLabel: 'Cancel “{name}”',
    },
    queues: {
      title: 'Queues and data',
      note: 'The work PathKeep does on browsing history after each backup, and the data it builds. Backups never wait for it.',
    },
    settings: {
      oneSchedule: 'One schedule backs up every kind of data that is on.',
      workGroup: 'Background work',
      title: 'Background tasks',
      body: 'What runs, what waits, and what finished or didn’t today. The indicator at the bottom left opens it too.',
      open: 'Open background tasks',
    },
    scheduleRow: {
      title: 'Automatic backup',
      every: 'Every {interval}',
      next: '{interval} · next {time}',
      off: 'Automatic backup is off',
      failed: 'Couldn’t check automatic backup.',
      checking: 'Checking automatic backup…',
      change: 'Change schedule',
      set: 'Set a schedule',
      fix: 'Fix',
      steps: 'See steps',
    },
  },
  'zh-CN': {
    title: '后台任务',
    subtitle: {
      keepsGoing_one: '{count} 项在运行，关掉窗口也会继续。',
      keepsGoing_other: '{count} 项在运行，关掉窗口也会继续。',
      running_one: '{count} 项在运行。',
      running_other: '{count} 项在运行。',
      idle: '现在没有在运行的任务。',
    },
    schedule: '排程与后台工作',
    groups: {
      attention: '需要你看一下',
      running: '正在运行',
      waiting: '等待中',
      today: '今天完成',
      noneRunning: '没有。',
      noneRunningNext: '没有。下次排程备份在 {time}。',
      noneRunningOff: '没有。自动备份关着。',
      noneToday: '今天还没有做完的任务。',
      more: '显示更多',
      loading: '正在读取任务…',
      agentUnavailable:
        '已存的 Agent 对话现在打不开，所以这里没有列出它们的备份。浏览记录不受影响。',
    },
    owner: {
      whole: '整个存档',
    },
    row: {
      progress: '{done} / {total}',
      done_one: '已完成 {count} 项',
      done_other: '已完成 {count} 项',
      youPaused: '你暂停了',
    },
    browsing: {
      backup: '备份浏览记录',
      job: '{queue} · {name}',
      other: '浏览记录：{code}',
      state: {
        partial: '没有全部做完',
        interrupted: '中途停下了',
        other: '「{code}」',
      },
    },
    actions: {
      pause: '暂停',
      pauseLabel: '暂停「{name}」',
      resume: '继续',
      resumeLabel: '继续「{name}」',
      retry: '再试一次',
      retryLabel: '再试一次「{name}」',
      cancel: '取消',
      cancelLabel: '取消「{name}」',
    },
    queues: {
      title: '队列与数据',
      note: 'PathKeep 每次备份后对浏览记录做的工作，以及由此建立的数据。备份从不等这些工作。',
    },
    settings: {
      oneSchedule: '一个排程备份所有「开启」的类型。',
      workGroup: '后台工作',
      title: '后台任务',
      body: '正在运行、在排队、今天做完或没做完的任务。左下角的指示器也能打开。',
      open: '打开后台任务',
    },
    scheduleRow: {
      title: '自动备份',
      every: '每 {interval}',
      next: '{interval} · 下次 {time}',
      off: '自动备份关着',
      failed: '没能检查自动备份。',
      checking: '正在检查自动备份…',
      change: '更改排程',
      set: '设置排程',
      fix: '处理',
      steps: '看步骤',
    },
  },
  'zh-TW': {
    title: '背景任務',
    subtitle: {
      keepsGoing_one: '{count} 項在跑，關掉視窗也會繼續。',
      keepsGoing_other: '{count} 項在跑，關掉視窗也會繼續。',
      running_one: '{count} 項在跑。',
      running_other: '{count} 項在跑。',
      idle: '現在沒有在跑的任務。',
    },
    schedule: '排程與背景工作',
    groups: {
      attention: '需要你看一下',
      running: '正在跑',
      waiting: '等待中',
      today: '今天完成',
      noneRunning: '沒有。',
      noneRunningNext: '沒有。下次排程備份在 {time}。',
      noneRunningOff: '沒有。自動備份關著。',
      noneToday: '今天還沒有做完的任務。',
      more: '顯示更多',
      loading: '正在讀取任務…',
      agentUnavailable:
        '已存的 Agent 對話現在打不開，所以這裡沒有列出它們的備份。瀏覽紀錄不受影響。',
    },
    owner: {
      whole: '整個存檔',
    },
    row: {
      progress: '{done} / {total}',
      done_one: '已做完 {count} 項',
      done_other: '已做完 {count} 項',
      youPaused: '你暫停了',
    },
    browsing: {
      backup: '備份瀏覽紀錄',
      job: '{queue} · {name}',
      other: '瀏覽紀錄：{code}',
      state: {
        partial: '沒有全部做完',
        interrupted: '中途停下了',
        other: '「{code}」',
      },
    },
    actions: {
      pause: '暫停',
      pauseLabel: '暫停「{name}」',
      resume: '繼續',
      resumeLabel: '繼續「{name}」',
      retry: '再試一次',
      retryLabel: '再試一次「{name}」',
      cancel: '取消',
      cancelLabel: '取消「{name}」',
    },
    queues: {
      title: '佇列與資料',
      note: 'PathKeep 每次備份後對瀏覽紀錄做的工作，以及由此建立的資料。備份從不等這些工作。',
    },
    settings: {
      oneSchedule: '一個排程備份所有「開啟」的類型。',
      workGroup: '背景工作',
      title: '背景任務',
      body: '正在跑、在排隊、今天做完或沒做完的任務。左下角的指示器也能打開。',
      open: '打開背景任務',
    },
    scheduleRow: {
      title: '自動備份',
      every: '每 {interval}',
      next: '{interval} · 下次 {time}',
      off: '自動備份關著',
      failed: '沒能檢查自動備份。',
      checking: '正在檢查自動備份…',
      change: '更改排程',
      set: '設定排程',
      fix: '處理',
      steps: '看步驟',
    },
  },
})
