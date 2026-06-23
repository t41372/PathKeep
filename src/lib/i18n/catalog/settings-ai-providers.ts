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
    aiMcpToggle: 'External tool access (MCP)',
    aiMcpToggleHelp:
      'Lets external AI tools you connect (such as Claude Code or Cursor) search your history through a localhost-only server PathKeep runs on demand. They get the same bounded, read-only search the in-app assistant uses — nothing more — and nothing is exposed until you turn this on.',
    aiMcpToggleAudit:
      'Every external query is recorded as an entry in your archive activity log, and the server refuses to run while PathKeep is locked.',
    aiMcpToggleAuditLink: 'Review external-query activity',
    aiMcpToggleConnect:
      'After enabling, open Integrations for the exact command and config to connect a tool.',
    aiMcpToggleConnectLink: 'Open Integrations',
    aiSkillToggle: 'Usage guide for external tools',
    aiSkillToggleHelp:
      'Gives the external AI tools you connect a built-in guide on how to query your history well — which level of detail to ask for, how the search mode is chosen, and how to cite the visits behind an answer. It is guidance only: read-only, and it exposes no history beyond what External tool access already allows.',
    aiSkillToggleDependency:
      'The guide is only reachable when External tool access (MCP) above is also on, since it is served through that same server.',
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
    aiConsentDisclosureCodeMode:
      'To answer a question, the assistant may write and run a small program over your history to search and combine results. These programs are sandboxed and read-only — they cannot reach the network or your files and run under strict time, memory, and output limits. Every answer shows the exact code and the queries it ran.',
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
      'Usage guide is enabled: the MCP server serves a read-only guide teaching connected tools how to query effectively. It exposes no extra data.',
    aiIntegrationCapabilitySkillUnreachable:
      'Usage guide is enabled but unreachable: it is only served while the MCP server above is also on. It exposes no extra data when reachable.',
    aiIntegrationCapabilitySkillDisabled:
      'Usage guide is disabled in saved Settings, so connected tools receive only a short disabled notice instead of the querying guide.',
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
    aiSearchTuningTitle: 'Advanced search tuning',
    aiSearchTuningIntro:
      'Fine-tune how Smart search blends keyword and meaning-based matches when it ranks results. The defaults work well for most people — adjust only if you know what you want, and Save to apply.',
    aiSearchTuningRrfKLabel: 'Rank smoothing (k)',
    aiSearchTuningRrfKHelp:
      'How much a result’s exact position in each list matters when the keyword and meaning lists are merged. A small value rewards the few top hits sharply; a larger value spreads credit more evenly so deeper matches still count. 60 is the standard balance.',
    aiSearchTuningLexicalLabel: 'Keyword match weight',
    aiSearchTuningLexicalHelp:
      'How much exact word matches count toward the final ranking. Raise it to favor pages that literally contain your terms; set it to 0 to rank purely by meaning.',
    aiSearchTuningSemanticLabel: 'Meaning match weight',
    aiSearchTuningSemanticHelp:
      'How much meaning-based (semantic) matches count toward the final ranking. Raise it to favor pages that are about your query even without the exact words; set it to 0 to rank purely by keyword.',
    aiSearchTuningStarredLabel: 'Starred boost',
    aiSearchTuningStarredHelp:
      'A small nudge that lifts pages you’ve starred when they’re relevant. It is deliberately capped at 0.5 so a starred page can rank a bit higher but can never push an unrelated favorite above a strongly matching result — Smart search stays search, not your bookmark list. Set it to 0 to give stars no ranking effect.',
    aiSearchTuningReset: 'Reset to defaults',
    aiSearchTuningResetHint: '60 · 1.0 · 1.0 · 0.15',
    aiGpuTitle: 'GPU acceleration & re-embedding',
    aiGpuIntro:
      'PathKeep’s in-app embedding model runs on the CPU by default. On Apple-Silicon Macs with a Metal-enabled build you can opt in to run it on the GPU, then re-embed your working set or whole archive — much faster, entirely on your machine.',
    aiGpuToggleLabel: 'Use the GPU for in-app embedding',
    aiGpuToggleHelp:
      'When on, the in-app embedding model runs on the Apple-Silicon Metal GPU instead of the CPU. It produces the same results, just faster, so turning it on does not invalidate an existing index — re-embedding is always your explicit choice below.',
    aiGpuUnavailable:
      'GPU acceleration requires a Metal-enabled build. This build runs on the CPU only; your preference is saved and will apply automatically if you switch to a Metal build.',
    aiGpuUnavailableBadge: 'CPU-only build',
    aiGpuAvailableBadge: 'Metal build',
    aiReembedTitle: 'Re-embed',
    aiReembedWorkingSetLabel: 'Re-embed working set',
    aiReembedWorkingSetHelp:
      'Re-embed only your high-value pages (starred, recent, tagged, and frequently revisited). Bounded in scope, but it runs in the background and can take a while on the CPU — see the estimate.',
    aiReembedFullLabel: 'Re-embed full archive',
    aiReembedFullHelp:
      'Re-embed every unique page in your archive from scratch. This is the expensive option — review the estimate before starting.',
    aiReembedFullRequiresGpu:
      'Re-embedding the full archive is available once GPU acceleration is on (and this is a Metal build). On the CPU it would take far too long.',
    aiReembedRequiresSemanticIndex:
      'Turn on Smart search (the semantic index) in AI settings to re-embed. Re-embedding builds the search vectors that Smart search uses.',
    aiReembedEstimateLoading: 'Estimating…',
    aiReembedEstimatePages: '{count} pages',
    aiReembedEstimateCpu: '≈ {minutes} min on CPU',
    aiReembedEstimateGpu: '≈ {minutes} min on GPU',
    aiReembedEstimateGpuUnavailable: 'GPU estimate needs a Metal build',
    aiReembedStart: 'Start',
    aiReembedQueued:
      'Re-embed queued — PathKeep is processing it in the background.',
    aiReembedProgress: 'Re-embedding… {queued} queued, {running} running',
    aiReembedDone: 'Re-embed complete.',
    aiReembedBackground:
      'Re-embed is running in the background — check Jobs for progress.',
    aiReembedError: 'Could not start re-embedding. Please try again.',
    aiReembedEstimateError: 'Could not load the estimate.',
  },
  'zh-CN': {
    aiMasterToggle: '启用 AI 功能',
    aiAssistantToggle: 'AI 助手（对话）',
    aiAssistantToggleHelp: '在助手页面用你配置的对话模型与历史记录对话。',
    aiSemanticToggle: '智能搜索',
    aiSemanticToggleHelp:
      '在浏览器页面使用语义和混合搜索。需要先配置向量模型，并完成一次索引构建（在浏览器 → 智能搜索里运行），结果才会出现。',
    aiMcpToggle: '外部工具访问（MCP）',
    aiMcpToggleHelp:
      '让你连接的外部 AI 工具（如 Claude Code 或 Cursor）通过 PathKeep 按需运行的仅本机服务器搜索你的历史记录。它们获得的是与应用内助手相同的、有界的只读搜索——仅此而已——在你开启之前不会暴露任何内容。',
    aiMcpToggleAudit:
      '每次外部查询都会作为一条记录写入你的归档活动日志，并且在 PathKeep 锁定时服务器会拒绝运行。',
    aiMcpToggleAuditLink: '查看外部查询活动',
    aiMcpToggleConnect:
      '启用后，打开“集成”页面获取连接工具所需的确切命令和配置。',
    aiMcpToggleConnectLink: '打开集成',
    aiSkillToggle: '外部工具使用指南',
    aiSkillToggleHelp:
      '为你连接的外部 AI 工具提供一份内置指南，告诉它们如何更好地查询你的历史记录——该请求哪种粒度、搜索模式如何选定，以及如何引用支撑答案的访问记录。它只是指引：只读，且不会暴露超出“外部工具访问”已允许范围的任何历史记录。',
    aiSkillToggleDependency:
      '该指南只有在上方的“外部工具访问（MCP）”同时开启时才可访问，因为它正是通过那个服务器提供的。',
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
    aiConsentDisclosureCodeMode:
      '为了回答问题，助手可能会编写并运行一个小程序，在你的历史记录上搜索并整合结果。这些程序运行在沙箱中且只读——无法访问网络或你的文件，并受到严格的时间、内存和输出限制。每个回答都会展示它实际运行的代码和查询。',
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
      '使用指南已开启：MCP 服务器会提供一份只读指南，教已连接的工具如何高效查询。它不会暴露任何额外数据。',
    aiIntegrationCapabilitySkillUnreachable:
      '使用指南已开启但无法访问：只有在上方的 MCP 服务器同时开启时才会提供。可访问时它也不会暴露任何额外数据。',
    aiIntegrationCapabilitySkillDisabled:
      '已保存的设置里关闭了使用指南，因此已连接的工具只会收到一条简短的停用提示，而不是查询指南。',
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
    aiSearchTuningTitle: '高级搜索调优',
    aiSearchTuningIntro:
      '微调智能搜索在排序时如何融合关键词匹配和语义匹配。默认值对大多数人都很合适——只有在你清楚自己想要什么时再调整，并点击保存以生效。',
    aiSearchTuningRrfKLabel: '排名平滑（k）',
    aiSearchTuningRrfKHelp:
      '在合并关键词列表和语义列表时，结果在各自列表中的具体名次有多重要。数值越小，越偏向少数靠前的命中；数值越大，分数分布越平均，让排得更靠后的匹配也能计入。60 是标准的平衡值。',
    aiSearchTuningLexicalLabel: '关键词匹配权重',
    aiSearchTuningLexicalHelp:
      '精确词语匹配在最终排序中所占的比重。调高它会更偏向真正包含你输入词语的页面；设为 0 则完全按语义排序。',
    aiSearchTuningSemanticLabel: '语义匹配权重',
    aiSearchTuningSemanticHelp:
      '基于含义（语义）的匹配在最终排序中所占的比重。调高它会更偏向即使没有完全相同词语、但主题相关的页面；设为 0 则完全按关键词排序。',
    aiSearchTuningStarredLabel: '收藏加权',
    aiSearchTuningStarredHelp:
      '当你收藏的页面与查询相关时，给它一点小幅提升。它被特意限制在 0.5 以内，所以收藏页面可以排得略高，却永远无法把一个不相关的收藏挤到强相关结果之上——智能搜索仍然是搜索，而不是你的书签列表。设为 0 则收藏不影响排序。',
    aiSearchTuningReset: '恢复默认值',
    aiSearchTuningResetHint: '60 · 1.0 · 1.0 · 0.15',
    aiGpuTitle: 'GPU 加速与重嵌入',
    aiGpuIntro:
      'PathKeep 的内置嵌入模型默认在 CPU 上运行。在装有 Metal 版本的 Apple 芯片 Mac 上，你可以选择启用 GPU 运行，然后重嵌入工作集或整个存档——更快，且完全在本机进行。',
    aiGpuToggleLabel: '使用 GPU 进行内置嵌入',
    aiGpuToggleHelp:
      '启用后，内置嵌入模型会在 Apple 芯片的 Metal GPU 上运行而非 CPU。结果完全相同，只是更快，因此启用它不会使现有索引失效——重嵌入始终由你在下方明确发起。',
    aiGpuUnavailable:
      'GPU 加速需要 Metal 版本。当前版本仅在 CPU 上运行；你的偏好会被保存，切换到 Metal 版本后将自动生效。',
    aiGpuUnavailableBadge: '仅 CPU 版本',
    aiGpuAvailableBadge: 'Metal 版本',
    aiReembedTitle: '重嵌入',
    aiReembedWorkingSetLabel: '重嵌入工作集',
    aiReembedWorkingSetHelp:
      '仅重嵌入你的高价值页面（已加星、近期、带标签以及频繁重访的页面）。范围有界，但会在后台运行，在 CPU 上可能需要一段时间——请参考预估。',
    aiReembedFullLabel: '重嵌入整个存档',
    aiReembedFullHelp:
      '从头重嵌入存档中的每个唯一页面。这是开销较大的选项——开始前请查看预估。',
    aiReembedFullRequiresGpu:
      '启用 GPU 加速（且为 Metal 版本）后才能重嵌入整个存档。在 CPU 上耗时过长。',
    aiReembedRequiresSemanticIndex:
      '请在 AI 设置中开启智能搜索（语义索引）后再重嵌入。重嵌入会构建智能搜索所用的搜索向量。',
    aiReembedEstimateLoading: '预估中…',
    aiReembedEstimatePages: '{count} 个页面',
    aiReembedEstimateCpu: 'CPU 约 {minutes} 分钟',
    aiReembedEstimateGpu: 'GPU 约 {minutes} 分钟',
    aiReembedEstimateGpuUnavailable: 'GPU 预估需要 Metal 版本',
    aiReembedStart: '开始',
    aiReembedQueued: '重嵌入已加入队列——PathKeep 正在后台处理。',
    aiReembedProgress: '重嵌入中…{queued} 个排队，{running} 个进行中',
    aiReembedDone: '重嵌入完成。',
    aiReembedBackground: '重嵌入正在后台运行——可在「任务」中查看进度。',
    aiReembedError: '无法开始重嵌入，请重试。',
    aiReembedEstimateError: '无法加载预估。',
  },
  'zh-TW': {
    aiMasterToggle: '啟用 AI 功能',
    aiAssistantToggle: 'AI 助手（對話）',
    aiAssistantToggleHelp: '在助手頁面用你設定的對話模型與歷史紀錄對話。',
    aiSemanticToggle: '智慧搜尋',
    aiSemanticToggleHelp:
      '在瀏覽器頁面使用語義和混合搜尋。需要先設定向量模型，並完成一次索引建立（在瀏覽器 → 智慧搜尋裡執行），結果才會出現。',
    aiMcpToggle: '外部工具存取（MCP）',
    aiMcpToggleHelp:
      '讓你連接的外部 AI 工具（如 Claude Code 或 Cursor）透過 PathKeep 按需執行的僅本機伺服器搜尋你的歷史紀錄。它們獲得的是與應用內助手相同的、有界的唯讀搜尋——僅此而已——在你開啟之前不會暴露任何內容。',
    aiMcpToggleAudit:
      '每次外部查詢都會作為一筆紀錄寫入你的封存活動紀錄，並且在 PathKeep 鎖定時伺服器會拒絕執行。',
    aiMcpToggleAuditLink: '檢視外部查詢活動',
    aiMcpToggleConnect:
      '啟用後，開啟「整合」頁面取得連接工具所需的確切指令和設定。',
    aiMcpToggleConnectLink: '開啟整合',
    aiSkillToggle: '外部工具使用指南',
    aiSkillToggleHelp:
      '為你連接的外部 AI 工具提供一份內建指南，告訴它們如何更好地查詢你的歷史紀錄——該請求哪種粒度、搜尋模式如何選定，以及如何引用支撐答案的造訪紀錄。它只是指引：唯讀，且不會暴露超出「外部工具存取」已允許範圍的任何歷史紀錄。',
    aiSkillToggleDependency:
      '該指南只有在上方的「外部工具存取（MCP）」同時開啟時才可存取，因為它正是透過那個伺服器提供的。',
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
    aiConsentDisclosureCodeMode:
      '為了回答問題，助手可能會撰寫並執行一個小程式，在你的歷史紀錄上搜尋並整合結果。這些程式執行於沙箱中且唯讀——無法存取網路或你的檔案，並受到嚴格的時間、記憶體和輸出限制。每個回答都會顯示它實際執行的程式碼和查詢。',
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
      '使用指南已開啟：MCP 伺服器會提供一份唯讀指南，教已連接的工具如何高效查詢。它不會暴露任何額外資料。',
    aiIntegrationCapabilitySkillUnreachable:
      '使用指南已開啟但無法存取：只有在上方的 MCP 伺服器同時開啟時才會提供。可存取時它也不會暴露任何額外資料。',
    aiIntegrationCapabilitySkillDisabled:
      '已儲存的設定關閉了使用指南，因此已連接的工具只會收到一則簡短的停用提示，而不是查詢指南。',
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
    aiSearchTuningTitle: '進階搜尋調校',
    aiSearchTuningIntro:
      '微調智慧搜尋在排序時如何融合關鍵字比對與語意比對。預設值對大多數人都很合適——只有在你清楚自己想要什麼時再調整，並點擊儲存以生效。',
    aiSearchTuningRrfKLabel: '排名平滑（k）',
    aiSearchTuningRrfKHelp:
      '在合併關鍵字清單與語意清單時，結果在各自清單中的確切名次有多重要。數值越小，越偏向少數靠前的命中；數值越大，分數分布越平均，讓排得更後面的比對也能計入。60 是標準的平衡值。',
    aiSearchTuningLexicalLabel: '關鍵字比對權重',
    aiSearchTuningLexicalHelp:
      '精確詞語比對在最終排序中所占的比重。調高它會更偏向真正含有你輸入詞語的頁面；設為 0 則完全依語意排序。',
    aiSearchTuningSemanticLabel: '語意比對權重',
    aiSearchTuningSemanticHelp:
      '基於含意（語意）的比對在最終排序中所占的比重。調高它會更偏向即使沒有完全相同詞語、但主題相關的頁面；設為 0 則完全依關鍵字排序。',
    aiSearchTuningStarredLabel: '收藏加權',
    aiSearchTuningStarredHelp:
      '當你收藏的頁面與查詢相關時，給它一點小幅提升。它被特意限制在 0.5 以內，所以收藏頁面可以排得略高，卻永遠無法把一個不相關的收藏擠到強相關結果之上——智慧搜尋仍然是搜尋，而不是你的書籤清單。設為 0 則收藏不影響排序。',
    aiSearchTuningReset: '還原預設值',
    aiSearchTuningResetHint: '60 · 1.0 · 1.0 · 0.15',
    aiGpuTitle: 'GPU 加速與重新嵌入',
    aiGpuIntro:
      'PathKeep 的內建嵌入模型預設在 CPU 上執行。在裝有 Metal 版本的 Apple 晶片 Mac 上，你可以選擇啟用 GPU 執行，然後重新嵌入工作集或整個封存——更快，且完全在本機進行。',
    aiGpuToggleLabel: '使用 GPU 進行內建嵌入',
    aiGpuToggleHelp:
      '啟用後，內建嵌入模型會在 Apple 晶片的 Metal GPU 上執行而非 CPU。結果完全相同，只是更快，因此啟用它不會使現有索引失效——重新嵌入一律由你在下方明確發起。',
    aiGpuUnavailable:
      'GPU 加速需要 Metal 版本。目前版本僅在 CPU 上執行；你的偏好會被儲存，切換到 Metal 版本後將自動生效。',
    aiGpuUnavailableBadge: '僅 CPU 版本',
    aiGpuAvailableBadge: 'Metal 版本',
    aiReembedTitle: '重新嵌入',
    aiReembedWorkingSetLabel: '重新嵌入工作集',
    aiReembedWorkingSetHelp:
      '僅重新嵌入你的高價值頁面（已加星號、近期、帶標籤以及頻繁重訪的頁面）。範圍有界，但會在背景執行，在 CPU 上可能需要一段時間——請參考預估。',
    aiReembedFullLabel: '重新嵌入整個封存',
    aiReembedFullHelp:
      '從頭重新嵌入封存中的每個唯一頁面。這是開銷較大的選項——開始前請查看預估。',
    aiReembedFullRequiresGpu:
      '啟用 GPU 加速（且為 Metal 版本）後才能重新嵌入整個封存。在 CPU 上耗時過長。',
    aiReembedRequiresSemanticIndex:
      '請在 AI 設定中開啟智慧搜尋（語意索引）後再重新嵌入。重新嵌入會建立智慧搜尋所用的搜尋向量。',
    aiReembedEstimateLoading: '預估中…',
    aiReembedEstimatePages: '{count} 個頁面',
    aiReembedEstimateCpu: 'CPU 約 {minutes} 分鐘',
    aiReembedEstimateGpu: 'GPU 約 {minutes} 分鐘',
    aiReembedEstimateGpuUnavailable: 'GPU 預估需要 Metal 版本',
    aiReembedStart: '開始',
    aiReembedQueued: '重新嵌入已加入佇列——PathKeep 正在背景處理。',
    aiReembedProgress: '重新嵌入中…{queued} 個排隊，{running} 個進行中',
    aiReembedDone: '重新嵌入完成。',
    aiReembedBackground: '重新嵌入正在背景執行——可在「工作」中查看進度。',
    aiReembedError: '無法開始重新嵌入，請重試。',
    aiReembedEstimateError: '無法載入預估。',
  },
} as const
