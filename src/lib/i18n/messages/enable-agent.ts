import { defineMessages } from '../define'

/**
 * Turning Agent conversations on (`?enable=agent`): the steps, the preview
 * of what would be read (its table, notes, numbers and every state of the
 * walk), and choosing the tools.
 */
export const enableAgent = defineMessages({
  en: {
    steps: {
      label: 'Steps',
      preview: 'See what it reads',
      choose: 'Choose',
      save: 'Save',
      done: '{step} (done)',
    },
    preview: {
      title: 'Keep Agent conversations: first, see what PathKeep would read',
      body: 'Claude Code deletes conversations after 30 days by default, and Codex history often gets cleaned out too. Once this is on, PathKeep saves them into your archive, where you can find them again by project, by date or by any word in them.',
      tableLabel: 'Tool folders found on this computer',
      columns: {
        tool: 'Tool',
        folder: 'Folder',
        files: 'Conversation files',
        size: 'Size',
        earliest: 'Earliest',
      },
      subagents_one: 'plus {count} subagent file',
      subagents_other: 'plus {count} subagent files',
      noCount: 'Can’t be counted from the folder’s layout',
      counting: 'Counting files…',
      checked_one: '{count} file or folder looked at so far',
      checked_other: '{count} files and folders looked at so far',
      nothingOpened:
        'So far PathKeep has only counted files and looked at their sizes and dates. It hasn’t opened a single conversation.',
      reads: {
        title: 'What it reads',
        transcripts: 'Conversation transcripts, subagent transcripts included',
        outputs: 'Tool output and pasted attachments',
        git: 'The .git of each project folder: the repo’s root folder, which repo a worktree belongs to, the current branch and the remote URL',
      },
      never: {
        title: 'What it never does',
        write:
          'Never writes to, moves or deletes anything in the tools’ own folders',
        run: 'Never starts a tool or reads the code in your projects',
        send: 'Never sends anything anywhere',
      },
      grows: 'The archive grows by',
      growsValue: 'about {size}',
      estimate: 'estimate',
      free: 'Free on this disk',
      encryption: 'Encryption',
      encrypted: 'Same as the archive now: encrypted',
      plaintext: 'Same as the archive now: not encrypted',
      cancelled: 'The preview was stopped. Nothing was saved.',
      runAgain: 'Run the preview again',
      failed: 'The preview didn’t finish',
      expired:
        'This preview is more than five minutes old, and the folders may have changed since. Run it again before saving.',
      nothingFound: {
        title: 'No Agent tool folders on this computer',
        body: 'PathKeep looks for the folders of Claude Code, Codex, Pi and DeepSeek Harness. Once one of them is installed and has been used, check again.',
        lookedIn: 'Looked in:',
        checkAgain: 'Check again',
      },
      rows: {
        unreadable:
          'PathKeep can’t read this folder: it has no permission, or the disk isn’t connected.',
        scopeDenied: 'PathKeep doesn’t read folders inside a .git folder.',
        partlyUnreadable:
          'Some folders inside couldn’t be read; the numbers cover the rest.',
        tooDeep:
          'Folders nested very deep were skipped; the numbers cover the rest.',
        unknownFormat:
          'Some files here are in a form PathKeep doesn’t recognise yet; the numbers leave them out.',
        changed:
          'This folder changed while it was being counted. Run the preview again.',
      },
      reasons: {
        busy: 'Another task is using the archive. Try again when it has finished.',
        destination: 'The disk the archive is on isn’t available.',
        unavailable:
          'The file that holds saved Agent conversations is damaged or can’t be read. Browsing history is not affected.',
        replaced: 'The archive was replaced while the preview ran.',
        policyChanged:
          'These choices were changed somewhere else while the preview ran.',
        connection: 'PathKeep lost the connection to the preview.',
        rootChanged: 'A folder changed while it was being counted.',
        uninitialized: 'The archive isn’t set up yet.',
        other: 'It stopped before it was done. The details are below.',
      },
    },
    choose: {
      title: 'Which tools’ conversations should PathKeep keep?',
      body: 'Tools you leave unticked are not read.',
      toolsLabel: 'Tools',
      toolSummary_one: '{count} conversation file · {size}',
      toolSummary_other: '{count} conversation files · {size}',
      toolUnreadable: 'Can’t read its folder',
      git: {
        title: 'Note each project’s git details',
        body: 'Once a worktree is deleted, nobody can tell which project it belonged to, so PathKeep notes it at backup time. It reads only these from .git: the repo’s root folder, which repo a worktree belongs to, the current branch and the remote URL. It never reads code.',
      },
      schedule:
        'Backups run on the schedule browsing history uses: {schedule}.',
      scheduleEvery: 'every {interval}',
      scheduleOff: 'automatic backup is off',
      switcher:
        'Once it is on, the switcher at the top left gets “{name}” ({keys} switches to it too), with its insights, records, projects and its own backup page.',
      switcherNoKeys:
        'Once it is on, the switcher at the top left gets “{name}”, with its insights, records, projects and its own backup page.',
      later:
        'You can change these choices later on the backup page of {name}, or turn the whole kind off in Settings › Manage data types. Turning it off doesn’t delete conversations already saved.',
      saveFailed: 'Couldn’t save your choice',
      needOne: 'Tick at least one tool.',
      notConnected:
        'This development build can’t start saving yet: the step that saves conversations isn’t connected.',
    },
    actions: {
      noThanks: 'No thanks',
      next: 'Next',
      back: 'Back',
      start: 'Start saving',
    },
  },
  'zh-CN': {
    steps: {
      label: '步骤',
      preview: '看会读什么',
      choose: '选择',
      save: '保存',
      done: '{step}（已完成）',
    },
    preview: {
      title: '保存 Agent 对话：先看看会读什么',
      body: 'Claude Code 默认 30 天后删掉旧对话，Codex 的记录也常被清掉。开启后，PathKeep 把它们存进你的存档，按项目、时间和全文都找得回来。',
      tableLabel: '这台电脑上找到的工具文件夹',
      columns: {
        tool: '工具',
        folder: '文件夹',
        files: '对话文件',
        size: '大小',
        earliest: '最早',
      },
      subagents_one: '另有 {count} 个 subagent 文件',
      subagents_other: '另有 {count} 个 subagent 文件',
      noCount: '从文件夹的结构数不出来',
      counting: '正在数文件…',
      checked_one: '已经看过 {count} 个文件和文件夹',
      checked_other: '已经看过 {count} 个文件和文件夹',
      nothingOpened:
        '到这里为止只数了文件的数量、大小和日期，还没有打开任何一段对话。',
      reads: {
        title: '会读取',
        transcripts: '对话记录文件，含 subagent 的记录',
        outputs: '工具输出和粘贴的附件',
        git: '项目文件夹的 .git：repo 的根目录、worktree 属于哪个 repo、当前分支、远程地址',
      },
      never: {
        title: '不会做',
        write: '不写入、不移动、不删除工具自己的文件夹',
        run: '不启动任何工具，不读项目里的代码',
        send: '不传到任何地方',
      },
      grows: '存档预计多出',
      growsValue: '约 {size}',
      estimate: '估计',
      free: '这块磁盘还有',
      encryption: '加密',
      encrypted: '跟现在的存档一样（已加密）',
      plaintext: '跟现在的存档一样（没有加密）',
      cancelled: '预览停下来了，什么都没有保存。',
      runAgain: '重新预览',
      failed: '预览没有完成',
      expired: '这份预览已经超过五分钟，文件夹可能变了。保存之前再预览一次。',
      nothingFound: {
        title: '这台电脑上没有找到 Agent 工具的文件夹',
        body: 'PathKeep 会寻找 Claude Code、Codex、Pi、DeepSeek Harness 的文件夹。装好并使用过之后，再检查一次。',
        lookedIn: '找过这些位置：',
        checkAgain: '重新检查',
      },
      rows: {
        unreadable: 'PathKeep 读不到这个文件夹：没有权限，或者磁盘没有接上。',
        scopeDenied: 'PathKeep 不读 .git 文件夹里的东西。',
        partlyUnreadable: '里面有些文件夹读不到，数字只算读得到的部分。',
        tooDeep: '太深的文件夹跳过了，数字只算其余的部分。',
        unknownFormat: '这里有些文件 PathKeep 还不认得，数字没有算进去。',
        changed: '这个文件夹在计数时变了，请再预览一次。',
      },
      reasons: {
        busy: '另一项工作正在使用存档。等它做完再试一次。',
        destination: '存档所在的磁盘现在用不了。',
        unavailable:
          '存放已存 Agent 对话的文件损坏了，或者读不了。浏览记录不受影响。',
        replaced: '预览时存档被换掉了。',
        policyChanged: '预览时这些设置在别处被改了。',
        connection: 'PathKeep 和预览的连接断了。',
        rootChanged: '计数时有个文件夹变了。',
        uninitialized: '存档还没建立。',
        other: '没做完就停了，详情在下面。',
      },
    },
    choose: {
      title: '要保存哪些工具的对话？',
      body: '没勾的工具不会被读取。',
      toolsLabel: '工具',
      toolSummary_one: '对话文件 {count} 个 · {size}',
      toolSummary_other: '对话文件 {count} 个 · {size}',
      toolUnreadable: '读不到它的文件夹',
      git: {
        title: '记下项目的 git 信息',
        body: 'worktree 删掉以后就查不到它属于哪个项目，所以要在备份当下记下来。只读 .git 里的 repo 的根目录、worktree 属于哪个 repo、当前分支、远程地址，不读代码。',
      },
      schedule: '排程跟浏览记录共用一个：{schedule}。',
      scheduleEvery: '每 {interval}',
      scheduleOff: '自动备份已关闭',
      switcher:
        '开启后，左上角的切换菜单会多一个「{name}」（{keys} 也能切过去），切过去有洞察、记录、项目和它自己的备份页。',
      switcherNoKeys:
        '开启后，左上角的切换菜单会多一个「{name}」，切过去有洞察、记录、项目和它自己的备份页。',
      later:
        '之后可以在{name}的备份页改这些选择；要整个关掉，在「设置 › 管理数据类型」。关掉不会删掉已经存下的对话。',
      saveFailed: '无法保存你的选择',
      needOne: '至少勾一个工具。',
      notConnected: '这个开发版还不能开始保存：保存对话的那一步还没接上。',
    },
    actions: {
      noThanks: '不用了',
      next: '下一步',
      back: '上一步',
      start: '开始保存',
    },
  },
  'zh-TW': {
    steps: {
      label: '步驟',
      preview: '看會讀什麼',
      choose: '選擇',
      save: '保存',
      done: '{step}（已完成）',
    },
    preview: {
      title: '保存 Agent 對話：先看看會讀什麼',
      body: 'Claude Code 預設 30 天後刪掉舊對話，Codex 的紀錄也常被清掉。開啟後，PathKeep 把它們存進你的存檔，照專案、時間和全文都找得回來。',
      tableLabel: '這台電腦上找到的工具資料夾',
      columns: {
        tool: '工具',
        folder: '資料夾',
        files: '對話檔',
        size: '大小',
        earliest: '最早',
      },
      subagents_one: '另有 {count} 個 subagent 檔',
      subagents_other: '另有 {count} 個 subagent 檔',
      noCount: '從資料夾的結構數不出來',
      counting: '正在數檔案…',
      checked_one: '已經看過 {count} 個檔案和資料夾',
      checked_other: '已經看過 {count} 個檔案和資料夾',
      nothingOpened:
        '到這裡為止只數了檔案的數量、大小和日期，還沒有打開任何一段對話。',
      reads: {
        title: '會讀取',
        transcripts: '對話紀錄檔，含 subagent 的紀錄',
        outputs: '工具輸出和貼上的附件',
        git: '專案資料夾的 .git：repo 的根目錄、worktree 屬於哪個 repo、目前分支、遠端網址',
      },
      never: {
        title: '不會做',
        write: '不寫入、不搬動、不刪除工具自己的資料夾',
        run: '不啟動任何工具，不讀專案裡的程式碼',
        send: '不傳到任何地方',
      },
      grows: '存檔預計多出',
      growsValue: '約 {size}',
      estimate: '估計',
      free: '這顆磁碟還有',
      encryption: '加密',
      encrypted: '跟現在的存檔一樣（已加密）',
      plaintext: '跟現在的存檔一樣（沒有加密）',
      cancelled: '預覽停下來了，什麼都沒有保存。',
      runAgain: '重新預覽',
      failed: '預覽沒有完成',
      expired: '這份預覽已經超過五分鐘，資料夾可能變了。保存之前再預覽一次。',
      nothingFound: {
        title: '這台電腦上沒有找到 Agent 工具的資料夾',
        body: 'PathKeep 會尋找 Claude Code、Codex、Pi、DeepSeek Harness 的資料夾。裝好並使用過之後，再檢查一次。',
        lookedIn: '找過這些位置：',
        checkAgain: '重新檢查',
      },
      rows: {
        unreadable: 'PathKeep 讀不到這個資料夾：沒有權限，或者磁碟沒有接上。',
        scopeDenied: 'PathKeep 不讀 .git 資料夾裡的東西。',
        partlyUnreadable: '裡面有些資料夾讀不到，數字只算讀得到的部分。',
        tooDeep: '太深的資料夾跳過了，數字只算其餘的部分。',
        unknownFormat: '這裡有些檔案 PathKeep 還不認得，數字沒有算進去。',
        changed: '這個資料夾在計數時變了，請再預覽一次。',
      },
      reasons: {
        busy: '另一項工作正在用存檔。等它做完再試一次。',
        destination: '存檔所在的磁碟現在用不了。',
        unavailable:
          '存放已存 Agent 對話的檔案損壞了，或者讀不了。瀏覽紀錄不受影響。',
        replaced: '預覽時存檔被換掉了。',
        policyChanged: '預覽時這些設定在別處被改了。',
        connection: 'PathKeep 和預覽的連線斷了。',
        rootChanged: '計數時有個資料夾變了。',
        uninitialized: '存檔還沒建立。',
        other: '沒做完就停了，詳情在下面。',
      },
    },
    choose: {
      title: '要保存哪些工具的對話？',
      body: '沒勾的工具不會被讀取。',
      toolsLabel: '工具',
      toolSummary_one: '對話檔 {count} 個 · {size}',
      toolSummary_other: '對話檔 {count} 個 · {size}',
      toolUnreadable: '讀不到它的資料夾',
      git: {
        title: '記下專案的 git 資訊',
        body: 'worktree 刪掉以後就查不到它屬於哪個專案，所以要在備份當下記下來。只讀 .git 裡的 repo 的根目錄、worktree 屬於哪個 repo、目前分支、遠端網址，不讀程式碼。',
      },
      schedule: '排程跟瀏覽紀錄共用一個：{schedule}。',
      scheduleEvery: '每 {interval}',
      scheduleOff: '自動備份關著',
      switcher:
        '開啟後，左上角的切換選單會多一個「{name}」（{keys} 也能切過去），切過去有洞察、紀錄、專案和它自己的備份頁。',
      switcherNoKeys:
        '開啟後，左上角的切換選單會多一個「{name}」，切過去有洞察、紀錄、專案和它自己的備份頁。',
      later:
        '之後可以在{name}的備份頁改這些選擇；要整個關掉，在「設定 › 管理資料類型」。關掉不會刪掉已經存下的對話。',
      saveFailed: '無法儲存你的選擇',
      needOne: '至少勾一個工具。',
      notConnected: '這個開發版還不能開始保存：保存對話的那一步還沒接上。',
    },
    actions: {
      noThanks: '不用了',
      next: '下一步',
      back: '上一步',
      start: '開始保存',
    },
  },
})
