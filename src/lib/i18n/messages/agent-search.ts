import { defineMessages } from '../define'

/**
 * Searching the Agent space's conversations (Records, `agent-history.md`
 * §3.4): the box, what is searched, the running and finished lines, the
 * result cards, and the no-results, index-behind and failure states.
 */
export const agentSearch = defineMessages({
  en: {
    box: {
      label: 'Search conversations',
      placeholder: 'Search all conversations…',
      placeholderProject: 'Search this project’s conversations…',
      clear: 'Clear search',
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
  },
  'zh-CN': {
    box: {
      label: '搜索对话',
      placeholder: '搜索全部对话…',
      placeholderProject: '搜索这个项目的对话…',
      clear: '清除搜索',
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
  },
  'zh-TW': {
    box: {
      label: '搜尋對話',
      placeholder: '搜尋全部對話…',
      placeholderProject: '搜尋這個專案的對話…',
      clear: '清除搜尋',
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
  },
})
