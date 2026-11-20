import { defineMessages } from '../define'

/**
 * Settings › Agent conversations (`agent-history.md` §7): project grouping,
 * the reader, usage and cost, AI access and what "Continue" copies.
 */
export const settingsAgent = defineMessages({
  en: {
    backupNote:
      'Which tools to back up, other folders to add, and whether a backup reads .git are on the backup page of Agent conversations, not here.',
    openBackup: 'Open the backup page',
    saveFailed: 'Couldn’t save this setting',
    loadFailed: 'Couldn’t read these settings',
    grouping: {
      title: 'Grouping into projects',
      intro:
        'Agent tools treat every folder as a project. PathKeep puts a repository’s main folder, its worktrees and its subfolders together as one project. This changes only what PathKeep shows.',
    },
    scratch: {
      title: 'Scratch folders go to “Loose folders”',
      description:
        'Conversations in these places don’t each count as a project; they all go to “Loose folders”.',
      add: 'Add place',
      addTitle: 'Add a scratch folder',
      folder: 'Folder',
      choose: 'Choose…',
      typePath:
        'Type the folder’s full path. Everything inside it counts as scratch too.',
      confirm: 'Add',
      cancel: 'Cancel',
      notAbsolute: 'Use the full path, starting from the top of the disk.',
      already: 'This place is already in the list.',
      home: '~ (the home folder itself)',
      remove: 'Remove {place}',
      none: 'No places. Every folder counts as a project of its own.',
      saving: 'Saving…',
    },
    suggestions: {
      title: 'Folders that may belong to the same project',
      description_one:
        '{count} folder looks like it belongs to the same project as another, for example folders with the same remote, folders that moved, or worktrees whose repository can’t be read. It’s added only when you say so.',
      description_other:
        '{count} folders look like they belong to the same project as another, for example folders with the same remote, folders that moved, or worktrees whose repository can’t be read. They’re added only when you say so.',
      none: 'No folders are waiting for you to confirm.',
      unknown:
        'Folders that look like they belong to the same project wait here for you to confirm. PathKeep can’t count them right now.',
      review: 'Review',
    },
    reader: {
      title: 'Reader',
      collapse: 'Fold tool calls',
      collapseHint:
        'Each turn’s tool calls fold into one line, like “Used tools 5 times · changed +12 −3 lines”; open it to see the details.',
      thinking: 'Show thinking',
      thinkingHint:
        'Where a conversation recorded thinking, a line like “Thought for 6 s” sits above the reply; open it to read it all.',
    },
    cost: {
      title: 'Usage and cost',
      intro:
        'Token usage follows the numbers the tools wrote down. Conversations without a record are marked, not counted as 0.',
      show: 'Show costs',
      showHint:
        'When this is off, Insights and the reader show no costs at all. Claude Code and Pi write down the costs they worked out themselves; PathKeep shows those as they are.',
      estimate: 'Estimate conversations without a record at list prices',
      estimateHint:
        'Conversations without a recorded cost are put at API prices with the price table PathKeep comes with (version {version}, updated with the app). Subscriptions aren’t charged per token; this only shows the size of the usage. Local models have no price and aren’t estimated.',
      estimateOff: 'Turn on “Show costs” to choose this.',
      unavailable:
        'These can’t be read while the saved conversations can’t be opened.',
    },
    ai: {
      title: 'AI access',
      intro:
        'Conversations often hold code, keys and private paths. Each of these three is turned on on its own, and all are off at first.',
      assistant: 'Assistant can read Agent conversations',
      assistantHint:
        'The assistant can quote your Agent conversations in its answers. It uses the model you set up in “AI”; if that is an online service, the quoted parts are sent there.',
      semantic: 'Build a semantic search index',
      semanticHint:
        'Turns conversations into vectors, so search also finds passages that mean the same in other words. The vectors stay in the archive on this computer; with an online embedding service, the text is sent there.',
      mcp: 'MCP can read Agent conversations',
      mcpHint:
        'Tools connected to PathKeep’s MCP server (Agent tools included) can search and read these conversations. Never while the archive is locked.',
      allOff: 'With all three off, Agent conversations aren’t sent anywhere.',
    },
    resume: {
      title: 'Continue',
      row: '“Continue” copies one command',
      rowHint:
        'In the reader, “Continue” copies one command (cd to the working folder, then the tool’s resume command) to paste into a terminal and carry on. For tool versions whose resume command PathKeep hasn’t confirmed, it copies the session ID instead. PathKeep never opens a terminal or starts a tool for you.',
      notYet:
        'Not offered yet: putting conversations a tool deleted back into its folder, and moving conversations from a worktree back to the main project. Both mean writing into the tool’s own folder; they will be tried in isolation first, to make sure they can’t damage the tool’s data, before deciding whether to offer them.',
    },
    off: 'Agent conversations are off, so they have no settings here. Turn them on in Manage data types.',
  },
  'zh-CN': {
    backupNote:
      '要备份哪些工具、加入其他文件夹、备份时要不要读 .git，都在 Agent 对话的备份页，不在这里。',
    openBackup: '打开备份页',
    saveFailed: '没能保存这项设置',
    loadFailed: '没能读取这些设置',
    grouping: {
      title: '项目归类',
      intro:
        'Agent 把每个文件夹都当成一个项目。PathKeep 会把同一个 repo 的主文件夹、worktree 和子文件夹并成一个项目，只改 PathKeep 这边的显示。',
    },
    scratch: {
      title: '暂存文件夹归到“零散文件夹”',
      description: '在这些位置里的对话不各自算一个项目，全部放进“零散文件夹”。',
      add: '加入位置',
      addTitle: '加入暂存文件夹',
      folder: '文件夹',
      choose: '选择…',
      typePath: '输入文件夹的完整路径。它里面的所有位置也都算暂存。',
      confirm: '加入',
      cancel: '取消',
      notAbsolute: '请用完整路径，从磁盘的最上层开始。',
      already: '这个位置已经在列表里了。',
      home: '~（主目录本身）',
      remove: '移除 {place}',
      none: '没有位置。每个文件夹都各自算一个项目。',
      saving: '正在保存…',
    },
    suggestions: {
      title: '可能属于同一个项目的文件夹',
      description_one:
        '有 {count} 个文件夹看起来属于同一个项目，例如远程地址相同的文件夹、搬过家的文件夹、读不到 repo 的 worktree。要你确认才会加进去。',
      description_other:
        '有 {count} 个文件夹看起来属于同一个项目，例如远程地址相同的文件夹、搬过家的文件夹、读不到 repo 的 worktree。要你确认才会加进去。',
      none: '没有等你确认的文件夹。',
      unknown:
        '看起来属于同一个项目的文件夹会在这里等你确认。PathKeep 现在数不出有几个。',
      review: '去看看',
    },
    reader: {
      title: '阅读器',
      collapse: '默认收起工具调用',
      collapseHint:
        '每一轮的工具调用收成一行，例如“用了 5 次工具 · 改了 +12 −3 行”，点开才看细节。',
      thinking: '显示思考过程',
      thinkingHint:
        '有记录 thinking 的对话，在回复上方显示一行“思考了 6 秒”，点开才看全文。',
    },
    cost: {
      title: '用量与费用',
      intro:
        'Token 用量照工具自己记的数字算；没有记录的对话会标出来，不当成 0。',
      show: '显示费用',
      showHint:
        '关掉时，洞察和阅读器都不显示任何费用。Claude Code 与 Pi 会在记录里写下它们自己算的费用，PathKeep 照抄，不重算。',
      estimate: '没有记录的对话按牌价估算',
      estimateHint:
        '没有记费用的对话，按 PathKeep 附带的牌价表（{version} 版，跟着 app 更新）换算成等值金额。订阅方案不按 token 收费，这只是看用量的分量。本地模型没有牌价，不估。',
      estimateOff: '先打开“显示费用”才能切换这一项。',
      unavailable: '已保存的对话打不开时，读不到这两项。',
    },
    ai: {
      title: 'AI 访问',
      intro: '对话里常有代码、密钥和私人路径。下面三项分开开，默认都关。',
      assistant: '助手可以读 Agent 对话',
      assistantHint:
        '助手回答时可以引用你的 Agent 对话。用的是你在“AI”里设置的模型；如果那是远程服务，引用到的段落会发送过去。',
      semantic: '建立语义搜索索引',
      semanticHint:
        '把对话内容转成向量，让搜索也能找到意思相近、字面不同的段落。向量存在本机存档里；用远程 embedding 服务时，内容会发送过去。',
      mcp: 'MCP 可以读 Agent 对话',
      mcpHint:
        '接上 PathKeep MCP 的工具（包括 Agent 自己）可以搜索和读取这些对话。存档锁定时一律读不到。',
      allOff: '这三项都关着时，Agent 对话不会发送到任何地方。',
    },
    resume: {
      title: '继续对话',
      row: '“继续对话”会复制一行命令',
      rowHint:
        '在阅读器里按“继续对话”，会复制一行命令（cd 到工作文件夹，再接工具的接续命令），粘贴到终端就能接着做。PathKeep 还没确认过接续命令的工具版本，按下去只复制会话 ID。PathKeep 不会替你打开终端或启动工具。',
      notYet:
        '还没提供：把工具已经删掉的对话放回工具的文件夹、把 worktree 里的对话搬回主项目。这两件事都要写进工具自己的文件夹，会先在隔离环境里试过，确定不会弄坏工具的数据再决定要不要做。',
    },
    off: 'Agent 对话已关闭，所以这里没有它的设置。可以在“管理数据类型”里打开。',
  },
  'zh-TW': {
    backupNote:
      '要備份哪些工具、加入其他資料夾、備份時要不要讀 .git，都在 Agent 對話的備份頁，不在這裡。',
    openBackup: '打開備份頁',
    saveFailed: '沒能儲存這項設定',
    loadFailed: '沒能讀取這些設定',
    grouping: {
      title: '專案歸類',
      intro:
        'Agent 把每個資料夾都當成一個專案。PathKeep 會把同一個 repo 的主資料夾、worktree 和子資料夾併成一個專案，只改 PathKeep 這邊的顯示。',
    },
    scratch: {
      title: '暫存資料夾歸到「零散資料夾」',
      description:
        '在這些位置裡的對話不各自算一個專案，全部放進「零散資料夾」。',
      add: '加入位置',
      addTitle: '加入暫存資料夾',
      folder: '資料夾',
      choose: '選擇…',
      typePath: '輸入資料夾的完整路徑。它裡面的所有位置也都算暫存。',
      confirm: '加入',
      cancel: '取消',
      notAbsolute: '請用完整路徑，從磁碟的最上層開始。',
      already: '這個位置已經在清單裡了。',
      home: '~（家目錄本身）',
      remove: '移除 {place}',
      none: '沒有位置。每個資料夾都各自算一個專案。',
      saving: '正在儲存…',
    },
    suggestions: {
      title: '可能屬於同一個專案的資料夾',
      description_one:
        '有 {count} 個資料夾看起來屬於同一個專案，例如遠端網址相同的資料夾、搬過家的資料夾、讀不到 repo 的 worktree。要你確認才會加進去。',
      description_other:
        '有 {count} 個資料夾看起來屬於同一個專案，例如遠端網址相同的資料夾、搬過家的資料夾、讀不到 repo 的 worktree。要你確認才會加進去。',
      none: '沒有等你確認的資料夾。',
      unknown:
        '看起來屬於同一個專案的資料夾會在這裡等你確認。PathKeep 現在數不出有幾個。',
      review: '去看看',
    },
    reader: {
      title: '閱讀器',
      collapse: '預設收起工具呼叫',
      collapseHint:
        '每一輪的工具呼叫收成一行，例如「用了 5 次工具 · 改了 +12 −3 行」，點開才看細節。',
      thinking: '顯示思考過程',
      thinkingHint:
        '有紀錄 thinking 的對話，在回覆上方顯示一行「思考了 6 秒」，點開才看全文。',
    },
    cost: {
      title: '用量與費用',
      intro:
        'Token 用量照工具自己記的數字算；沒有紀錄的對話會標出來，不當成 0。',
      show: '顯示費用',
      showHint:
        '關掉時，洞察和閱讀器都不顯示任何費用。Claude Code 與 Pi 會在紀錄裡寫下它們自己算的費用，PathKeep 照抄，不重算。',
      estimate: '沒有紀錄的對話按牌價估算',
      estimateHint:
        '沒有記費用的對話，按 PathKeep 附的牌價表（{version} 版，跟著 app 更新）換算成等值金額。訂閱方案不按 token 收費，這只是看用量的分量。本機模型沒有牌價，不估。',
      estimateOff: '先打開「顯示費用」才能切換這一項。',
      unavailable: '已存的對話打不開時，讀不到這兩項。',
    },
    ai: {
      title: 'AI 存取',
      intro: '對話裡常有程式碼、金鑰和私人路徑。下面三項分開開，預設都關。',
      assistant: '助理可以讀 Agent 對話',
      assistantHint:
        '助理回答時可以引用你的 Agent 對話。用的是你在「AI」裡設定的模型；如果那是遠端服務，引用到的段落會送過去。',
      semantic: '建立語意搜尋索引',
      semanticHint:
        '把對話內容轉成向量，讓搜尋也能找到意思相近、字面不同的段落。向量存在本機存檔裡；用遠端 embedding 服務時，內容會送過去。',
      mcp: 'MCP 可以讀 Agent 對話',
      mcpHint:
        '接上 PathKeep MCP 的工具（包括 Agent 自己）可以搜尋和讀取這些對話。存檔鎖定時一律讀不到。',
      allOff: '這三個開關都關著時，Agent 對話不會送到任何地方。',
    },
    resume: {
      title: '繼續對話',
      row: '「繼續對話」會複製一行指令',
      rowHint:
        '閱讀器裡按「繼續對話」，會複製一行指令（cd 到工作資料夾，再接工具的接續指令），貼到終端機就能接著做。PathKeep 還沒確認過接續指令的工具版本，按下去只複製工作階段 ID。PathKeep 不會替你開終端機或啟動工具。',
      notYet:
        '還沒提供：把工具已經刪掉的對話放回工具的資料夾、把 worktree 裡的對話搬回主專案。這兩件事都要寫進工具自己的資料夾，會先在隔離環境裡試過，確定不會弄壞工具的資料再決定要不要做。',
    },
    off: 'Agent 對話已關閉，所以這裡沒有它的設定。可以在「管理資料類型」裡打開。',
  },
})
