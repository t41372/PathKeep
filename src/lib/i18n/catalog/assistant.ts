/**
 * @file assistant.ts
 * @description Owns assistant surface copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `assistant` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so assistant copy stays separate from provider/runtime integration.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `assistant` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const assistantNamespaceCatalog = {
  en: {
    archiveNotInitializedTitle: 'Archive not set up',
    archiveNotInitializedDescription:
      'Set up your archive first so the assistant has history to search.',
    goToSetup: 'Go to setup',
    lockedTitle: 'Unlock to use the assistant',
    lockedDetail:
      'The archive is locked. Unlock it to ask questions about your history.',
    lockedEyebrow: 'LOCKED',
    disabledTitle: 'Assistant is turned off',
    disabledBody:
      'Assistant and smart search are coming in v0.3. Keyword search and Core Intelligence are available now.',
    deferredTitle: 'Assistant is coming in v0.3',
    deferredBody:
      'This part of PathKeep is tracked for v0.3. v0.2.0 ships the local archive, keyword search, and Core Intelligence first.',
    deferredPanelEyebrow: 'IN PROGRESS',
    deferredBadge: 'Coming in v0.3',
    deferredPanelBody:
      'The path is still extending. Assistant answers, embeddings, and vector search will open in a future release after they are reliable enough to trust.',
    statusEyebrow: 'STATUS',
    testProvider: 'Test connection',
    refreshQueue: 'Refresh',
    openSettings: 'Open settings',
    reviewSecurity: 'Review security',
    scopedViewTitle: 'Profile-scoped answers',
    scopedViewBody:
      'The assistant is only searching {profile} right now. Clear the shared profile scope to ask about your whole archive.',
    runningContext: 'CONNECTION',
    noLlmProviderSelected: 'No AI provider selected',
    llm: 'AI Model',
    retrieval: 'Search',
    unset: 'Not set',
    lexicalFallback: 'Keyword only',
    queuedLabel: 'Queued',
    runningLabel: 'Running',
    queuedJobLabel: 'assistant · #{id}',
    providerReachable: 'Connected',
    providerNeedsAttention: 'Connection issue',
    providerProbeLatency: '{model} · {latency} ms',
    responseMeta: 'Job #{jobId} · Run #{runId} · {provider}',
    queuedAssistantRequest: 'Your question is queued.',
    cancelledQueuedRequest:
      'This question was cancelled before it was processed.',
    attentionTitle: 'Assistant needs attention',
    loadExamplePrompt: 'Try an example',
    examplePrompt: 'What have I been reading about recently?',
    examplePromptFocus: 'Which topic got most of my attention this week?',
    examplePromptTimeline: 'When did I last spend time reading about SQLite?',
    emptyEyebrow: 'ASK YOUR HISTORY',
    emptyTitle: 'Ask a question',
    emptyDescription:
      'The assistant answers based on your browsing history and always cites the pages it used.',
    evidenceLabel: 'Sources · {count} records',
    checkStatus: 'Check status',
    runQueuedJob: 'Run now',
    cancel: 'Cancel',
    working: 'Working',
    preparingAnswer: 'Finding an answer…',
    searchingArchive: 'Searching your history and gathering sources…',
    inputPlaceholder: 'Ask about your browsing history…',
    inputLabel: 'Your question',
    sendAction: 'Send',
    auditTraceHint:
      'Every answer shows its sources, the AI provider used, and a run ID for auditing.',
    queueBoundary: 'ABOUT THE QUEUE',
    queueBoundaryBody:
      "Queued questions can be cancelled. Once processing starts, you'll need to wait for it to finish.",
    queueProgressLabel: '{queued} queued / {running} running',
    loadingQueueAction: 'Loading…',
    testingProviderAction: 'Testing connection…',
    loadingQueuedAnswerAction: 'Loading answer…',
    runningQueuedJobsAction: 'Processing…',
    cancellingAssistantJobAction: 'Cancelling…',
    failedResponse: "Couldn't get an answer",
    paperGreetingTitle: 'Ask your archive anything.',
    paperGreetingSubtitle:
      'Answers come grounded in the pages you actually visited — every claim cites a record. Nothing leaves your machine.',
    paperPrompt1: 'Where did I read about Rust async runtimes last week?',
    paperPrompt2: 'When did I last spend time with SQLite?',
    paperPrompt3: 'Which topic took most of my attention this month?',
    paperComposerPlaceholder:
      'Ask in plain English — the assistant cites every source.',
    paperComposerSendLabel: 'Send',
    paperComposerAttribution: 'Local · {provider}',
    paperComposerAttributionFallback: 'Local · keyword only',
    paperComposerKeyHint: '↵ send · ⇧↵ newline',
    paperEvidenceLabel: 'Sources · {count} records',
    paperUserByline: 'You',
    paperAssistantByline: 'Local · {provider}',
  },
  'zh-CN': {
    archiveNotInitializedTitle: '数据库尚未初始化',
    archiveNotInitializedDescription:
      'AI 助手需要先初始化数据库，才能搜索你的浏览历史。',
    goToSetup: '前往设置',
    lockedTitle: '请先解锁数据库',
    lockedDetail: '解锁数据库后才能使用 AI 助手。',
    lockedEyebrow: '已锁定',
    disabledTitle: 'AI 助手已关闭',
    disabledBody:
      'AI 助手和智能搜索会在 v0.3 开放。现在可以使用关键词搜索和确定性智能分析。',
    deferredTitle: 'AI 助手将在 v0.3 中开放',
    deferredBody:
      '这个功能已排入 v0.3。v0.2.0 会先把本地存档、关键词搜索和确定性智能分析做好。',
    deferredPanelEyebrow: '建设中',
    deferredBadge: 'v0.3 开放',
    deferredPanelBody:
      '这条路还在延伸。助手回答、embedding 和向量搜索会在足够可靠之后，于后续版本开放。',
    statusEyebrow: '助手状态',
    testProvider: '测试连接',
    refreshQueue: '刷新队列',
    openSettings: '前往设置',
    reviewSecurity: '查看安全设置',
    scopedViewTitle: '当前只搜索这个浏览器',
    scopedViewBody:
      '助手当前只会搜索 {profile} 的记录。清除顶部的共享浏览器范围后，才能询问整个存档。',
    runningContext: '当前配置',
    noLlmProviderSelected: '未选择 AI 模型',
    llm: 'LLM',
    retrieval: '搜索方式',
    unset: '未设置',
    lexicalFallback: '关键词搜索',
    queuedLabel: '排队中',
    runningLabel: '处理中',
    queuedJobLabel: '助手 · #{id}',
    providerReachable: '连接正常',
    providerNeedsAttention: '连接异常',
    providerProbeLatency: '{model} · {latency} ms',
    responseMeta: '任务 #{jobId} · 运行 #{runId} · {provider}',
    queuedAssistantRequest: '问题已加入队列，稍后会自动处理。',
    cancelledQueuedRequest: '已取消排队中的提问。',
    attentionTitle: '助手需要处理',
    loadExamplePrompt: '试试这个例子',
    examplePrompt: '我最近看了哪些关于语义搜索的文章？',
    examplePromptFocus: '最近一周我最专注的主题是什么？',
    examplePromptTimeline: '我上次认真研究 SQLite 是什么时候？',
    emptyEyebrow: '基于你的浏览记录',
    emptyTitle: '问点什么吧',
    emptyDescription:
      '助手会根据你的浏览记录来回答问题，并标注引用来源。试着问一个具体的问题。',
    evidenceLabel: '引用来源 · {count} 条记录',
    checkStatus: '查看状态',
    runQueuedJob: '立即处理',
    cancel: '取消',
    working: '思考中',
    preparingAnswer: '正在搜索相关记录并生成回答',
    searchingArchive: '正在搜索历史记录…',
    inputPlaceholder: '问问关于你浏览历史的问题…',
    inputLabel: '你的问题',
    sendAction: '发送',
    auditTraceHint:
      '每个回答都会保留完整的追溯信息：AI 模型、任务编号、引用来源等。',
    queueBoundary: '队列说明',
    queueBoundaryBody:
      '排队中的任务可以取消。但任务一旦开始处理，就无法中途停止，只能等待完成。',
    queueProgressLabel: '{queued} 排队中 / {running} 处理中',
    loadingQueueAction: '正在加载任务队列',
    testingProviderAction: '正在测试连接',
    loadingQueuedAnswerAction: '正在加载回答',
    runningQueuedJobsAction: '正在处理任务',
    cancellingAssistantJobAction: '正在取消任务',
    failedResponse: '获取回答失败',
    paperGreetingTitle: '问你的存档任何问题。',
    paperGreetingSubtitle:
      '回答都基于你真正浏览过的页面 — 每一条结论都会注明出处。一切都在本机完成。',
    paperPrompt1: '上周我在哪里看的 Rust 异步运行时？',
    paperPrompt2: '我上次认真研究 SQLite 是什么时候？',
    paperPrompt3: '这个月最占用我注意力的话题是什么？',
    paperComposerPlaceholder: '用自然语言提问 — 助手会注明每一条引用。',
    paperComposerSendLabel: '发送',
    paperComposerAttribution: '本机 · {provider}',
    paperComposerAttributionFallback: '本机 · 关键词模式',
    paperComposerKeyHint: '↵ 发送 · ⇧↵ 换行',
    paperEvidenceLabel: '引用来源 · {count} 条记录',
    paperUserByline: '你',
    paperAssistantByline: '本机 · {provider}',
  },
  'zh-TW': {
    archiveNotInitializedTitle: '資料庫尚未初始化',
    archiveNotInitializedDescription:
      'AI 助手需要先初始化資料庫，才能搜尋你的瀏覽歷史。',
    goToSetup: '前往設定',
    lockedTitle: '請先解鎖資料庫',
    lockedDetail: '解鎖資料庫後才能使用 AI 助手。',
    lockedEyebrow: '已鎖定',
    disabledTitle: 'AI 助手已關閉',
    disabledBody:
      'AI 助手和智慧搜尋會在 v0.3 開放。現在可以使用關鍵字搜尋和確定性智慧分析。',
    deferredTitle: 'AI 助手會在 v0.3 開放',
    deferredBody:
      '這個功能已排入 v0.3。v0.2.0 會先把本機封存、關鍵字搜尋和確定性智慧分析做好。',
    deferredPanelEyebrow: '建設中',
    deferredBadge: 'v0.3 開放',
    deferredPanelBody:
      '這條路還在延伸。助手回答、embedding 和向量搜尋會在足夠可靠之後，於後續版本開放。',
    statusEyebrow: '助手狀態',
    testProvider: '測試連線',
    refreshQueue: '重新整理佇列',
    openSettings: '前往設定',
    reviewSecurity: '查看安全設定',
    scopedViewTitle: '目前只搜尋這個瀏覽器',
    scopedViewBody:
      '助手目前只會搜尋 {profile} 的紀錄。清除頂部的共享瀏覽器範圍後，才能詢問整個歸檔。',
    runningContext: '目前設定',
    noLlmProviderSelected: '尚未選擇 AI 模型',
    llm: 'LLM',
    retrieval: '搜尋方式',
    unset: '未設定',
    lexicalFallback: '關鍵字搜尋',
    queuedLabel: '排隊中',
    runningLabel: '處理中',
    queuedJobLabel: '助手 · #{id}',
    providerReachable: '連線正常',
    providerNeedsAttention: '連線異常',
    providerProbeLatency: '{model} · {latency} ms',
    responseMeta: '任務 #{jobId} · 執行 #{runId} · {provider}',
    queuedAssistantRequest: '問題已加入佇列，稍後會自動處理。',
    cancelledQueuedRequest: '已取消排隊中的提問。',
    attentionTitle: '助手需要處理',
    loadExamplePrompt: '試試這個範例',
    examplePrompt: '我最近看了哪些關於語義搜尋的文章？',
    examplePromptFocus: '最近一週我最專注的主題是什麼？',
    examplePromptTimeline: '我上次認真研究 SQLite 是什麼時候？',
    emptyEyebrow: '根據你的瀏覽紀錄',
    emptyTitle: '問點什麼吧',
    emptyDescription:
      '助手會根據你的瀏覽紀錄來回答問題，並標註引用來源。試著問一個具體的問題。',
    evidenceLabel: '引用來源 · {count} 筆紀錄',
    checkStatus: '查看狀態',
    runQueuedJob: '立即處理',
    cancel: '取消',
    working: '思考中',
    preparingAnswer: '正在搜尋相關紀錄並產生回答',
    searchingArchive: '正在搜尋歷史紀錄…',
    inputPlaceholder: '問問關於你瀏覽歷史的問題…',
    inputLabel: '你的問題',
    sendAction: '送出',
    auditTraceHint:
      '每個回答都會保留完整的追溯資訊：AI 模型、工作編號、引用來源等。',
    queueBoundary: '佇列說明',
    queueBoundaryBody:
      '排隊中的工作可以取消。但工作一旦開始處理，就無法中途停止，只能等待完成。',
    queueProgressLabel: '{queued} 排隊中 / {running} 處理中',
    loadingQueueAction: '正在載入工作佇列',
    testingProviderAction: '正在測試連線',
    loadingQueuedAnswerAction: '正在載入回答',
    runningQueuedJobsAction: '正在處理工作',
    cancellingAssistantJobAction: '正在取消工作',
    failedResponse: '取得回答失敗',
    paperGreetingTitle: '問你的存檔任何問題。',
    paperGreetingSubtitle:
      '回答都基於你真正瀏覽過的頁面 — 每一條結論都會註明出處。一切都在本機完成。',
    paperPrompt1: '上週我在哪裡看的 Rust 非同步執行時？',
    paperPrompt2: '我上次認真研究 SQLite 是什麼時候？',
    paperPrompt3: '這個月最占用我注意力的話題是什麼？',
    paperComposerPlaceholder: '用自然語言提問 — 助手會註明每一條引用。',
    paperComposerSendLabel: '送出',
    paperComposerAttribution: '本機 · {provider}',
    paperComposerAttributionFallback: '本機 · 關鍵字模式',
    paperComposerKeyHint: '↵ 送出 · ⇧↵ 換行',
    paperEvidenceLabel: '引用來源 · {count} 筆紀錄',
    paperUserByline: '你',
    paperAssistantByline: '本機 · {provider}',
  },
} as const
