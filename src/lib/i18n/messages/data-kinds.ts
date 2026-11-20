import { defineMessages } from '../define'

/**
 * Kinds of data (browsing, Agent conversations) as a whole: why each is
 * kept, the Agent tools' names, the page an address of a kind that is off
 * shows, the Agent intro card on browsing's Home, and Settings › Manage
 * data types.
 */
export const dataKinds = defineMessages({
  en: {
    why: {
      browsing:
        'Chrome keeps 90 days of history, and Safari keeps a year by default.',
      agent:
        'Claude Code deletes conversations after 30 days by default, and Codex history often gets cleaned out by hand.',
    },
    /** Product names; the same in every language. */
    tools: {
      claudeCode: 'Claude Code',
      codex: 'Codex',
      pi: 'Pi',
      deepseek: 'DeepSeek Harness',
    },
    folders: {
      claudeDesktop: 'Claude desktop app (local Cowork sessions)',
    },
    listSeparator: ', ',
    off: {
      title: '“{name}” is off',
      body: 'PathKeep isn’t keeping this kind of data now. What it saved before is still in the archive, and you’ll see it again once you turn it on.',
      manage: 'Manage data types',
      enable: 'Preview and turn on {name}',
    },
    intro: {
      title: 'PathKeep can now keep your Agent conversations too',
      body: 'Claude Code deletes conversations after 30 days by default, and Codex history often gets cleaned out too. Once this is on, they are saved in the same archive, where you can find them again by project, by date or by any word in them.',
      found: 'Folders of these tools are on this computer:',
      onlyChecked:
        'PathKeep only checked that the folders are there. Nothing in them has been read.',
      preview: 'Preview what would be saved',
      later: 'Not now',
      dismiss: 'Hide this card',
      dismissFailed: 'Couldn’t hide this card',
    },
    manage: {
      title: 'Manage data types',
      intro:
        'Each kind of data turns on and off on its own. Pausing or turning one off deletes nothing: what is saved stays in the archive and comes back when you turn it on again. Deleting saved data is a separate action.',
      stateLabel: 'State of {name}',
      states: {
        on: 'On',
        paused: 'Paused',
        off: 'Off',
      },
      hints: {
        on: 'Backed up on schedule, and shown in the navigation',
        paused:
          'No more backups; what is saved can still be viewed and searched',
        off: 'No more backups, and not shown in the navigation; saved data is kept',
      },
      holds: '{hint}. The archive holds {count}.',
      holdsSize: '{hint}. The archive holds {count}, {size}.',
      hintOnly: '{hint}.',
      count: {
        visits_one: '{count} visit',
        visits_other: '{count} visits',
        conversations_one: '{count} conversation',
        conversations_other: '{count} conversations',
      },
      sources: 'Sources: {list}',
      sourcesNone: 'Sources: none found on this computer',
      sourcesFailed: 'Sources: couldn’t check this computer',
      sourcesLoading: 'Looking for sources on this computer',
      lastOn:
        'At least one kind of data has to stay on, so “{name}” can’t be paused or turned off. Turn on another kind first.',
      turnedOff: '{name} is off. Everything it saved is still there.',
      lastOnRefused: 'At least one kind of data has to stay on.',
      changedElsewhere:
        'This was changed somewhere else meanwhile. What you see now is what is stored.',
      changeFailed: 'Couldn’t change “{name}”',
      notInitialized: 'The archive isn’t set up yet.',
      viewOnly:
        'This computer isn’t backing conversations up yet. The archive holds conversations saved on another computer, and you can read and search them.',
      setUp: 'Set up backup…',
      heldOn:
        '“{name}” has to stay on: “{other}” is on but has nothing to back up until its sources are chosen.',
      review: {
        browsing:
          'On, but no browser profile is chosen, so nothing is backed up. Choose which browsers to back up.',
        agent:
          'On, but nothing on this computer is chosen to back up, so nothing is backed up. Everything saved is still here.',
        chooseBrowsers: 'Choose browsers',
        chooseSources: 'Choose sources',
      },
      sourceSelectionEmpty:
        'That would leave PathKeep with nothing to back up. Choose another source, or turn on the other kind of data, first.',
      stoppedBackup_one:
        'The backup of {name} that was running stopped. What it saved stays in the archive.',
      stoppedBackup_other:
        '{count} backups of {name} that were running stopped. What they saved stays in the archive.',
    },
    damage: {
      title: 'Saved {name} can’t be opened right now',
      generic: 'A file that holds them is damaged or can’t be read.',
      parts: {
        evidence:
          'The file that holds the saved conversations is damaged or can’t be read.',
        readModel:
          'The file that holds their lists, search and insights is damaged or can’t be read.',
        shared:
          'A file they share with the rest of the archive is damaged or can’t be read.',
      },
      stages: {
        preflight: 'Found while checking that it was there.',
        open: 'Found while opening it.',
        keyCheck: 'Found while reading it.',
        migrate: 'Found while updating it for this version of PathKeep.',
        recover: 'Found while recovering it after an interruption.',
        read: 'Found while reading saved conversations.',
        write: 'Found while saving conversations.',
      },
      notAffected: 'Browsing history is not affected.',
      accessDenied:
        'PathKeep isn’t allowed to read their files right now. Nothing is damaged: once the files can be read again, for example after their permissions are fixed, choose Try again.',
      busy: 'Their files are in use right now. Nothing is damaged: try again in a moment.',
      foreign:
        'The file that holds the saved conversations belongs to another archive, for example because a sync tool or a backup app put a different file back here. PathKeep doesn’t read another archive’s conversations as this one’s.',
      diverged:
        'The conversations’ lists don’t match the saved conversations, for example because one of their files came back from an older backup. Rebuilding the lists from the saved conversations fixes this.',
      retry: 'Try again',
      retryFailed: 'Couldn’t try again',
    },
  },
  'zh-CN': {
    why: {
      browsing: 'Chrome 只保留 90 天，Safari 默认一年',
      agent: 'Claude Code 默认 30 天清掉旧对话，Codex 会被手动清理',
    },
    tools: {
      claudeCode: 'Claude Code',
      codex: 'Codex',
      pi: 'Pi',
      deepseek: 'DeepSeek Harness',
    },
    folders: {
      claudeDesktop: 'Claude 桌面应用（本机的 Cowork 会话）',
    },
    listSeparator: '、',
    off: {
      title: '「{name}」没有开启',
      body: 'PathKeep 目前不保存这类数据。以前存下的内容还在存档里，开启后就能再看到。',
      manage: '管理数据类型',
      enable: '预览并开启 {name}',
    },
    intro: {
      title: 'PathKeep 现在也能保存 Agent 对话',
      body: 'Claude Code 默认 30 天后删掉旧对话，Codex 的记录也常被清掉。开启后，它们会存进同一个存档，按项目、时间和全文都找得回来。',
      found: '这台电脑上有这些工具的文件夹：',
      onlyChecked: '只看了文件夹在不在，还没有读任何内容。',
      preview: '预览会保存什么',
      later: '以后再说',
      dismiss: '关闭这张卡片',
      dismissFailed: '没能关闭这张卡片',
    },
    manage: {
      title: '管理数据类型',
      intro:
        '每一种数据各自开关。暂停和关闭都不会删除任何东西：已存的留在存档里，重新开启就回来。要删除已存的数据，是另外的操作。',
      stateLabel: '{name}的状态',
      states: {
        on: '开启',
        paused: '暂停',
        off: '关闭',
      },
      hints: {
        on: '定时备份，导航栏里看得到',
        paused: '不再备份，已存的照样能看、能搜',
        off: '不再备份，也不在导航栏显示；已存的数据保留',
      },
      holds: '{hint}。存档里有 {count}。',
      holdsSize: '{hint}。存档里有 {count}，{size}。',
      hintOnly: '{hint}。',
      count: {
        visits_one: '{count} 次浏览',
        visits_other: '{count} 次浏览',
        conversations_one: '{count} 段对话',
        conversations_other: '{count} 段对话',
      },
      sources: '来源：{list}',
      sourcesNone: '来源：这台电脑上没有找到',
      sourcesFailed: '来源：无法检查这台电脑',
      sourcesLoading: '正在这台电脑上找来源',
      lastOn:
        '至少要开着一种数据类型，所以「{name}」不能暂停或关闭。先开启另一种。',
      turnedOff: '{name}已关闭，已存的数据都还在',
      lastOnRefused: '至少要开着一种数据类型',
      changedElsewhere:
        '这段时间它在别处被改过了，现在显示的是保存下来的状态。',
      changeFailed: '无法更改「{name}」',
      notInitialized: '存档还没建立。',
      viewOnly:
        '这台电脑还没在备份对话。存档里有另一台电脑存下的对话，可以照常阅读和搜索。',
      setUp: '设置备份…',
      heldOn:
        '「{other}」开着，但还没有可以备份的来源，所以在选好它的来源之前，「{name}」得一直开着。',
      review: {
        browsing:
          '开着，但没有选任何浏览器配置文件，所以什么都没在备份。选一下要备份哪些浏览器。',
        agent:
          '开着，但这台电脑上没有选任何要备份的来源，所以什么都没在备份。已经存下的都还在。',
        chooseBrowsers: '选择浏览器',
        chooseSources: '选择来源',
      },
      sourceSelectionEmpty:
        '这样 PathKeep 就没有东西可以备份了。先选另一个来源，或者先打开另一种数据。',
      stoppedBackup_one: '正在进行的{name}备份已停下，已存的都还在存档里。',
      stoppedBackup_other:
        '{count} 个正在进行的{name}备份已停下，已存的都还在存档里。',
    },
    damage: {
      title: '已存的{name}现在打不开',
      generic: '存放它们的文件损坏了，或者读不了。',
      parts: {
        evidence: '存放已存对话的文件损坏了，或者读不了。',
        readModel: '存放对话列表、搜索和洞察的文件损坏了，或者读不了。',
        shared: '它们和存档其他部分共用的文件损坏了，或者读不了。',
      },
      stages: {
        preflight: '在确认它存在时发现的。',
        open: '在打开它时发现的。',
        keyCheck: '在读取它时发现的。',
        migrate: '在把它更新到这一版 PathKeep 时发现的。',
        recover: '在中断后恢复它时发现的。',
        read: '在读取已存下的对话时发现的。',
        write: '在保存对话时发现的。',
      },
      notAffected: '浏览记录不受影响。',
      accessDenied:
        'PathKeep 现在没有读取它们文件的权限。文件没有坏：等能读取了（例如修好权限之后），点「再试一次」。',
      busy: '它们的文件现在正被占用。文件没有坏：过一会儿再试一次。',
      foreign:
        '存放已存对话的文件属于另一个存档，例如同步工具或备份软件把别的文件放回了这里。PathKeep 不会把另一个存档的对话当成这个存档的来读。',
      diverged:
        '对话列表和已存的对话对不上，例如其中一个文件是从较旧的备份放回来的。用已存的对话重建列表就能解决。',
      retry: '再试一次',
      retryFailed: '没能再试一次',
    },
  },
  'zh-TW': {
    why: {
      browsing: 'Chrome 只留 90 天，Safari 預設一年',
      agent: 'Claude Code 預設 30 天清掉舊對話，Codex 會被手動清理',
    },
    tools: {
      claudeCode: 'Claude Code',
      codex: 'Codex',
      pi: 'Pi',
      deepseek: 'DeepSeek Harness',
    },
    folders: {
      claudeDesktop: 'Claude 桌面應用程式（本機的 Cowork 工作階段）',
    },
    listSeparator: '、',
    off: {
      title: '「{name}」沒有開啟',
      body: 'PathKeep 目前不保存這類資料。以前存下的內容還在存檔裡，開啟後就能再看到。',
      manage: '管理資料類型',
      enable: '預覽並開啟 {name}',
    },
    intro: {
      title: 'PathKeep 現在也能保存 Agent 對話',
      body: 'Claude Code 預設 30 天後刪掉舊對話，Codex 的紀錄也常被清掉。開啟後，它們會存進同一個存檔，照專案、時間和全文都找得回來。',
      found: '這台電腦上有這些工具的資料夾：',
      onlyChecked: '只看了資料夾在不在，還沒有讀任何內容。',
      preview: '預覽會保存什麼',
      later: '以後再說',
      dismiss: '關閉這張卡片',
      dismissFailed: '沒能關閉這張卡片',
    },
    manage: {
      title: '管理資料類型',
      intro:
        '每一種資料各自開關。暫停和關閉都不會刪任何東西：已存的留在存檔裡，重新開啟就回來。要刪掉已存的資料，是另外的動作。',
      stateLabel: '{name}的狀態',
      states: {
        on: '開啟',
        paused: '暫停',
        off: '關閉',
      },
      hints: {
        on: '定時備份，導航列裡看得到',
        paused: '不再備份，已存的照樣能看、能搜',
        off: '不再備份，也不在導航列顯示；已存的資料保留',
      },
      holds: '{hint}。存檔裡有 {count}。',
      holdsSize: '{hint}。存檔裡有 {count}，{size}。',
      hintOnly: '{hint}。',
      count: {
        visits_one: '{count} 次瀏覽',
        visits_other: '{count} 次瀏覽',
        conversations_one: '{count} 段對話',
        conversations_other: '{count} 段對話',
      },
      sources: '來源：{list}',
      sourcesNone: '來源：這台電腦上沒有找到',
      sourcesFailed: '來源：無法檢查這台電腦',
      sourcesLoading: '正在這台電腦上找來源',
      lastOn:
        '至少要開著一種資料類型，所以「{name}」不能暫停或關閉。先開啟另一種。',
      turnedOff: '{name}已關閉，已存的資料都還在',
      lastOnRefused: '至少要開著一種資料類型',
      changedElsewhere: '這段時間它在別處被改過了，現在顯示的是存下來的狀態。',
      changeFailed: '無法變更「{name}」',
      notInitialized: '存檔還沒建立。',
      viewOnly:
        '這台電腦還沒在備份對話。存檔裡有另一台電腦存下的對話，可以照常閱讀和搜尋。',
      setUp: '設定備份…',
      heldOn:
        '「{other}」開著，但還沒有可以備份的來源，所以在選好它的來源之前，「{name}」得一直開著。',
      review: {
        browsing:
          '開著，但沒有選任何瀏覽器設定檔，所以什麼都沒在備份。選一下要備份哪些瀏覽器。',
        agent:
          '開著，但這台電腦上沒有選任何要備份的來源，所以什麼都沒在備份。已經存下的都還在。',
        chooseBrowsers: '選擇瀏覽器',
        chooseSources: '選擇來源',
      },
      sourceSelectionEmpty:
        '這樣 PathKeep 就沒有東西可以備份了。先選另一個來源，或先打開另一種資料。',
      stoppedBackup_one: '正在進行的{name}備份已停下，已存的都還在存檔裡。',
      stoppedBackup_other:
        '{count} 個正在進行的{name}備份已停下，已存的都還在存檔裡。',
    },
    damage: {
      title: '已存的{name}現在打不開',
      generic: '存放它們的檔案損壞了，或者讀不了。',
      parts: {
        evidence: '存放已存對話的檔案損壞了，或者讀不了。',
        readModel: '存放對話清單、搜尋和洞察的檔案損壞了，或者讀不了。',
        shared: '它們和存檔其他部分共用的檔案損壞了，或者讀不了。',
      },
      stages: {
        preflight: '在確認它存在時發現的。',
        open: '在打開它時發現的。',
        keyCheck: '在讀取它時發現的。',
        migrate: '在把它更新到這一版 PathKeep 時發現的。',
        recover: '在中斷後復原它時發現的。',
        read: '在讀取已存下的對話時發現的。',
        write: '在保存對話時發現的。',
      },
      notAffected: '瀏覽紀錄不受影響。',
      accessDenied:
        'PathKeep 現在沒有讀取它們檔案的權限。檔案沒有壞：等能讀取了（例如修好權限之後），按「再試一次」。',
      busy: '它們的檔案現在正被佔用。檔案沒有壞：過一會兒再試一次。',
      foreign:
        '存放已存對話的檔案屬於另一個存檔，例如同步工具或備份軟體把別的檔案放回了這裡。PathKeep 不會把另一個存檔的對話當成這個存檔的來讀。',
      diverged:
        '對話清單和已存的對話對不上，例如其中一個檔案是從較舊的備份放回來的。用已存的對話重建清單就能解決。',
      retry: '再試一次',
      retryFailed: '沒能再試一次',
    },
  },
})
