import { defineMessages } from '../define'

/**
 * A backup of Agent conversations as it runs and once it ended, wherever it
 * shows: the enable dialog's save step, the backup page's header, the
 * rail's task list and Settings › Background work. Its progress in the
 * units it has at each moment, how each tool's folder fared, what was not
 * saved (`warnings`) and what a saved file's notes mean (`notes`), a
 * format notice, the task's name, states and buttons, and the notice when
 * it ends.
 */
export const agentTasks = defineMessages({
  en: {
    names: {
      backup: 'Back up Agent conversations',
      firstSave: 'Save Agent conversations for the first time',
    },
    progress: {
      waiting: 'Waiting to start',
      looking: 'Looking for files',
      found_one: '{count} file found so far',
      found_other: '{count} files found so far',
      files_one: '{count} file',
      files_other: '{count} files',
      saved: '{files} saved, {size}',
      conversations_one: '{count} conversation',
      conversations_other: '{count} conversations',
      subagents_one: '{count} subagent',
      subagents_other: '{count} subagents',
      indexing: 'Making what was saved searchable',
      finishing: 'Finishing up',
      stopping: 'Stopping',
      join: ' · ',
    },
    availability: {
      available: 'Read in full.',
      disabled: 'Switched off: not backed up.',
      stale: 'Not read yet.',
      unobserved: 'Not read in this backup.',
      partial: 'Some of its folders were read, some weren’t.',
      rootMissing: 'Its folder wasn’t there.',
      rootUnreadable:
        'PathKeep couldn’t read its folder: no permission, or the disk isn’t connected. That doesn’t mean its files are gone.',
      rootChanged:
        'Its folder was replaced by a different one since it was approved, so PathKeep didn’t read it. Look at it again before backing it up.',
      unknownFormat:
        'Some files are in a form PathKeep doesn’t read yet. They were saved as they are and will be sorted into conversations once it does.',
      changing:
        'Some files were being written while it ran. They’ll be read next time.',
      unreadable: 'Some files couldn’t be read. What could be read is saved.',
      destination: 'The disk the archive is on wasn’t available.',
      other: 'It ended with “{code}”.',
    },
    warnings: {
      missing: 'The folder wasn’t there.',
      unreadable:
        'A folder or file couldn’t be read: no permission, or the disk isn’t connected.',
      changed:
        'The folder was replaced by a different one since it was approved, so it wasn’t read.',
      format:
        'In a form PathKeep doesn’t read yet; saved as it is, to be read once it does.',
      writing: 'Being written while the backup ran; read next time.',
      damaged:
        'Part of it is damaged or cut off; saved as it is, and the rest was read.',
      skipped:
        'Skipped: a link, or something that isn’t an ordinary file inside the folder.',
      destination: 'The disk the archive is on wasn’t available.',
      omitted_one: '{count} more note like these isn’t listed.',
      omitted_other: '{count} more notes like these aren’t listed.',
      other: 'A note from the backup.',
      tool: '{tool}: {text}',
      writeFailed:
        'Writing to the archive failed. What was saved before it stays.',
      layout:
        'Its folder is laid out in a way PathKeep doesn’t recognise, so it wasn’t read.',
      tooDeep: 'Folders nested too deep weren’t read.',
      badName: 'A file name PathKeep can’t handle; that file wasn’t read.',
      git: 'The project’s .git couldn’t be read.',
      attachment: 'An attachment it points to couldn’t be read.',
      unsaved: 'Something PathKeep was asked to save wasn’t saved.',
      outputLimit:
        'Some tool outputs it points to were past the size PathKeep keeps, so they weren’t saved.',
      attachmentMissing:
        'A file it points to was gone by the time of the backup, so it wasn’t saved.',
      attachmentUnreadable:
        'A file it points to couldn’t be read (no permission, or a read error), so it wasn’t saved.',
    },
    notes: {
      empty: 'An empty file: it holds no conversation yet.',
      damaged:
        'Part of it couldn’t be read, such as a damaged or cut-off line; the rest was read.',
      details:
        'Its session details are missing or damaged; what could be read was read.',
      format:
        'In a form PathKeep can’t read yet; it will be readable after an update.',
      formatVersion:
        'In format version {version}, which PathKeep can’t read yet; it will be readable after an update.',
      unknownRecords:
        'Some of its records are of a kind PathKeep doesn’t know yet.',
      compressed: 'A compressed file PathKeep couldn’t unpack.',
      writing:
        'It was being written while the backup read it; the rest is read next time.',
      skipped:
        'A link or a special file, which PathKeep doesn’t copy; the conversations themselves are saved.',
      attachment:
        'An attachment it points to wasn’t copied (outside the folder, on the network, or past the size limit).',
      lineage:
        'Which conversation it continues isn’t clear yet; it will be put in place once it is.',
      other: 'Saved with a note from the backup.',
      isNew: 'New',
    },
    formatNotice:
      'PathKeep can’t read {tool} format {version} yet. Its files are saved as they are and will be readable after an update.',
    states: {
      queued: 'Starting',
      running: 'Running',
      cancelling: 'Stopping',
      succeeded: 'Done',
      partial: 'Didn’t save everything',
      interrupted: 'Stopped before the end',
      failed: 'Failed',
    },
    reasons: {
      cancelled: 'You cancelled it.',
      ownerRestarted: 'PathKeep closed while it ran.',
      readerRebuild:
        'The conversations’ files were saved, but the lists of some of them couldn’t be updated. Repair conversations rebuilds them.',
      terminalPublication:
        'The disk wasn’t available when it came to record the end, so it was recorded later. What it saved is kept.',
      other: 'It ended with “{code}”.',
    },
    actions: {
      cancel: 'Cancel',
      cancelLabel: 'Cancel “{name}”',
      retry: 'Try again',
      retryLabel: 'Try “{name}” again',
      dismiss: 'Dismiss',
      dismissLabel: 'Dismiss “{name}” from Needs a look',
      dismissNoticeLabel: 'Dismiss the notice about {tool} format {version}',
    },
    control: {
      failed: 'Couldn’t do that',
      changed:
        'It changed meanwhile; what you see now is how it is. Try again if it still applies.',
      finishing: 'It’s finishing up and can’t be stopped now.',
      busy: 'Another backup is running. Try again when it has finished.',
      previewRequired:
        'This backup can’t run again as it was: its folders or choices changed since. Back up now reads what is chosen today.',
      openBackupPage: 'Open the backup page',
    },
    group: {
      title: 'Conversation backups',
      note: 'Backups of Agent conversations: what runs now, what needs a look, and what finished today.',
      attention: 'Needs a look',
      running: 'Running',
      today: 'Finished today',
      noneRunning: 'Nothing is running.',
      noneToday: 'Nothing has finished today.',
      more: 'Show more',
      unavailable:
        'Saved Agent conversations can’t be opened right now, so these backups can’t be listed. Browsing history is not affected.',
    },
    rail: {
      needsLook_one: '{count} conversation backup needs a look',
      needsLook_other: '{count} conversation backups need a look',
    },
    finished: {
      done: 'Agent conversations backed up',
      partial: 'Agent conversations backed up, but not everything was saved',
      failed: 'Backing up Agent conversations failed',
      interrupted: 'Backing up Agent conversations stopped before the end',
      interruptedBody: 'What it saved stays. Back up again to save the rest.',
      notes_one:
        'Everything was saved, with {count} new note about reading it.',
      notes_other:
        'Everything was saved, with {count} new notes about reading it.',
      nothingNew: 'Nothing new.',
      newConversations_one: '{count} new conversation',
      newConversations_other: '{count} new conversations',
      updated_one: '{count} with new content',
      updated_other: '{count} with new content',
      newSubagents_one: '{count} new subagent',
      newSubagents_other: '{count} new subagents',
      list: '{items}.',
      // Between two sentences of one notice.
      sentenceJoin: ' ',
      details: 'Details',
    },
  },
  'zh-CN': {
    names: {
      backup: '备份 Agent 对话',
      firstSave: '第一次保存 Agent 对话',
    },
    progress: {
      waiting: '等着开始',
      looking: '正在找文件',
      found_one: '已经找到 {count} 个文件',
      found_other: '已经找到 {count} 个文件',
      files_one: '{count} 个文件',
      files_other: '{count} 个文件',
      saved: '已保存 {files}，{size}',
      conversations_one: '{count} 段对话',
      conversations_other: '{count} 段对话',
      subagents_one: '{count} 个 subagent',
      subagents_other: '{count} 个 subagent',
      indexing: '正在让存下的内容可以搜索',
      finishing: '正在收尾',
      stopping: '正在停下',
      join: ' · ',
    },
    availability: {
      available: '全部读到了。',
      disabled: '关着：不备份。',
      stale: '还没读。',
      unobserved: '这次备份没有读它。',
      partial: '它的文件夹有的读到了，有的没有。',
      rootMissing: '它的文件夹不在。',
      rootUnreadable:
        'PathKeep 读不到它的文件夹：没有权限，或者磁盘没有接上。读不到不代表文件不见了。',
      rootChanged:
        '它的文件夹在同意之后被换成了另一个，所以 PathKeep 没有读。备份之前请再看一次。',
      unknownFormat:
        '有些文件的格式 PathKeep 还读不懂。它们照原样存下了，等读得懂之后再整理进对话。',
      changing: '有些文件在备份时正在写入，下次再读。',
      unreadable: '有些文件读不了，读得到的都存下了。',
      destination: '存档所在的磁盘当时用不了。',
      other: '结束时是「{code}」。',
    },
    warnings: {
      missing: '文件夹不在。',
      unreadable: '有个文件夹或文件读不到：没有权限，或者磁盘没有接上。',
      changed: '文件夹在同意之后被换成了另一个，所以没有读。',
      format: '格式 PathKeep 还读不懂；照原样存下了，等读得懂再读。',
      writing: '备份时正在写入；下次再读。',
      damaged: '有一部分坏了或者被截断；照原样存下了，其余的都读了。',
      skipped: '跳过了：文件夹里的链接，或者不是一般文件的东西。',
      destination: '存档所在的磁盘当时用不了。',
      omitted_one: '还有 {count} 条类似的说明没有列出来。',
      omitted_other: '还有 {count} 条类似的说明没有列出来。',
      other: '备份留下的说明。',
      tool: '{tool}：{text}',
      writeFailed: '写入存档失败。之前存好的都还在。',
      layout: '它的文件夹结构 PathKeep 认不出来，所以没有读。',
      tooDeep: '嵌套太深的文件夹没有读。',
      badName: '有个文件名 PathKeep 处理不了，那个文件没有读。',
      git: '项目的 .git 读不到。',
      attachment: '它指向的附件读不到。',
      unsaved: '有要保存的东西没存下来。',
      outputLimit:
        '它指向的一些工具输出超过 PathKeep 保留的大小，所以没有存下来。',
      attachmentMissing: '它指向的文件在备份时已经不在了，所以没有存下来。',
      attachmentUnreadable:
        '它指向的文件读不了（没有权限或读取出错），所以没有存下来。',
    },
    notes: {
      empty: '空文件：里面还没有对话。',
      damaged: '有一部分读不了，例如坏掉或被截断的一行；其余的都读了。',
      details: '会话信息缺了或坏了；读得到的都读了。',
      format: '格式 PathKeep 还读不懂；更新之后就能读。',
      formatVersion:
        '用的是格式版本 {version}，PathKeep 还读不懂；更新之后就能读。',
      unknownRecords: '有些记录的类型 PathKeep 还不认识。',
      compressed: '压缩文件，PathKeep 解不开。',
      writing: '备份读它的时候它正在写入；剩下的下次再读。',
      skipped: '链接或特殊文件，PathKeep 不复制这些；对话本身都存下了。',
      attachment:
        '它指向的附件没有复制（在文件夹外面、在网络上，或者超过大小上限）。',
      lineage: '还不清楚它接着哪一段对话；弄清楚后会放到对的位置。',
      other: '存下了，备份留下一条备注。',
      isNew: '新',
    },
    formatNotice:
      'PathKeep 还读不懂 {tool} 的格式 {version}。它的文件照原样存下了，更新 PathKeep 之后就能读。',
    states: {
      queued: '正在开始',
      running: '正在跑',
      cancelling: '正在停下',
      succeeded: '完成',
      partial: '没有全部存下',
      interrupted: '中途停下了',
      failed: '失败',
    },
    reasons: {
      cancelled: '你取消了它。',
      ownerRestarted: '它跑的时候 PathKeep 关掉了。',
      readerRebuild:
        '对话的文件都存下了，但其中一些对话的列表没能更新。「修复对话」会重建它们。',
      terminalPublication:
        '记录结束时磁盘用不了，所以晚一点才记下。它存下的都还在。',
      other: '结束时是「{code}」。',
    },
    actions: {
      cancel: '取消',
      cancelLabel: '取消「{name}」',
      retry: '再试一次',
      retryLabel: '再试一次「{name}」',
      dismiss: '收起',
      dismissLabel: '从「需要你看一下」收起「{name}」',
      dismissNoticeLabel: '收起 {tool} 格式 {version} 的提醒',
    },
    control: {
      failed: '没能做到',
      changed: '它刚刚变了；现在看到的就是它的状态。如果还需要，再试一次。',
      finishing: '它正在收尾，现在不能停。',
      busy: '另一个备份正在跑。等它做完再试一次。',
      previewRequired:
        '这次备份没办法照原样再跑：它的文件夹或选择后来变了。「立即备份」会照现在的选择读。',
      openBackupPage: '打开备份页',
    },
    group: {
      title: '对话的备份',
      note: 'Agent 对话的备份：正在跑的、需要你看一下的，和今天完成的。',
      attention: '需要你看一下',
      running: '正在跑',
      today: '今天完成',
      noneRunning: '没有在跑的。',
      noneToday: '今天还没有完成的。',
      more: '显示更多',
      unavailable:
        '已存的 Agent 对话现在打不开，所以列不出这些备份。浏览记录不受影响。',
    },
    rail: {
      needsLook_one: '{count} 个对话的备份需要你看一下',
      needsLook_other: '{count} 个对话的备份需要你看一下',
    },
    finished: {
      done: 'Agent 对话备份好了',
      partial: 'Agent 对话备份好了，但没有全部存下',
      failed: 'Agent 对话备份失败',
      interrupted: 'Agent 对话的备份中途停下了',
      interruptedBody: '已经存下的留着。再备份一次就会存完剩下的。',
      notes_one: '全部都存下了，读取时留下 {count} 条新备注。',
      notes_other: '全部都存下了，读取时留下 {count} 条新备注。',
      nothingNew: '没有新增。',
      newConversations_one: '新增 {count} 段对话',
      newConversations_other: '新增 {count} 段对话',
      updated_one: '{count} 段有新内容',
      updated_other: '{count} 段有新内容',
      newSubagents_one: '新增 {count} 个 subagent',
      newSubagents_other: '新增 {count} 个 subagent',
      list: '{items}。',
      sentenceJoin: '',
      details: '详情',
    },
  },
  'zh-TW': {
    names: {
      backup: '備份 Agent 對話',
      firstSave: '第一次保存 Agent 對話',
    },
    progress: {
      waiting: '等著開始',
      looking: '正在找檔案',
      found_one: '已經找到 {count} 個檔案',
      found_other: '已經找到 {count} 個檔案',
      files_one: '{count} 個檔案',
      files_other: '{count} 個檔案',
      saved: '已保存 {files}，{size}',
      conversations_one: '{count} 段對話',
      conversations_other: '{count} 段對話',
      subagents_one: '{count} 個 subagent',
      subagents_other: '{count} 個 subagent',
      indexing: '正在讓存下的內容可以搜尋',
      finishing: '正在收尾',
      stopping: '正在停下',
      join: ' · ',
    },
    availability: {
      available: '全部讀到了。',
      disabled: '關著：不備份。',
      stale: '還沒讀。',
      unobserved: '這次備份沒有讀它。',
      partial: '它的資料夾有的讀到了，有的沒有。',
      rootMissing: '它的資料夾不在。',
      rootUnreadable:
        'PathKeep 讀不到它的資料夾：沒有權限，或者磁碟沒有接上。讀不到不代表檔案不見了。',
      rootChanged:
        '它的資料夾在同意之後被換成了另一個，所以 PathKeep 沒有讀。備份之前請再看一次。',
      unknownFormat:
        '有些檔案的格式 PathKeep 還讀不懂。它們照原樣存下了，等讀得懂之後再整理進對話。',
      changing: '有些檔案在備份時正在寫入，下次再讀。',
      unreadable: '有些檔案讀不了，讀得到的都存下了。',
      destination: '存檔所在的磁碟當時用不了。',
      other: '結束時是「{code}」。',
    },
    warnings: {
      missing: '資料夾不在。',
      unreadable: '有個資料夾或檔案讀不到：沒有權限，或者磁碟沒有接上。',
      changed: '資料夾在同意之後被換成了另一個，所以沒有讀。',
      format: '格式 PathKeep 還讀不懂；照原樣存下了，等讀得懂再讀。',
      writing: '備份時正在寫入；下次再讀。',
      damaged: '有一部分壞了或者被截斷；照原樣存下了，其餘的都讀了。',
      skipped: '跳過了：資料夾裡的連結，或者不是一般檔案的東西。',
      destination: '存檔所在的磁碟當時用不了。',
      omitted_one: '還有 {count} 則類似的說明沒有列出來。',
      omitted_other: '還有 {count} 則類似的說明沒有列出來。',
      other: '備份留下的說明。',
      tool: '{tool}：{text}',
      writeFailed: '寫入存檔失敗。之前存好的都還在。',
      layout: '它的資料夾結構 PathKeep 認不出來，所以沒有讀。',
      tooDeep: '巢狀太深的資料夾沒有讀。',
      badName: '有個檔名 PathKeep 處理不了，那個檔案沒有讀。',
      git: '專案的 .git 讀不到。',
      attachment: '它指向的附件讀不到。',
      unsaved: '有要保存的東西沒存下來。',
      outputLimit:
        '它指向的一些工具輸出超過 PathKeep 保留的大小，所以沒有存下來。',
      attachmentMissing: '它指向的檔案在備份時已經不在了，所以沒有存下來。',
      attachmentUnreadable:
        '它指向的檔案讀不了（沒有權限或讀取出錯），所以沒有存下來。',
    },
    notes: {
      empty: '空檔案：裡面還沒有對話。',
      damaged: '有一部分讀不了，例如壞掉或被截斷的一行；其餘的都讀了。',
      details: '工作階段的資訊缺了或壞了；讀得到的都讀了。',
      format: '格式 PathKeep 還讀不懂；更新之後就能讀。',
      formatVersion:
        '用的是格式版本 {version}，PathKeep 還讀不懂；更新之後就能讀。',
      unknownRecords: '有些紀錄的類型 PathKeep 還不認識。',
      compressed: '壓縮檔，PathKeep 解不開。',
      writing: '備份讀它的時候它正在寫入；剩下的下次再讀。',
      skipped: '連結或特殊檔案，PathKeep 不複製這些；對話本身都存下了。',
      attachment:
        '它指向的附件沒有複製（在資料夾外面、在網路上，或超過大小上限）。',
      lineage: '還不清楚它接著哪一段對話；弄清楚後會放到對的位置。',
      other: '存下了，備份留下一則備註。',
      isNew: '新',
    },
    formatNotice:
      'PathKeep 還讀不懂 {tool} 的格式 {version}。它的檔案照原樣存下了，更新 PathKeep 之後就能讀。',
    states: {
      queued: '正在開始',
      running: '正在跑',
      cancelling: '正在停下',
      succeeded: '完成',
      partial: '沒有全部存下',
      interrupted: '中途停下了',
      failed: '失敗',
    },
    reasons: {
      cancelled: '你取消了它。',
      ownerRestarted: '它跑的時候 PathKeep 關掉了。',
      readerRebuild:
        '對話的檔案都存下了，但其中一些對話的清單沒能更新。「修復對話」會重建它們。',
      terminalPublication:
        '記錄結束時磁碟用不了，所以晚一點才記下。它存下的都還在。',
      other: '結束時是「{code}」。',
    },
    actions: {
      cancel: '取消',
      cancelLabel: '取消「{name}」',
      retry: '再試一次',
      retryLabel: '再試一次「{name}」',
      dismiss: '收起',
      dismissLabel: '從「需要你看一下」收起「{name}」',
      dismissNoticeLabel: '收起 {tool} 格式 {version} 的提醒',
    },
    control: {
      failed: '沒能做到',
      changed: '它剛剛變了；現在看到的就是它的狀態。如果還需要，再試一次。',
      finishing: '它正在收尾，現在不能停。',
      busy: '另一個備份正在跑。等它做完再試一次。',
      previewRequired:
        '這次備份沒辦法照原樣再跑：它的資料夾或選擇後來變了。「立即備份」會照現在的選擇讀。',
      openBackupPage: '打開備份頁',
    },
    group: {
      title: '對話的備份',
      note: 'Agent 對話的備份：正在跑的、需要你看一下的，和今天完成的。',
      attention: '需要你看一下',
      running: '正在跑',
      today: '今天完成',
      noneRunning: '沒有在跑的。',
      noneToday: '今天還沒有完成的。',
      more: '顯示更多',
      unavailable:
        '已存的 Agent 對話現在打不開，所以列不出這些備份。瀏覽紀錄不受影響。',
    },
    rail: {
      needsLook_one: '{count} 個對話的備份需要你看一下',
      needsLook_other: '{count} 個對話的備份需要你看一下',
    },
    finished: {
      done: 'Agent 對話備份好了',
      partial: 'Agent 對話備份好了，但沒有全部存下',
      failed: 'Agent 對話備份失敗',
      interrupted: 'Agent 對話的備份中途停下了',
      interruptedBody: '已經存下的留著。再備份一次就會存完剩下的。',
      notes_one: '全部都存下了，讀取時留下 {count} 則新備註。',
      notes_other: '全部都存下了，讀取時留下 {count} 則新備註。',
      nothingNew: '沒有新增。',
      newConversations_one: '新增 {count} 段對話',
      newConversations_other: '新增 {count} 段對話',
      updated_one: '{count} 段有新內容',
      updated_other: '{count} 段有新內容',
      newSubagents_one: '新增 {count} 個 subagent',
      newSubagents_other: '新增 {count} 個 subagent',
      list: '{items}。',
      sentenceJoin: '',
      details: '詳情',
    },
  },
})
