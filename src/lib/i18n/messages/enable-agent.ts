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
      unknownFiles_one:
        '{count} other file in these folders isn’t one PathKeep recognises, so it isn’t counted or saved.',
      unknownFiles_other:
        '{count} other files in these folders aren’t ones PathKeep recognises, so they aren’t counted or saved.',
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
      starting: 'Starting…',
      startFailed: 'Couldn’t start saving',
      turnedOnElsewhere:
        'Agent conversations were turned on somewhere else meanwhile.',
    },
    restart: {
      expired:
        'The preview had expired, so PathKeep looked at the folders again. Check them, then start saving.',
      changed:
        'A folder changed after the preview, so PathKeep looked at the folders again. Check them, then start saving.',
      policy:
        'These choices were changed somewhere else meanwhile, so PathKeep looked at the folders again.',
      empty:
        'None of the ticked folders could be read any more, so PathKeep looked at the folders again.',
    },
    run: {
      title: 'Saving',
      body: 'The first time reads every conversation; after that, only what is new.',
      toolsLabel: 'Saving, by tool',
      background:
        'Closing this dialog doesn’t stop the save. You can follow it in Background tasks at the bottom left.',
      continue: 'Keep going in the background',
      continued: 'Saving goes on in the background',
      continuedBody:
        'You can follow it in Background tasks at the bottom left.',
      loading: 'Finding out how far the save has got',
    },
    done: {
      title: 'Saved',
      body: 'Saved {conversations} and {subagents}, {size} in all.',
      nothingNew:
        'Nothing new to save: what these tools hold was already in the archive.',
      notes: 'Not everything was saved. What could be read is saved:',
      failedTitle: 'Saving didn’t finish',
      failedBody:
        'Saving stopped at the problem below. What was saved before it stays in the archive.',
      interruptedTitle: 'Saving stopped before the end',
      interruptedBody:
        'What was saved so far stays in the archive. Try again to save the rest.',
      inTasks:
        'Until it is saved in full, it is listed in Background tasks under Needs a look.',
      view: 'View conversations',
      withNotes_one:
        'Everything was saved, with {count} note about reading it; the backup page lists it.',
      withNotes_other:
        'Everything was saved, with {count} notes about reading it; the backup page lists them.',
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
      unknownFiles_one:
        '这些文件夹里还有 {count} 个 PathKeep 认不出来的文件，不计入，也不保存。',
      unknownFiles_other:
        '这些文件夹里还有 {count} 个 PathKeep 认不出来的文件，不计入，也不保存。',
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
        '之后可以在「{name}」的备份页改这些选择；要整个关掉，在「设置 › 管理数据类型」。关掉不会删掉已经存下的对话。',
      saveFailed: '无法保存你的选择',
      needOne: '至少勾一个工具。',
      starting: '正在开始…',
      startFailed: '没能开始保存',
      turnedOnElsewhere: 'Agent 对话刚刚在别处被开启了。',
    },
    restart: {
      expired:
        '预览已经过期，所以 PathKeep 重新看了一次文件夹。确认之后再开始保存。',
      changed:
        '预览之后有个文件夹变了，所以 PathKeep 重新看了一次文件夹。确认之后再开始保存。',
      policy: '这些设置刚刚在别处被改了，所以 PathKeep 重新看了一次文件夹。',
      empty: '勾选的文件夹都读不到了，所以 PathKeep 重新看了一次文件夹。',
    },
    run: {
      title: '正在保存',
      body: '第一次要读完全部的记录，之后只读新增的部分。',
      toolsLabel: '每个工具的保存进度',
      background: '关掉这个对话框也会继续存，进度在左下角的后台任务里看得到。',
      continue: '在后台继续',
      continued: '在后台继续保存',
      continuedBody: '进度在左下角的后台任务里看得到。',
      loading: '正在看保存进行到哪里',
    },
    done: {
      title: '存好了',
      body: '保存了 {conversations}和 {subagents}，共 {size}。',
      nothingNew: '没有新的要保存：这些工具里的内容已经在存档里了。',
      notes: '没有全部存下。读得到的都存下了：',
      failedTitle: '保存没有完成',
      failedBody: '保存停在下面这个问题。在那之前存下的留在存档里。',
      interruptedTitle: '保存中途停下了',
      interruptedBody: '已经存下的留在存档里。再试一次就会存完剩下的。',
      inTasks: '在全部存好之前，它会列在后台任务的「需要你看一下」里。',
      view: '看对话',
      withNotes_one:
        '全部都存下了，读取时留下 {count} 条备注；备份页上有列出来。',
      withNotes_other:
        '全部都存下了，读取时留下 {count} 条备注；备份页上有列出来。',
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
      unknownFiles_one:
        '這些資料夾裡還有 {count} 個 PathKeep 認不出來的檔案，不計入，也不保存。',
      unknownFiles_other:
        '這些資料夾裡還有 {count} 個 PathKeep 認不出來的檔案，不計入，也不保存。',
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
        '之後可以在「{name}」的備份頁改這些選擇；要整個關掉，在「設定 › 管理資料類型」。關掉不會刪掉已經存下的對話。',
      saveFailed: '無法儲存你的選擇',
      needOne: '至少勾一個工具。',
      starting: '正在開始…',
      startFailed: '沒能開始保存',
      turnedOnElsewhere: 'Agent 對話剛剛在別處被開啟了。',
    },
    restart: {
      expired:
        '預覽已經過期，所以 PathKeep 重新看了一次資料夾。確認之後再開始保存。',
      changed:
        '預覽之後有個資料夾變了，所以 PathKeep 重新看了一次資料夾。確認之後再開始保存。',
      policy: '這些設定剛剛在別處被改了，所以 PathKeep 重新看了一次資料夾。',
      empty: '勾選的資料夾都讀不到了，所以 PathKeep 重新看了一次資料夾。',
    },
    run: {
      title: '正在保存',
      body: '第一次要讀完全部的紀錄，之後只讀新增的部分。',
      toolsLabel: '每個工具的保存進度',
      background: '關掉這個對話框也會繼續存，進度在左下角的背景任務裡看得到。',
      continue: '在背景繼續',
      continued: '在背景繼續保存',
      continuedBody: '進度在左下角的背景任務裡看得到。',
      loading: '正在看保存進行到哪裡',
    },
    done: {
      title: '存好了',
      body: '保存了 {conversations}和 {subagents}，共 {size}。',
      nothingNew: '沒有新的要保存：這些工具裡的內容已經在存檔裡了。',
      notes: '沒有全部存下。讀得到的都存下了：',
      failedTitle: '保存沒有完成',
      failedBody: '保存停在下面這個問題。在那之前存下的留在存檔裡。',
      interruptedTitle: '保存中途停下了',
      interruptedBody: '已經存下的留在存檔裡。再試一次就會存完剩下的。',
      inTasks: '在全部存好之前，它會列在背景任務的「需要你看一下」裡。',
      view: '看對話',
      withNotes_one:
        '全部都存下了，讀取時留下 {count} 則備註；備份頁上有列出來。',
      withNotes_other:
        '全部都存下了，讀取時留下 {count} 則備註；備份頁上有列出來。',
    },
    actions: {
      noThanks: '不用了',
      next: '下一步',
      back: '上一步',
      start: '開始保存',
    },
  },
})
