/**
 * @file settings-ai-providers.ts
 * @description Defines one focused settings translation owner so the overall settings namespace no longer lives in a single mega-file.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide one bounded subsection of the settings namespace for en, zh-CN, and zh-TW.
 * - Keep related settings copy together without reintroducing a second language-specific owner.
 *
 * ## Not responsible for
 * - Other settings copy that belongs to different subgroup owners
 * - Translator creation, flattening, or language resolution
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` imports this static dictionary during catalog assembly.
 *
 * ## Performance notes
 * - Static dictionary data only; keep this file side-effect free so large locale loads stay cheap.
 */

/**
 * Keeps the settings namespace subsection aligned across shipping locales so copy updates stay in one owner.
 */
export const settingsAiProvidersNamespace = {
  en: {
    aiMasterToggle: 'Enable AI features',
    aiAssistantToggle: 'AI assistant (chat)',
    aiAssistantToggleHelp:
      'Lets you chat with your history on the Assistant page using your configured chat provider.',
    aiSemanticToggle: 'Smart search',
    aiSemanticToggleHelp:
      'Semantic and hybrid search in Explorer. Needs an embedding provider configured and a one-time index build (run it from Explorer → smart search) before results appear.',
    aiSubToggleDisabledHint: 'Enable AI features above to turn these on.',
    aiTestConnection: 'Test connection',
    aiTestingConnection: 'Testing…',
    aiProbeReachable: 'Connected',
    aiProbeUnreachable: 'Connection issue',
    aiProbeLatency: '{model} · {latency} ms',
    aiAddProviderPresetLabel: 'Start from a preset',
    aiPresetLmStudio: 'LM Studio',
    aiPresetOllama: 'Ollama',
    aiPresetOpenai: 'OpenAI',
    aiPresetAnthropic: 'Anthropic',
    aiPresetGoogle: 'Google',
    aiConsentDisclosureTitle: 'How AI uses your data',
    aiConsentDisclosureBody:
      'AI is optional and stays off until you turn it on here. Once enabled, the text of the matching history, your search queries, and your chat messages are sent to whichever LLM and embedding provider you configure below — for example a local LM Studio endpoint or your own cloud provider key.',
    aiConsentDisclosureNoProvider:
      'PathKeep ships no AI provider of its own. Nothing leaves your machine until you both enable AI and configure a provider.',
    aiConsentDisclosureEgress:
      'A configured provider only receives what a given request needs (matching history snippets and your prompt). It does not get a copy of your whole archive.',
    aiConsentDisclosureLocal:
      'Vectors and audit traces are stored locally beside your archive, and chat transcripts are excluded from export.',
    aiIntegrationCopyFailed: "Couldn't copy that artifact.",
    aiLlmProviders: 'CHAT PROVIDERS',
    aiEmbeddingProviders: 'EMBEDDING PROVIDERS',
    aiAddLlmProvider: 'Add chat provider',
    aiAddEmbeddingProvider: 'Add embedding provider',
    aiActiveLlmProvider: 'Active chat provider',
    aiActiveEmbeddingProvider: 'Active embedding provider',
    aiNoneSelected: 'None',
    aiSavingConfig: 'Saving…',
    aiSaveConfig: 'Save',
    aiResetDraft: 'Discard changes',
    aiUnsavedChanges: 'You have unsaved changes',
    aiDraftSaved: 'Settings are up to date',
    aiGettingStartedTitle: 'No AI providers configured yet',
    aiGettingStartedBody:
      'Add a chat provider to use the AI assistant and a embedding provider for smart search. Click "Add chat provider" below to get started with a preset.',
    aiDraftBoundaryBody:
      'Changes are saved only when you click Save. API keys are stored separately.',
    aiArtifactsMovedTitle: 'Generated artifacts live in Integrations',
    aiArtifactsMovedBody:
      'Provider settings stay here. MCP commands, skill files, and local-host payload review now live on the Integrations page.',
    aiProviderName: 'Name',
    aiProviderId: 'ID',
    aiRequestFormat: 'API format',
    aiBaseUrl: 'Base URL',
    aiBaseUrlPlaceholder: 'https://api.example.com/v1',
    aiDefaultModel: 'Default model',
    aiModelCatalog: 'Available models',
    aiModelCatalogHint: 'Comma-separated model names',
    aiEnabled: 'Enabled',
    aiTemperature: 'Temperature',
    aiMaxTokens: 'Max tokens',
    aiDimensions: 'Dimensions',
    aiNotes: 'Notes',
    aiApiKey: 'API key',
    aiApiKeyPlaceholder: 'sk-...',
    aiKeySaved: 'Saved',
    aiKeyNotSaved: 'Not saved',
    aiSaveKey: 'Save key',
    aiClearKey: 'Remove key',
    aiRemoveProvider: 'Remove',
    aiRequestFormatOpenai: 'OpenAI-compatible',
    aiRequestFormatAnthropic: 'Anthropic-compatible',
    aiRequestFormatGoogle: 'Google AI Studio',
    aiRequestFormatOllama: 'Ollama',
    aiRequestFormatLmStudio: 'LM Studio',
    aiIndexHealthTitle: 'INDEX HEALTH · {status}',
    aiIndexedRows: 'Indexed rows',
    aiSemanticSidecar: 'Semantic sidecar',
    aiSemanticMetadata: 'SQLite metadata',
    aiEstimatedTokens: 'Estimated tokens',
    aiIndexWarning: 'Current index warning',
    aiIndexWarningEmbeddingMissing:
      'Select an embedding provider in Settings before enabling semantic retrieval.',
    aiIntegrationUnavailable: 'Integration preview unavailable',
    aiIntegrationArtifactsTitle: 'AI integration artifacts',
    aiIntegrationArtifactsSummaryTitle:
      'Review generated files before using them externally',
    aiIntegrationArtifactsSummaryBody:
      'PathKeep can prepare MCP and skill snippets, but it never installs them into external tools automatically. Review the contents and copy only what you trust.',
    aiIntegrationLoadingTitle: 'Preparing integration preview',
    aiIntegrationLoadingBody:
      'Generated files and commands appear here after the local preview finishes loading.',
    aiIntegrationReview: 'EXTERNAL AI INTEGRATIONS',
    aiMcpCommand: 'MCP command',
    aiCapabilityNotes: 'Capability notes',
    aiScopeBoundary: 'Scope boundary',
    aiAuditTrace: 'Audit trace',
    aiGeneratedFiles: 'Generated files',
    aiManualSteps: 'Manual steps',
    aiIntegrationConsentSummary:
      'External AI integrations stay local-first and explicit. PathKeep only exposes localhost MCP tools after you turn on AI + MCP in Settings, and the current app session must stay unlocked.',
    aiIntegrationManualEnable:
      'Enable MCP or Skill integration in Settings first. Both are off by default.',
    aiIntegrationManualStoreKey:
      'Store the database key in the native keyring if the archive is encrypted, so background and MCP lookups can unlock the archive.',
    aiIntegrationManualCopyJson:
      'Copy the generated MCP JSON into your local MCP client configuration and restart that client.',
    aiIntegrationManualCopySkill:
      'Copy the generated skill markdown into your local skills directory if you want a reusable history-research workflow.',
    aiIntegrationCapabilityMcpEnabled:
      'MCP server toggle is currently enabled in saved Settings.',
    aiIntegrationCapabilityMcpDisabled:
      'MCP server toggle is currently disabled in saved Settings.',
    aiIntegrationCapabilitySkillEnabled:
      'Skill integration toggle is currently enabled in saved Settings.',
    aiIntegrationCapabilitySkillDisabled:
      'Skill integration toggle is currently disabled in saved Settings.',
    aiIntegrationCapabilityEmbeddingEnabled:
      'Semantic retrieval can use the configured embedding provider when the semantic index is built.',
    aiIntegrationCapabilityEmbeddingDisabled:
      'No embedding provider is selected right now, so MCP and external assistants fall back to lexical recall only. They still respect archive visibility and App Lock.',
    aiIntegrationScopeVisibleOnly:
      'Queries only see currently visible archive facts. Reverted visits stay hidden even if an old embedding row still exists.',
    aiIntegrationScopeLock:
      'If App Lock re-locks the session, MCP search returns a locked refusal instead of reading the archive behind the UI.',
    aiIntegrationScopeLocalhost:
      'The MCP surface is localhost-only and never publishes the archive to a remote PathKeep service.',
    aiIntegrationAuditMcp:
      'Every MCP request is recorded as a dedicated `mcp_query` run in the unified archive ledger.',
    aiIntegrationAuditAssistant:
      'Assistant answers keep their provider snapshot, retrieval provider, and citations inside `ai_assistant_runs`.',
    aiIntegrationAuditDerivedPath:
      'Derived AI state lives beside the archive at {path} and can be cleared/rebuilt without touching canonical visits.',
    aiIntegrationWarningDisabled:
      'MCP and skill integration are both disabled in Settings right now.',
    aiIntegrationGeneratedFileMcpPurpose:
      'Local MCP client configuration snippet for PathKeep.',
    aiIntegrationGeneratedFileSkillPurpose:
      'Codex skill starter that teaches an external assistant how to query PathKeep through MCP.',
  },
  'zh-CN': {
    aiMasterToggle: '启用 AI 功能',
    aiAssistantToggle: 'AI 助手（对话）',
    aiAssistantToggleHelp: '在助手页面用你配置的对话模型与历史记录对话。',
    aiSemanticToggle: '智能搜索',
    aiSemanticToggleHelp:
      '在浏览器页面使用语义和混合搜索。需要先配置向量模型，并完成一次索引构建（在浏览器 → 智能搜索里运行），结果才会出现。',
    aiSubToggleDisabledHint: '请先在上方启用 AI 功能，才能开启这些选项。',
    aiTestConnection: '测试连接',
    aiTestingConnection: '测试中…',
    aiProbeReachable: '连接正常',
    aiProbeUnreachable: '连接异常',
    aiProbeLatency: '{model} · {latency} ms',
    aiAddProviderPresetLabel: '从预设开始',
    aiPresetLmStudio: 'LM Studio',
    aiPresetOllama: 'Ollama',
    aiPresetOpenai: 'OpenAI',
    aiPresetAnthropic: 'Anthropic',
    aiPresetGoogle: 'Google',
    aiConsentDisclosureTitle: 'AI 如何使用你的数据',
    aiConsentDisclosureBody:
      'AI 是可选功能，在你于此处开启之前一直保持关闭。开启后，匹配到的历史记录文本、你的搜索查询和聊天消息会发送给你在下方配置的 LLM 与向量模型——例如本地 LM Studio 端点，或你自己的云服务密钥。',
    aiConsentDisclosureNoProvider:
      'PathKeep 自身不附带任何 AI 服务。只有当你同时启用 AI 并配置好服务后，数据才会离开你的设备。',
    aiConsentDisclosureEgress:
      '已配置的服务只会收到单次请求所需的内容（匹配到的历史片段和你的提问），不会拿到你整个存档的副本。',
    aiConsentDisclosureLocal:
      '向量和审计记录保存在存档旁的本地，聊天记录也不会包含在导出里。',
    aiIntegrationCopyFailed: '复制该产物失败。',
    aiLlmProviders: '对话模型',
    aiEmbeddingProviders: '向量模型',
    aiAddLlmProvider: '添加对话模型',
    aiAddEmbeddingProvider: '添加向量模型',
    aiActiveLlmProvider: '当前对话模型',
    aiActiveEmbeddingProvider: '当前向量模型',
    aiNoneSelected: '未选择',
    aiSavingConfig: '保存中…',
    aiSaveConfig: '保存',
    aiResetDraft: '放弃更改',
    aiUnsavedChanges: '有未保存的更改',
    aiDraftSaved: '设置已是最新',
    aiGettingStartedTitle: '还没有配置 AI 服务',
    aiGettingStartedBody:
      '添加对话模型可以使用 AI 助手，添加向量模型可以使用智能搜索。点击下方「添加对话模型」按钮，选择一个预设开始。',
    aiDraftBoundaryBody: '更改只在点击保存后生效。API 密钥需要单独保存。',
    aiArtifactsMovedTitle: '生成产物已移到集成页',
    aiArtifactsMovedBody:
      '这里保留服务设置。MCP 命令、skill 文件和本地宿主载荷复核现在由集成页负责。',
    aiProviderName: '名称',
    aiProviderId: 'ID',
    aiRequestFormat: 'API 格式',
    aiBaseUrl: 'Base URL',
    aiBaseUrlPlaceholder: 'https://api.example.com/v1',
    aiDefaultModel: '默认模型',
    aiModelCatalog: '可用模型',
    aiModelCatalogHint: '用逗号分隔模型名',
    aiEnabled: '已启用',
    aiTemperature: '温度',
    aiMaxTokens: '最大 token 数',
    aiDimensions: '维度',
    aiNotes: '备注',
    aiApiKey: 'API 密钥',
    aiApiKeyPlaceholder: 'sk-...',
    aiKeySaved: '已保存',
    aiKeyNotSaved: '未保存',
    aiSaveKey: '保存密钥',
    aiClearKey: '删除密钥',
    aiRemoveProvider: '移除',
    aiRequestFormatOpenai: 'OpenAI 兼容格式',
    aiRequestFormatAnthropic: 'Anthropic 兼容格式',
    aiRequestFormatGoogle: 'Google AI Studio',
    aiRequestFormatOllama: 'Ollama',
    aiRequestFormatLmStudio: 'LM Studio',
    aiIndexHealthTitle: '索引状态 · {status}',
    aiIndexedRows: '已索引记录',
    aiSemanticSidecar: '语义侧车',
    aiSemanticMetadata: 'SQLite 元数据',
    aiEstimatedTokens: '预估 tokens',
    aiIndexWarning: '当前索引警告',
    aiIndexWarningEmbeddingMissing:
      '请先在设置里选择向量模型，再启用语义检索。',
    aiIntegrationUnavailable: '集成预览不可用',
    aiIntegrationArtifactsTitle: 'AI 集成产物',
    aiIntegrationArtifactsSummaryTitle: '使用前先检查生成文件',
    aiIntegrationArtifactsSummaryBody:
      'PathKeep 可以准备 MCP 和 skill 片段，但不会自动安装到外部工具。请先检查内容，再只复制你信任的部分。',
    aiIntegrationLoadingTitle: '正在准备集成预览',
    aiIntegrationLoadingBody: '本地预览加载完成后，这里会显示生成文件和命令。',
    aiIntegrationReview: '外部 AI 集成',
    aiMcpCommand: 'MCP 命令',
    aiCapabilityNotes: '能力说明',
    aiScopeBoundary: '边界说明',
    aiAuditTrace: '审计记录',
    aiGeneratedFiles: '生成文件',
    aiManualSteps: '手动步骤',
    aiIntegrationConsentSummary:
      '外部 AI 集成保持本地优先且必须显式开启。只有你在设置里打开 AI 与 MCP 后，PathKeep 才会暴露本机 MCP 工具，而且当前应用会话必须保持已解锁。',
    aiIntegrationManualEnable:
      '先在设置里开启 MCP 或 Skill 集成，两者默认都关闭。',
    aiIntegrationManualStoreKey:
      '如果存档已加密，请把数据库密钥存进系统钥匙串，这样后台任务和 MCP 查询才能解锁存档。',
    aiIntegrationManualCopyJson:
      '把生成好的 MCP JSON 复制到本地 MCP client 配置里，然后重启那个 client。',
    aiIntegrationManualCopySkill:
      '如果你想要可复用的历史研究工作流，再把生成的 skill markdown 复制到本地 skills 目录。',
    aiIntegrationCapabilityMcpEnabled:
      '已保存的设置里目前开启了 MCP server toggle。',
    aiIntegrationCapabilityMcpDisabled:
      '已保存的设置里目前关闭了 MCP server toggle。',
    aiIntegrationCapabilitySkillEnabled:
      '已保存的设置里目前开启了 Skill integration toggle。',
    aiIntegrationCapabilitySkillDisabled:
      '已保存的设置里目前关闭了 Skill integration toggle。',
    aiIntegrationCapabilityEmbeddingEnabled:
      '建立语义索引后，语义检索会使用当前已配置的 embedding provider。',
    aiIntegrationCapabilityEmbeddingDisabled:
      '目前还没有选择向量模型，所以 MCP 和外部助手会回退到词法召回，但仍然遵守存档可见性和 App Lock。',
    aiIntegrationScopeVisibleOnly:
      '查询只能看到当前仍然可见的存档事实。即使旧向量索引行还在，已回滚的访问记录也会保持隐藏。',
    aiIntegrationScopeLock:
      '如果 App Lock 再次锁住会话，MCP 搜索会返回锁定拒绝，而不是绕过 UI 直接读取存档。',
    aiIntegrationScopeLocalhost:
      'MCP 只在本机可用，不会把存档发布到远程 PathKeep 服务。',
    aiIntegrationAuditMcp:
      '每一次 MCP 请求都会在统一存档账本里记录成独立的 `mcp_query` 运行。',
    aiIntegrationAuditAssistant:
      '助手回答会把 provider snapshot、retrieval provider 和 citations 一起保存在 `ai_assistant_runs` 里。',
    aiIntegrationAuditDerivedPath:
      '派生 AI 状态保存在存档旁边的 {path}，可以单独清理或重建，而不会碰规范访问记录。',
    aiIntegrationWarningDisabled:
      '当前设置里 MCP 和 skill integration 都处于关闭状态。',
    aiIntegrationGeneratedFileMcpPurpose:
      '供 PathKeep 使用的本地 MCP client 配置片段。',
    aiIntegrationGeneratedFileSkillPurpose:
      '教外部助手如何通过 MCP 查询 PathKeep 的 Codex skill 起始模板。',
  },
  'zh-TW': {
    aiMasterToggle: '啟用 AI 功能',
    aiAssistantToggle: 'AI 助手（對話）',
    aiAssistantToggleHelp: '在助手頁面用你設定的對話模型與歷史紀錄對話。',
    aiSemanticToggle: '智慧搜尋',
    aiSemanticToggleHelp:
      '在瀏覽器頁面使用語義和混合搜尋。需要先設定向量模型，並完成一次索引建立（在瀏覽器 → 智慧搜尋裡執行），結果才會出現。',
    aiSubToggleDisabledHint: '請先在上方啟用 AI 功能，才能開啟這些選項。',
    aiTestConnection: '測試連線',
    aiTestingConnection: '測試中…',
    aiProbeReachable: '連線正常',
    aiProbeUnreachable: '連線異常',
    aiProbeLatency: '{model} · {latency} ms',
    aiAddProviderPresetLabel: '從預設開始',
    aiPresetLmStudio: 'LM Studio',
    aiPresetOllama: 'Ollama',
    aiPresetOpenai: 'OpenAI',
    aiPresetAnthropic: 'Anthropic',
    aiPresetGoogle: 'Google',
    aiConsentDisclosureTitle: 'AI 如何使用你的資料',
    aiConsentDisclosureBody:
      'AI 是選用功能，在你於此處開啟之前都保持關閉。開啟後，比對到的歷史紀錄文字、你的搜尋查詢和聊天訊息會傳送給你在下方設定的 LLM 與向量模型——例如本機 LM Studio 端點，或你自己的雲端服務金鑰。',
    aiConsentDisclosureNoProvider:
      'PathKeep 本身不附帶任何 AI 服務。只有當你同時啟用 AI 並設定好服務後，資料才會離開你的裝置。',
    aiConsentDisclosureEgress:
      '已設定的服務只會收到單次請求所需的內容（比對到的歷史片段和你的提問），不會拿到你整個封存的副本。',
    aiConsentDisclosureLocal:
      '向量和稽核記錄保存在封存旁的本機，聊天紀錄也不會包含在匯出裡。',
    aiIntegrationCopyFailed: '複製該產物失敗。',
    aiLlmProviders: '對話模型',
    aiEmbeddingProviders: '向量模型',
    aiAddLlmProvider: '新增對話模型',
    aiAddEmbeddingProvider: '新增向量模型',
    aiActiveLlmProvider: '目前對話模型',
    aiActiveEmbeddingProvider: '目前向量模型',
    aiNoneSelected: '未選擇',
    aiSavingConfig: '儲存中…',
    aiSaveConfig: '儲存',
    aiResetDraft: '捨棄變更',
    aiUnsavedChanges: '有未儲存的變更',
    aiDraftSaved: '設定已是最新',
    aiGettingStartedTitle: '還沒有設定 AI 服務',
    aiGettingStartedBody:
      '新增對話模型可以使用 AI 助手，新增向量模型可以使用智慧搜尋。點選下方「新增對話模型」按鈕，選擇一個預設開始。',
    aiDraftBoundaryBody: '變更只在點擊儲存後生效。API 金鑰需要另外儲存。',
    aiArtifactsMovedTitle: '生成產物已移到整合頁',
    aiArtifactsMovedBody:
      '這裡保留服務設定。MCP 命令、skill 檔案和本地宿主載荷複核現在由整合頁負責。',
    aiProviderName: '名稱',
    aiProviderId: 'ID',
    aiRequestFormat: 'API 格式',
    aiBaseUrl: 'Base URL',
    aiBaseUrlPlaceholder: 'https://api.example.com/v1',
    aiDefaultModel: '預設模型',
    aiModelCatalog: '可用模型',
    aiModelCatalogHint: '以逗號分隔模型名稱',
    aiEnabled: '已啟用',
    aiTemperature: '溫度',
    aiMaxTokens: '最大 token 數',
    aiDimensions: '維度',
    aiNotes: '備註',
    aiApiKey: 'API 金鑰',
    aiApiKeyPlaceholder: 'sk-...',
    aiKeySaved: '已儲存',
    aiKeyNotSaved: '未儲存',
    aiSaveKey: '儲存金鑰',
    aiClearKey: '移除金鑰',
    aiRemoveProvider: '移除',
    aiRequestFormatOpenai: 'OpenAI 相容格式',
    aiRequestFormatAnthropic: 'Anthropic 相容格式',
    aiRequestFormatGoogle: 'Google AI Studio',
    aiRequestFormatOllama: 'Ollama',
    aiRequestFormatLmStudio: 'LM Studio',
    aiIndexHealthTitle: '索引狀態 · {status}',
    aiIndexedRows: '已索引記錄',
    aiSemanticSidecar: '語意側車',
    aiSemanticMetadata: 'SQLite 中繼資料',
    aiEstimatedTokens: '預估 tokens',
    aiIndexWarning: '目前索引警告',
    aiIndexWarningEmbeddingMissing:
      '請先在設定裡選擇向量模型，再啟用語義檢索。',
    aiIntegrationUnavailable: '整合預覽無法使用',
    aiIntegrationArtifactsTitle: 'AI 整合產物',
    aiIntegrationArtifactsSummaryTitle: '使用前先檢查生成檔案',
    aiIntegrationArtifactsSummaryBody:
      'PathKeep 可以準備 MCP 和 skill 片段，但不會自動安裝到外部工具。請先檢查內容，再只複製你信任的部分。',
    aiIntegrationLoadingTitle: '正在準備整合預覽',
    aiIntegrationLoadingBody: '本地預覽載入完成後，這裡會顯示生成檔案和命令。',
    aiIntegrationReview: '外部 AI 整合',
    aiMcpCommand: 'MCP 指令',
    aiCapabilityNotes: '能力說明',
    aiScopeBoundary: '邊界說明',
    aiAuditTrace: '稽核記錄',
    aiGeneratedFiles: '產生的檔案',
    aiManualSteps: '手動步驟',
    aiIntegrationConsentSummary:
      '外部 AI 整合維持本地優先且必須明確開啟。只有你在設定裡打開 AI 與 MCP 後，PathKeep 才會提供本機 MCP 工具，而且目前的應用工作階段必須保持已解鎖。',
    aiIntegrationManualEnable:
      '先在設定裡開啟 MCP 或 Skill integration，兩者預設都關閉。',
    aiIntegrationManualStoreKey:
      '如果封存已加密，請把資料庫金鑰存進系統鑰匙圈，這樣背景工作和 MCP 查詢才能解鎖封存。',
    aiIntegrationManualCopyJson:
      '把產生好的 MCP JSON 複製到本機 MCP client 設定裡，然後重新啟動那個 client。',
    aiIntegrationManualCopySkill:
      '如果你想要可重用的歷史研究 workflow，再把產生的 skill markdown 複製到本機 skills 目錄。',
    aiIntegrationCapabilityMcpEnabled:
      '已儲存的設定目前已開啟 MCP server toggle。',
    aiIntegrationCapabilityMcpDisabled:
      '已儲存的設定目前已關閉 MCP server toggle。',
    aiIntegrationCapabilitySkillEnabled:
      '已儲存的設定目前已開啟 Skill integration toggle。',
    aiIntegrationCapabilitySkillDisabled:
      '已儲存的設定目前已關閉 Skill integration toggle。',
    aiIntegrationCapabilityEmbeddingEnabled:
      '建立語義索引後，語義檢索會使用目前已設定的 embedding provider。',
    aiIntegrationCapabilityEmbeddingDisabled:
      '目前尚未選擇向量模型，所以 MCP 和外部助手會回退到詞彙召回，但仍然遵守封存可見性與 App Lock。',
    aiIntegrationScopeVisibleOnly:
      '查詢只能看到目前仍可見的封存事實。即使舊的向量索引列仍存在，已回滾的造訪紀錄也會維持隱藏。',
    aiIntegrationScopeLock:
      '如果 App Lock 再次鎖住工作階段，MCP 搜尋會回傳鎖定拒絕，而不是繞過 UI 直接讀取封存。',
    aiIntegrationScopeLocalhost:
      'MCP 只在本機可用，不會把封存發布到遠端 PathKeep 服務。',
    aiIntegrationAuditMcp:
      '每一次 MCP 請求都會在統一封存帳本裡記成獨立的 `mcp_query` 執行。',
    aiIntegrationAuditAssistant:
      '助手回答會把 provider snapshot、retrieval provider 與 citations 一起保存在 `ai_assistant_runs` 裡。',
    aiIntegrationAuditDerivedPath:
      '衍生 AI 狀態保存在封存旁邊的 {path}，可以單獨清除或重建，而不會碰規範造訪紀錄。',
    aiIntegrationWarningDisabled:
      '目前設定裡 MCP 和 skill integration 都處於關閉狀態。',
    aiIntegrationGeneratedFileMcpPurpose:
      '供 PathKeep 使用的本機 MCP client 設定片段。',
    aiIntegrationGeneratedFileSkillPurpose:
      '教外部助手如何透過 MCP 查詢 PathKeep 的 Codex skill 起始模板。',
  },
} as const
