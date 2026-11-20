import { defineMessages } from '../define'

/**
 * Searching the Agent space's conversations (Records, `agent-history.md`
 * §3.4): the box, how it matches and what is searched, the running and
 * finished lines, the result cards, and the no-results, index-behind and
 * failure states; for a search by meaning, its lines and what stands in
 * its way (AI settings, the switch, the index).
 */
export const agentSearch = defineMessages({
  en: {
    box: {
      label: 'Search conversations',
      placeholder: 'Search all conversations…',
      placeholderProject: 'Search this project’s conversations…',
      placeholderSemantic: 'Describe what you’re looking for…',
      placeholderSemanticProject:
        'Describe what you’re looking for in this project…',
      clear: 'Clear search',
    },
    mode: {
      label: 'Search mode',
      text: 'Full text',
      semantic: 'Semantic',
    },
    hint: {
      everything: 'search every conversation',
      project: 'search this project’s conversations',
    },
    results: 'Search results',
    roles: {
      label: 'What to search',
      user: 'Your words',
      agent: 'Agent’s words',
      tool: 'Tool output',
      thinking: 'Thinking',
      lastOne: 'Keep at least one on',
    },
    scope: {
      user: 'what you said',
      agent: 'what the agent said',
      tool: 'tool output',
      thinking: 'thinking',
    },
    status: {
      starting: 'Searching…',
      found_one: '{shown} conversation',
      found_other: '{shown} conversations',
      conversations_one: '{shown} conversation',
      conversations_other: '{shown} conversations',
      matches_one: '{shown} match',
      matches_other: '{shown} matches',
      searching: 'Found {found}, {matches} · searching {progress}',
      stopped: 'Stopped: found {found}, {matches} · searched {progress}',
      done: '{conversations}, {matches} in all · searched {searched}',
      searchedAll_one: '{shown} conversation',
      searchedAll_other: '{shown} conversations',
      searchedFiltered_one: '{shown} filtered conversation',
      searchedFiltered_other: '{shown} filtered conversations',
    },
    stop: 'Stop',
    progress: 'Search progress',
    indexLag_one:
      '{count} conversation isn’t in the search index yet (a backup is saving it); this searched the last finished index.',
    indexLag_other:
      '{count} conversations aren’t in the search index yet (a backup is saving them); this searched the last finished index.',
    card: {
      matches_one: '{shown} match',
      matches_other: '{shown} matches',
      more_one:
        '{shown} more in this conversation; open it and press Enter to step through',
      more_other:
        '{shown} more in this conversation; open it and press Enter to step through',
      titleOnly: 'The title matches',
      subagent: 'subagent',
    },
    role: {
      user: 'You',
      agent: 'Agent',
      tool: 'Tool',
      thinking: 'Thinking',
      title: 'Title',
    },
    empty: {
      title: 'No conversation mentions “{query}”.',
      all_one:
        'Searched what you said, what the agent said, tool output and thinking in {count} conversation.',
      all_other:
        'Searched what you said, what the agent said, tool output and thinking in all {count} conversations.',
      some_one:
        'Searched {kinds} in the {count} conversation that matches the filters.',
      some_other:
        'Searched {kinds} in the {count} conversations that match the filters.',
    },
    error: {
      regex: 'This regular expression is wrong',
      index: 'Couldn’t read the search index',
      failed: 'The search didn’t work',
    },
    semantic: {
      status: {
        starting: 'Searching by meaning…',
        searching_one: 'Found {shown} passage so far…',
        searching_other: 'Found {shown} passages so far…',
        stopped_one: 'Stopped: found {shown} passage',
        stopped_other: 'Stopped: found {shown} passages',
        done_one: '{shown} passage close in meaning',
        done_other: '{shown} passages close in meaning',
      },
      using: 'Semantic search with {model}, built {when}',
      stale:
        'Conversations changed after it was built, so the newest may be missing.',
      updating: 'Conversations changed after it was built; it’s being updated.',
      empty: {
        title:
          'Nothing in your conversations is close in meaning to “{query}”.',
        body: 'Try saying it another way, or search the full text.',
      },
      textInstead: 'Search the full text instead',
      settings: 'Settings › {section}',
      blocked: {
        aiOffTitle: 'Semantic search is off',
        aiOff: 'Turn on {ai} in Settings › AI.',
        aiOffSwitch:
          'Then turn on “{switch}” in Settings › Agent conversations.',
        switchOffTitle: 'Semantic search of conversations is off',
        switchOff: 'Turn on “{switch}” in Settings › Agent conversations.',
        offTitle: 'Semantic search is off',
        off: 'It needs {ai} on in Settings › AI and “{switch}” on in Settings › Agent conversations.',
        noModelTitle: 'Semantic search needs an embedding model',
        noModel: 'Choose one in Settings › AI.',
        buildingTitle: 'The semantic index is being built',
        building_one:
          '{shown} passage read so far. You can search by meaning once it’s done; full-text search works meanwhile.',
        building_other:
          '{shown} passages read so far. You can search by meaning once it’s done; full-text search works meanwhile.',
        pausedTitle: 'Building the semantic index is paused',
        paused_one:
          '{shown} passage read so far. Continue it in Settings › Agent conversations.',
        paused_other:
          '{shown} passages read so far. Continue it in Settings › Agent conversations.',
        failedTitle: 'The last build of the semantic index failed',
        failed: 'Try again in Settings › Agent conversations.',
        notBuiltTitle: 'There’s no semantic index of your conversations yet',
        notBuilt: 'Build it in Settings › Agent conversations.',
        notReadyTitle: 'The semantic index isn’t ready',
        notReady:
          'It may have been built with another model, or it’s being made again. See Settings › Agent conversations.',
        unknownTitle: 'Couldn’t read the semantic index',
      },
    },
  },
  'zh-CN': {
    box: {
      label: '搜索对话',
      placeholder: '搜索全部对话…',
      placeholderProject: '搜索这个项目的对话…',
      placeholderSemantic: '描述你要找的内容…',
      placeholderSemanticProject: '描述你要在这个项目里找的内容…',
      clear: '清除搜索',
    },
    mode: {
      label: '搜索方式',
      text: '全文',
      semantic: '语义',
    },
    hint: {
      everything: '搜索全部对话',
      project: '搜索这个项目的对话',
    },
    results: '搜索结果',
    roles: {
      label: '搜哪些内容',
      user: '你说的',
      agent: 'Agent 说的',
      tool: '工具输出',
      thinking: '思考',
      lastOne: '至少留一项开着',
    },
    scope: {
      user: '你说的',
      agent: 'Agent 说的',
      tool: '工具输出',
      thinking: '思考',
    },
    status: {
      starting: '正在搜索…',
      found_one: '{shown} 段',
      found_other: '{shown} 段',
      conversations_one: '{shown} 段对话',
      conversations_other: '{shown} 段对话',
      matches_one: '{shown} 处',
      matches_other: '{shown} 处',
      searching: '已找到 {found}、{matches} · 搜索中 {progress}',
      stopped: '已停止：找到 {found}、{matches} · 搜过 {progress}',
      done: '{conversations}、共 {matches}符合 · 搜完{searched}',
      searchedAll_one: ' {shown} 段',
      searchedAll_other: ' {shown} 段',
      searchedFiltered_one: '筛选后的 {shown} 段',
      searchedFiltered_other: '筛选后的 {shown} 段',
    },
    stop: '停止',
    progress: '搜索进度',
    indexLag_one:
      '还有 {count} 段对话没进索引（备份正在保存）；这次搜的是上一次存好的索引。',
    indexLag_other:
      '还有 {count} 段对话没进索引（备份正在保存）；这次搜的是上一次存好的索引。',
    card: {
      matches_one: '{shown} 处',
      matches_other: '{shown} 处',
      more_one: '这段对话里还有 {shown} 处，打开后按 Enter 逐一看',
      more_other: '这段对话里还有 {shown} 处，打开后按 Enter 逐一看',
      titleOnly: '标题符合',
      subagent: 'subagent',
    },
    role: {
      user: '你',
      agent: 'Agent',
      tool: '工具',
      thinking: '思考',
      title: '标题',
    },
    empty: {
      title: '没有对话提到「{query}」。',
      all_one:
        '搜索涵盖全部 {count} 段对话里你说的、Agent 说的、工具输出和思考。',
      all_other:
        '搜索涵盖全部 {count} 段对话里你说的、Agent 说的、工具输出和思考。',
      some_one: '搜索涵盖符合筛选的 {count} 段对话里{kinds}。',
      some_other: '搜索涵盖符合筛选的 {count} 段对话里{kinds}。',
    },
    error: {
      regex: '这个正则表达式写错了',
      index: '读不到搜索索引',
      failed: '搜索失败了',
    },
    semantic: {
      status: {
        starting: '正在按意思搜索…',
        searching_one: '已找到 {shown} 个段落…',
        searching_other: '已找到 {shown} 个段落…',
        stopped_one: '已停止：找到 {shown} 个段落',
        stopped_other: '已停止：找到 {shown} 个段落',
        done_one: '找到 {shown} 个意思相近的段落',
        done_other: '找到 {shown} 个意思相近的段落',
      },
      using: '语义搜索用{model}，{when}建立',
      stale: '建立之后对话有变动，最新的内容可能还找不到。',
      updating: '建立之后对话有变动，正在更新。',
      empty: {
        title: '对话里没有和「{query}」意思相近的内容。',
        body: '换个说法试试，或者改搜全文。',
      },
      textInstead: '改搜全文',
      settings: '设置 › {section}',
      blocked: {
        aiOffTitle: '语义搜索关着',
        aiOff: '在“设置 › AI”里打开“{ai}”。',
        aiOffSwitch: '然后在“设置 › Agent 对话”里打开“{switch}”。',
        switchOffTitle: '对话的语义搜索关着',
        switchOff: '在“设置 › Agent 对话”里打开“{switch}”。',
        offTitle: '语义搜索关着',
        off: '需要打开“设置 › AI”里的“{ai}”和“设置 › Agent 对话”里的“{switch}”。',
        noModelTitle: '语义搜索需要 embedding 模型',
        noModel: '在“设置 › AI”里选一个。',
        buildingTitle: '语义索引正在建立',
        building_one:
          '已读了 {shown} 个段落。建好之后才能按意思搜索，这期间全文搜索照常可用。',
        building_other:
          '已读了 {shown} 个段落。建好之后才能按意思搜索，这期间全文搜索照常可用。',
        pausedTitle: '语义索引的建立已暂停',
        paused_one: '已读了 {shown} 个段落。可以在“设置 › Agent 对话”里继续。',
        paused_other:
          '已读了 {shown} 个段落。可以在“设置 › Agent 对话”里继续。',
        failedTitle: '上次建立语义索引失败了',
        failed: '可以在“设置 › Agent 对话”里再试一次。',
        notBuiltTitle: '对话还没有语义索引',
        notBuilt: '可以在“设置 › Agent 对话”里建立。',
        notReadyTitle: '语义索引还不能用',
        notReady:
          '它可能是用别的模型建立的，或者正在重新建立。到“设置 › Agent 对话”看看。',
        unknownTitle: '读不到语义索引的状态',
      },
    },
  },
  'zh-TW': {
    box: {
      label: '搜尋對話',
      placeholder: '搜尋全部對話…',
      placeholderProject: '搜尋這個專案的對話…',
      placeholderSemantic: '描述你要找的內容…',
      placeholderSemanticProject: '描述你要在這個專案裡找的內容…',
      clear: '清除搜尋',
    },
    mode: {
      label: '搜尋方式',
      text: '全文',
      semantic: '語意',
    },
    hint: {
      everything: '搜尋全部對話',
      project: '搜尋這個專案的對話',
    },
    results: '搜尋結果',
    roles: {
      label: '搜哪些內容',
      user: '你說的',
      agent: 'Agent 說的',
      tool: '工具輸出',
      thinking: '思考',
      lastOne: '至少留一顆開著',
    },
    scope: {
      user: '你說的',
      agent: 'Agent 說的',
      tool: '工具輸出',
      thinking: '思考',
    },
    status: {
      starting: '搜尋中…',
      found_one: '{shown} 段',
      found_other: '{shown} 段',
      conversations_one: '{shown} 段對話',
      conversations_other: '{shown} 段對話',
      matches_one: '{shown} 處',
      matches_other: '{shown} 處',
      searching: '已找到 {found}、{matches} · 搜尋中 {progress}',
      stopped: '已停止：找到 {found}、{matches} · 搜過 {progress}',
      done: '{conversations}、共 {matches}符合 · 搜完{searched}',
      searchedAll_one: ' {shown} 段',
      searchedAll_other: ' {shown} 段',
      searchedFiltered_one: '篩選後的 {shown} 段',
      searchedFiltered_other: '篩選後的 {shown} 段',
    },
    stop: '停止',
    progress: '搜尋進度',
    indexLag_one:
      '還有 {count} 段對話沒進索引（備份正在存）；這次搜的是上一次存好的索引。',
    indexLag_other:
      '還有 {count} 段對話沒進索引（備份正在存）；這次搜的是上一次存好的索引。',
    card: {
      matches_one: '{shown} 處',
      matches_other: '{shown} 處',
      more_one: '這段對話裡還有 {shown} 處，打開後按 Enter 逐一看',
      more_other: '這段對話裡還有 {shown} 處，打開後按 Enter 逐一看',
      titleOnly: '標題符合',
      subagent: 'subagent',
    },
    role: {
      user: '你',
      agent: 'Agent',
      tool: '工具',
      thinking: '思考',
      title: '標題',
    },
    empty: {
      title: '沒有對話提到「{query}」。',
      all_one:
        '搜尋涵蓋全部 {count} 段對話裡你說的、Agent 說的、工具輸出和思考。',
      all_other:
        '搜尋涵蓋全部 {count} 段對話裡你說的、Agent 說的、工具輸出和思考。',
      some_one: '搜尋涵蓋符合篩選的 {count} 段對話裡{kinds}。',
      some_other: '搜尋涵蓋符合篩選的 {count} 段對話裡{kinds}。',
    },
    error: {
      regex: '這個正規表示式寫錯了',
      index: '讀不到搜尋索引',
      failed: '搜尋失敗了',
    },
    semantic: {
      status: {
        starting: '按意思搜尋中…',
        searching_one: '已找到 {shown} 個段落…',
        searching_other: '已找到 {shown} 個段落…',
        stopped_one: '已停止：找到 {shown} 個段落',
        stopped_other: '已停止：找到 {shown} 個段落',
        done_one: '找到 {shown} 個意思相近的段落',
        done_other: '找到 {shown} 個意思相近的段落',
      },
      using: '語意搜尋用{model}，{when}建立',
      stale: '建立之後對話有變動，最新的內容可能還找不到。',
      updating: '建立之後對話有變動，正在更新。',
      empty: {
        title: '對話裡沒有和「{query}」意思相近的內容。',
        body: '換個說法試試，或者改搜全文。',
      },
      textInstead: '改搜全文',
      settings: '設定 › {section}',
      blocked: {
        aiOffTitle: '語意搜尋關著',
        aiOff: '在「設定 › AI」裡打開「{ai}」。',
        aiOffSwitch: '然後在「設定 › Agent 對話」裡打開「{switch}」。',
        switchOffTitle: '對話的語意搜尋關著',
        switchOff: '在「設定 › Agent 對話」裡打開「{switch}」。',
        offTitle: '語意搜尋關著',
        off: '需要打開「設定 › AI」裡的「{ai}」和「設定 › Agent 對話」裡的「{switch}」。',
        noModelTitle: '語意搜尋需要 embedding 模型',
        noModel: '在「設定 › AI」裡選一個。',
        buildingTitle: '語意索引正在建立',
        building_one:
          '已讀了 {shown} 個段落。建好之後才能按意思搜尋，這段期間全文搜尋照常可用。',
        building_other:
          '已讀了 {shown} 個段落。建好之後才能按意思搜尋，這段期間全文搜尋照常可用。',
        pausedTitle: '語意索引的建立已暫停',
        paused_one:
          '已讀了 {shown} 個段落。可以在「設定 › Agent 對話」裡繼續。',
        paused_other:
          '已讀了 {shown} 個段落。可以在「設定 › Agent 對話」裡繼續。',
        failedTitle: '上次建立語意索引失敗了',
        failed: '可以在「設定 › Agent 對話」裡再試一次。',
        notBuiltTitle: '對話還沒有語意索引',
        notBuilt: '可以在「設定 › Agent 對話」裡建立。',
        notReadyTitle: '語意索引還不能用',
        notReady:
          '它可能是用別的模型建立的，或者正在重新建立。到「設定 › Agent 對話」看看。',
        unknownTitle: '讀不到語意索引的狀態',
      },
    },
  },
})
