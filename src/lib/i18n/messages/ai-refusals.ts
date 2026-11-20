import { defineMessages } from '../define'

/**
 * Why AI work on Agent conversations was refused, by the backend's typed
 * code, and what to do: the semantic search of Records, the index's state
 * in Settings, and the assistant in Ask (`features/ai-setup/ai-refusals.ts`).
 */
export const aiRefusals = defineMessages({
  en: {
    aiDisabled: {
      title: 'AI is off',
      body: 'Turn on AI in Settings › AI.',
    },
    semanticDisabled: {
      title: 'Semantic search is off',
      body: 'Turn on “{semantic}” in Settings › AI.',
    },
    assistantDisabled: {
      title: 'The assistant is off',
      body: 'Turn on the assistant in Settings › AI.',
    },
    semanticAccess: {
      title: 'Semantic search of conversations is off',
      body: 'Turn on “{switch}” in Settings › Agent conversations.',
    },
    assistantAccess: {
      title: 'The assistant can’t read Agent conversations',
      body: 'Turn on “{switch}” in Settings › Agent conversations.',
    },
    kindOff: {
      title: 'Agent conversations are off',
      body: 'Turn them on in Manage data types.',
    },
    providerMissing: {
      title: 'Semantic search needs an embedding model',
      body: 'Choose one in Settings › AI.',
    },
    providerChanged: {
      title: 'The embedding model changed',
      body: 'The index was made with another model or service. Check Settings › Agent conversations, then try again.',
    },
    serviceUnavailable: {
      title: 'The embedding service didn’t answer',
      body: 'Check that it’s running and can be reached, then try again. It’s chosen in Settings › AI.',
    },
    modelUnavailable: {
      title: 'The in-app model isn’t ready',
      body: 'It may not be downloaded, or its files can’t be read. Check it in Settings › AI.',
    },
  },
  'zh-CN': {
    aiDisabled: {
      title: 'AI 关着',
      body: '在“设置 › AI”里打开 AI。',
    },
    semanticDisabled: {
      title: '语义搜索关着',
      body: '在“设置 › AI”里打开“{semantic}”。',
    },
    assistantDisabled: {
      title: '助理关着',
      body: '在“设置 › AI”里打开助理。',
    },
    semanticAccess: {
      title: '对话的语义搜索关着',
      body: '在“设置 › Agent 对话”里打开“{switch}”。',
    },
    assistantAccess: {
      title: '助理不能读 Agent 对话',
      body: '在“设置 › Agent 对话”里打开“{switch}”。',
    },
    kindOff: {
      title: 'Agent 对话关着',
      body: '在“管理数据类型”里打开。',
    },
    providerMissing: {
      title: '语义搜索需要 embedding 模型',
      body: '在“设置 › AI”里选一个。',
    },
    providerChanged: {
      title: 'Embedding 模型换了',
      body: '索引是用别的模型或服务建立的。看一下“设置 › Agent 对话”，再试一次。',
    },
    serviceUnavailable: {
      title: 'Embedding 服务没有回应',
      body: '确认它在运行、连得上，再试一次。它是在“设置 › AI”里选的。',
    },
    modelUnavailable: {
      title: '应用内置模型还不能用',
      body: '可能还没下载，或者文件读不了。到“设置 › AI”里看看。',
    },
  },
  'zh-TW': {
    aiDisabled: {
      title: 'AI 關著',
      body: '在「設定 › AI」裡打開 AI。',
    },
    semanticDisabled: {
      title: '語意搜尋關著',
      body: '在「設定 › AI」裡打開「{semantic}」。',
    },
    assistantDisabled: {
      title: '助理關著',
      body: '在「設定 › AI」裡打開助理。',
    },
    semanticAccess: {
      title: '對話的語意搜尋關著',
      body: '在「設定 › Agent 對話」裡打開「{switch}」。',
    },
    assistantAccess: {
      title: '助理不能讀 Agent 對話',
      body: '在「設定 › Agent 對話」裡打開「{switch}」。',
    },
    kindOff: {
      title: 'Agent 對話關著',
      body: '在「管理資料類型」裡打開。',
    },
    providerMissing: {
      title: '語意搜尋需要 embedding 模型',
      body: '在「設定 › AI」裡選一個。',
    },
    providerChanged: {
      title: 'Embedding 模型換了',
      body: '索引是用別的模型或服務建立的。看一下「設定 › Agent 對話」，再試一次。',
    },
    serviceUnavailable: {
      title: 'Embedding 服務沒有回應',
      body: '確認它在執行、連得上，再試一次。它是在「設定 › AI」裡選的。',
    },
    modelUnavailable: {
      title: 'App 內建模型還不能用',
      body: '可能還沒下載，或者檔案讀不了。到「設定 › AI」裡看看。',
    },
  },
})
