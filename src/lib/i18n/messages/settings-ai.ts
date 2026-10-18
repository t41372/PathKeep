import { defineMessages } from '../define'

/** Settings → AI, plus the provider and semantic-search pieces onboarding reuses. */
export const settingsAi = defineMessages({
  en: {
    title: 'AI',
    optionalNote:
      'Everything here is optional. PathKeep backs up, searches and analyzes your history without any of it.',
    semantic: {
      title: 'Semantic search',
      descriptionService:
        'Find pages by meaning, not only by keywords. {name} builds the index.',
      model: {
        label: 'Model',
        inApp: 'In-app static model',
        inAppAbout:
          '{model}, {dims} dimensions. It comes with PathKeep and runs on this computer. It is a static embedding model: a word means the same to it wherever it appears, so it cannot tell apple the fruit from Apple the company. That makes it very fast, but it finds pages by meaning less well than the usual contextual embedding models, which read every word in its context. To use one, add an embeddings service such as LM Studio or Ollama.',
        add: 'Add an embeddings service…',
      },
      serviceDialog: {
        title: 'Add an embeddings service',
        description:
          'A service that turns text into embeddings, OpenAI-compatible. To build the index, semantic search sends it each page’s title, site and the words in its address, with the page’s saved text when there is some, and each different search you have made; to find pages, your searches.',
      },
      serviceLocalNote: 'Runs on this computer. Pages and searches stay here.',
      serviceRemoteNote:
        'An online service. Each page’s title, site, the words in its address and its saved text, and your searches, are sent to it.',
      serviceRemoveBody:
        'The service and its stored API key are removed from this computer. Semantic search goes back to the in-app static model.',
      serviceRemoveBodyIndexed:
        'The service and its stored API key are removed from this computer. The index made with it can’t be used without it and is deleted. Semantic search goes back to the in-app static model, which has to build its own index first.',
      switch: {
        title: 'Switch semantic search to {name}?',
        confirm: 'Switch model',
        inAppName: 'the in-app static model',
        onSave: 'Saving switches semantic search to this model:',
        ready:
          'The index made with {model} is complete, so semantic search uses it right away.',
        newIndex:
          '{model} needs its own index: {count} visits, read in the background once you choose Build index. Nothing starts before that.',
        newIndexCounting:
          '{model} needs its own index, read from your history in the background once you choose Build index. Nothing starts before that.',
        newIndexLater:
          '{model} needs its own index: {count} visits, read in the background once semantic search is on.',
        newIndexLaterCounting:
          '{model} needs its own index, read from your history in the background once semantic search is on.',
        replaces:
          'Once it is complete, the index made with {old} is deleted. Going back to {old} means building that index again.',
        dropsUnfinished_one:
          'The unfinished index made with {model} ({count} visit) is not kept: it is deleted the next time an index is built or updated.',
        dropsUnfinished_other:
          'The unfinished index made with {model} ({count} visits) is not kept: it is deleted the next time an index is built or updated.',
        service:
          'From then on every semantic search goes to {name}. When it isn’t running or can’t be reached, semantic search doesn’t work and no other model stands in; keyword search still works.',
        static:
          'It is a static embedding model: very fast, but it finds pages by meaning less well than the usual contextual embedding models.',
        download: 'Building its index first downloads it once, about {size}.',
      },
      modelChange:
        'Another model needs its own index ({count} visits). Nothing starts until you build it in Settings → AI; until it is complete, semantic search keeps using {old}.',
      modelChangeCounting:
        'Another model needs its own index. Nothing starts until you build it in Settings → AI; until it is complete, semantic search keeps using {old}.',
      modelChangeLater:
        'Another model needs its own index ({count} visits). You can build it once semantic search is on.',
      modelChangeLaterCounting:
        'Another model needs its own index. You can build it once semantic search is on.',
      description:
        'Find pages by meaning, not only by keywords. The model runs on this computer. It downloads once, about {size}.',
      offNote: 'Turning it off keeps the downloaded model.',
      downloading: 'Downloading the language model…',
      downloadProgress: '{done} of {total}',
      cancel: 'Cancel',
      downloadFailed: 'The download did not finish. {message}',
      modelMissing: 'The model is not downloaded yet.',
      download: 'Download',
      retry: 'Try again',
      indexed_one: '{count} visit indexed',
      indexed_other: '{count} visits indexed',
      lastIndexed: 'Last indexed {when}',
      nothingIndexed: 'Nothing indexed yet.',
      indexing: 'Building the index…',
      indexProgress: '{done} of {total} visits',
      speed: '{rate} a second',
      etaSoon: 'under a minute left',
      etaMinutes_one: 'about {count} minute left',
      etaMinutes_other: 'about {count} minutes left',
      etaHours: 'about {hours} h {minutes} min left',
      indexWaiting: 'Waiting for other background work to finish…',
      needsBuild:
        '{model} has no index yet. Building it reads {count} visits in the background, and you can pause it at any time.',
      needsBuildCounting:
        '{model} has no index yet. Building it reads your history in the background, and you can pause it at any time.',
      unfinished_one:
        'The index made with {model} is not finished: {count} visit so far.',
      unfinished_other:
        'The index made with {model} is not finished: {count} visits so far.',
      meanwhilePrevious:
        'Until it is complete, semantic search keeps using {model}.',
      meanwhilePartial:
        'Until it is complete, searching by meaning only finds visits already indexed.',
      olderText:
        'This index was made from an older version of the page text, which also carried visit times and leftover page code. It still works; rebuilt, it finds pages by meaning more reliably.',
      olderTextUnfinished:
        'The visits indexed so far were read the older way. Start over to read every visit the current way, or continue and keep them.',
      rebuildIndex: 'Rebuild index',
      startOver: 'Start over',
      build: 'Build index',
      continue: 'Continue',
      pause: 'Pause',
      cancelBuild: 'Cancel',
      inBackground: 'Background work',
      pausedByUser: 'Paused at {done} of {total} visits.',
      pausedByService:
        '{name} could not run the model, so building the index is paused. It said: {message}',
      pausedByQueue:
        'All background work is paused. Building the index goes on when it resumes.',
      actionFailed: 'Could not do that. {message}',
      autoUpdate: 'Index new visits after each backup',
      autoUpdateNote: 'Starts once the index is complete.',
      indexFailed: 'Indexing failed. Rebuild the search index to try again.',
      indexIncomplete: 'The index is incomplete. Rebuild it to fix this.',
      failed: 'Could not change this setting. {message}',
    },
    provider: {
      title: 'AI service',
      description: 'The model the assistant talks to.',
      select: 'AI service',
      none: 'None',
      add: 'Add provider…',
      localNote: 'Runs on this computer. Nothing is sent anywhere.',
      remoteNote:
        'An online service. When you ask a question, the pages the assistant finds are sent to it.',
      keyMissing: 'No API key is stored for this service yet.',
      checking: 'Checking {name}…',
      checkOk:
        '{name} answers and has {model}. Test connection runs the model once.',
      checkNoModel:
        '{name} answers, but has no model called {model}. Check the model name.',
      checkFailed: 'Could not read {name}’s models: {message}',
      test: 'Test connection',
      testing: 'Testing…',
      testOk: 'Connected to {model} in {ms} ms.',
      testFailed: 'Could not connect. {message}',
      testErrors: {
        'secret-missing':
          'The service did not accept the API key. Check the key and test again.',
        'network-error':
          'Could not reach {address}. Check the address and that the service is running.',
        'rate-limited':
          'The service is limiting requests right now. Try again in a while.',
        'bad-model':
          'The service has no model called {model}. Check the model name.',
        'bad-endpoint':
          '{address} reaches a server, but not its API. For LM Studio and Ollama the address usually ends in /v1, like http://localhost:1234/v1.',
        'unsupported-capability': 'This service cannot be used for this.',
        'provider-disabled': 'This service is turned off.',
        other: 'The service answered with an error: {message}',
      },
      remove: 'Remove',
      removeTitle: 'Remove {name}?',
      removeBody:
        'The service and its stored API key are removed from this computer. Your conversations are kept.',
      removeFailed: 'Could not remove the service.',
      saved: '{name} is ready.',
      updated: '{name} is saved.',
      edit: 'Edit',
      editDialog: {
        title: 'Edit {name}',
        description: 'Change how PathKeep reaches this service.',
        submit: 'Save',
      },
      saveFailed: 'Could not save the service. {message}',
      selectFailed: 'Could not change the service. {message}',
      dialog: {
        title: 'Add an AI service',
        description: 'Choose where the assistant gets its answers from.',
        submit: 'Add service',
        cancel: 'Cancel',
        saving: 'Saving…',
      },
      form: {
        kind: 'Service',
        name: 'Name',
        namePlaceholder: 'Optional',
        baseUrl: 'Address',
        model: 'Model',
        modelPlaceholder: 'For example {example}',
        apiKey: 'API key',
        apiKeyHint:
          'Kept in your system keychain, never in PathKeep’s settings file.',
        apiKeyOptional:
          'Optional: only for a server on this computer that asks for one. It is kept in the system keychain.',
        apiKeyKeep: 'Leave it empty to keep the saved key.',
        optionalLater:
          'You can finish setup, including the API key and a connection test, later in Settings → AI.',
      },
      models: {
        browse: 'Browse the list',
        search: 'Find a model',
        empty: 'No model matches.',
        loading: 'Reading the service’s models…',
        found_one: '{count} model on this service. Pick it, or type a name.',
        found_other:
          '{count} models on this service. Pick one, or type a name.',
        none: 'The service has no models yet. Download or load one there, or type a name.',
        noneFor: {
          llm: 'The service has no chat models, only embedding models. Type a name if it has one.',
          embedding:
            'The service has no embedding models. In LM Studio and Ollama their names usually contain “embed”.',
        },
        failed:
          'Could not read the service’s models: {message} You can still type a name.',
        loaded: 'Loaded',
        context: 'reads {size}',
        upTo: 'up to {size}',
      },
      kinds: {
        ollama: 'Ollama (this computer)',
        'lm-studio': 'LM Studio (this computer)',
        openai: 'OpenAI or compatible',
        anthropic: 'Anthropic',
        gemini: 'Google Gemini',
      },
      defaultName: {
        ollama: 'Ollama',
        'lm-studio': 'LM Studio',
        openai: 'OpenAI',
        anthropic: 'Anthropic',
        gemini: 'Gemini',
      },
    },
    mcp: {
      title: 'MCP server',
      description: 'Let tools like Claude read your archive. Read-only.',
      how: 'PathKeep does not keep a server running. Your tool starts one when it needs it, using the command below. This switch only allows that.',
      details: 'Connection details',
      copy: 'Copy',
      copied: 'Copied',
      loadFailed: 'Could not load the connection details.',
      failed: 'Could not change this setting. {message}',
    },
    mcpLog: {
      title: 'What other apps read',
      description:
        'Every request an app made through the MCP server: what it asked and how much came back.',
      label: 'Requests through the MCP server',
      empty: 'No app has asked anything yet.',
      loadFailed: 'Could not load the requests: {message}',
      loadingMore: 'Loading earlier requests…',
      allShown: 'That’s every request.',
      moreFailed: 'Couldn’t load earlier requests: {message}',
      tool: {
        'search-history': 'Searched your history',
        'archive-status': 'Checked the archive’s status',
        'usage-guide': 'Read how to use PathKeep',
        other: 'Used {tool}',
      },
      sent_one: 'Sent {count} page of {matches} matching',
      sent_other: 'Sent {count} pages of {matches} matching',
      matching_one: '{count} matching page',
      matching_other: '{count} matching pages',
      onSite: 'on {site}',
      asked: '“{query}”',
    },
    rebuild: {
      title: 'Rebuild search index',
      description:
        'Reads your whole history again and rebuilds the semantic index. You can keep using PathKeep meanwhile.',
      action: 'Rebuild',
      needsSemantic: 'Turn on local semantic search first.',
      dialogTitle: 'Rebuild the search index?',
      dialogBody_one:
        'This indexes {count} visit again, in the background: you can keep using PathKeep, and its progress shows here. Your history is not changed.',
      dialogBody_other:
        'This indexes {count} visits again, in the background: you can keep using PathKeep, and its progress shows here. Your history is not changed.',
      dialogBodyUnknown:
        'This indexes your whole history again. It can take a while. Your history is not changed.',
      confirm: 'Rebuild',
      started: 'Rebuilding the search index.',
      failed: 'Could not start the rebuild. {message}',
    },
    tuning: {
      title: 'Search ranking',
      description:
        'How searching by meaning in History orders results. It blends two lists, pages whose words match and pages whose meaning matches, then lifts starred pages a little.',
      needsSemantic:
        'Used only when local semantic search is on. You can still set it now.',
      k: {
        label: 'Rank smoothing',
        help: 'How much the first few results of each list count over the rest. Lower lets the very top matches win; higher blends both lists more evenly.',
      },
      lexical: {
        label: 'Word matches',
        help: 'How much pages containing your exact words count. 0 ignores them.',
      },
      semantic: {
        label: 'Meaning matches',
        help: 'How much pages about the same idea count, even without your words. 0 ignores them.',
      },
      starred: {
        label: 'Starred boost',
        help: 'How far starred pages move up. Kept small so a starred page never beats a much better match.',
      },
      range: 'Default {value} · from {min} to {max}',
      invalid: 'Enter a number from {min} to {max}.',
      save: 'Save',
      reset: 'Reset to defaults',
      saved: 'Search ranking saved.',
    },
  },
  'zh-CN': {
    title: 'AI',
    optionalNote:
      '这里的一切都是可选的。没有这些，PathKeep 也能备份、搜索和分析你的历史。',
    semantic: {
      title: '语义搜索',
      descriptionService:
        '按含义而不只是关键词来查找页面。由 {name} 建立索引。',
      model: {
        label: '模型',
        inApp: '应用内置静态模型',
        inAppAbout:
          '{model}，{dims} 维。随 PathKeep 提供，在这台电脑上运行。它是静态嵌入（static embedding）模型：同一个词不管出现在哪里，对它来说意思都一样，所以分不清“苹果”是水果还是公司。因此它非常快，但找意思相近的页面不如一般的上下文嵌入（contextual embedding）模型，那种模型会看每个词的上下文。想用的话，添加一个嵌入服务，例如 LM Studio 或 Ollama。',
        add: '添加嵌入服务…',
      },
      serviceDialog: {
        title: '添加嵌入服务',
        description:
          '把文字转成嵌入向量的服务，需兼容 OpenAI。为了建立索引，语义搜索会把每个页面的标题、网站、网址里的词，以及保存的网页正文（如果有）发给它，也会发送你搜索过的每一组关键词；搜索时也会把你的搜索发给它。',
      },
      serviceLocalNote: '在这台电脑上运行，页面和搜索都留在这里。',
      serviceRemoteNote:
        '在线服务。每个页面的标题、网站、网址里的词和保存的网页正文，以及你的搜索，都会发给它。',
      serviceRemoveBody:
        '这个服务和它保存的 API 密钥会从这台电脑上移除，语义搜索改回使用应用内置静态模型。',
      serviceRemoveBodyIndexed:
        '这个服务和它保存的 API 密钥会从这台电脑上移除。用它建立的索引少了它就用不了，会被删除。语义搜索改回使用应用内置静态模型，要先建立它自己的索引。',
      switch: {
        title: '把语义搜索换成 {name}？',
        confirm: '换模型',
        inAppName: '应用内置静态模型',
        onSave: '保存后，语义搜索就换成这个模型：',
        ready: '用 {model} 建立的索引已经完整，语义搜索会直接用它。',
        newIndex:
          '{model} 需要自己的索引：在后台读取 {count} 条访问记录，等你点“建立索引”才开始，在此之前什么都不会做。',
        newIndexCounting:
          '{model} 需要自己的索引：在后台读取你的历史，等你点“建立索引”才开始，在此之前什么都不会做。',
        newIndexLater:
          '{model} 需要自己的索引：开启语义搜索后在后台读取 {count} 条访问记录。',
        newIndexLaterCounting:
          '{model} 需要自己的索引：开启语义搜索后在后台读取你的历史。',
        replaces:
          '新索引完成后，用 {old} 建立的索引会被删除。之后想换回 {old}，就得重新建立它的索引。',
        dropsUnfinished_one:
          '用 {model} 建立、还没完成的索引（{count} 条访问记录）不会保留：下次建立或更新索引时就会删除。',
        dropsUnfinished_other:
          '用 {model} 建立、还没完成的索引（{count} 条访问记录）不会保留：下次建立或更新索引时就会删除。',
        service:
          '之后每次语义搜索都要经过 {name}。它没开或连不上时，语义搜索就用不了，也没有别的模型顶替；关键词搜索照常可用。',
        static:
          '它是静态嵌入模型：非常快，但找意思相近的页面不如一般的上下文嵌入模型。',
        download: '建立索引前要先下载一次，约 {size}。',
      },
      modelChange:
        '换成别的模型需要它自己的索引（{count} 条访问记录）。要在“设置 → AI”里建立才会开始；建好之前，语义搜索继续用 {old}。',
      modelChangeCounting:
        '换成别的模型需要它自己的索引。要在“设置 → AI”里建立才会开始；建好之前，语义搜索继续用 {old}。',
      modelChangeLater:
        '换成别的模型需要它自己的索引（{count} 条访问记录）。打开语义搜索后就能建立。',
      modelChangeLaterCounting:
        '换成别的模型需要它自己的索引。打开语义搜索后就能建立。',
      description:
        '按含义而不只是关键词来查找页面。模型在这台电脑上运行，只需下载一次，约 {size}。',
      offNote: '关闭后会保留已下载的模型。',
      downloading: '正在下载语言模型…',
      downloadProgress: '{done} / {total}',
      cancel: '取消',
      downloadFailed: '下载没有完成。{message}',
      modelMissing: '模型还没有下载。',
      download: '下载',
      retry: '重试',
      indexed_one: '已索引 {count} 条访问记录',
      indexed_other: '已索引 {count} 条访问记录',
      lastIndexed: '上次索引：{when}',
      nothingIndexed: '还没有索引任何内容。',
      indexing: '正在建立索引…',
      indexProgress: '{done} / {total} 条访问记录',
      speed: '每秒 {rate} 条',
      etaSoon: '剩不到一分钟',
      etaMinutes_one: '大约还要 {count} 分钟',
      etaMinutes_other: '大约还要 {count} 分钟',
      etaHours: '大约还要 {hours} 小时 {minutes} 分钟',
      indexWaiting: '正在等其他后台任务完成…',
      needsBuild:
        '{model} 还没有索引。建立索引会在后台读取 {count} 条访问记录，随时可以暂停。',
      needsBuildCounting:
        '{model} 还没有索引。建立索引会在后台读取你的历史，随时可以暂停。',
      unfinished_one:
        '用 {model} 建立的索引还没完成：目前 {count} 条访问记录。',
      unfinished_other:
        '用 {model} 建立的索引还没完成：目前 {count} 条访问记录。',
      meanwhilePrevious: '完成之前，语义搜索继续用 {model}。',
      meanwhilePartial: '完成之前，按含义搜索只找得到已经建立索引的访问记录。',
      olderText:
        '这份索引是用旧版的页面文字建立的，里面还夹着访问时间和网页代码残片。它照样能用；重建之后，按含义搜索会找得更准。',
      olderTextUnfinished:
        '目前已经建立索引的访问记录是用旧方式读的。可以从头重来，用现在的方式读每一条访问记录；也可以继续，保留已经建好的部分。',
      rebuildIndex: '重建索引',
      startOver: '从头重来',
      build: '建立索引',
      continue: '继续',
      pause: '暂停',
      cancelBuild: '取消',
      inBackground: '后台任务',
      pausedByUser: '已暂停：{done} / {total} 条访问记录。',
      pausedByService:
        '{name} 没能运行这个模型，建立索引已暂停。它的回复：{message}',
      pausedByQueue: '所有后台任务都已暂停，恢复后会继续建立索引。',
      actionFailed: '没能完成这个操作。{message}',
      autoUpdate: '每次备份后为新的访问记录建立索引',
      autoUpdateNote: '索引完成后才会开始。',
      indexFailed: '建立索引失败。请重建搜索索引再试一次。',
      indexIncomplete: '索引不完整。请重建索引来修复。',
      failed: '无法更改这项设置。{message}',
    },
    provider: {
      title: 'AI 服务',
      description: '助理使用的模型。',
      select: 'AI 服务',
      none: '无',
      add: '添加服务…',
      localNote: '在这台电脑上运行，不会向任何地方发送内容。',
      remoteNote: '这是在线服务。你提问时，助理找到的页面会发送给它。',
      keyMissing: '还没有为这个服务保存 API 密钥。',
      checking: '正在检查 {name}…',
      checkOk: '{name} 有回应，也有 {model}。“测试连接”会实际运行一次模型。',
      checkNoModel: '{name} 有回应，但没有叫 {model} 的模型。请检查模型名称。',
      checkFailed: '没能读取 {name} 的模型：{message}',
      test: '测试连接',
      testing: '测试中…',
      testOk: '已连接到 {model}，耗时 {ms} 毫秒。',
      testFailed: '无法连接。{message}',
      testErrors: {
        'secret-missing': '服务不接受这个 API 密钥。检查密钥后再测试一次。',
        'network-error': '连不上 {address}。检查地址，以及服务是否在运行。',
        'rate-limited': '服务暂时限制了请求，稍后再试。',
        'bad-model': '服务上没有名为 {model} 的模型。检查模型名称。',
        'bad-endpoint':
          '{address} 有服务在运行，但不是它的 API 地址。LM Studio 和 Ollama 的地址通常以 /v1 结尾，例如 http://localhost:1234/v1。',
        'unsupported-capability': '这个服务不能用在这里。',
        'provider-disabled': '这个服务已关闭。',
        other: '服务返回了错误：{message}',
      },
      remove: '移除',
      removeTitle: '移除 {name}？',
      removeBody:
        '这个服务和已保存的 API 密钥会从这台电脑上移除。你的对话会保留。',
      removeFailed: '无法移除这个服务。',
      saved: '{name} 已就绪。',
      updated: '{name} 已保存。',
      edit: '编辑',
      editDialog: {
        title: '编辑 {name}',
        description: '更改 PathKeep 连接这个服务的方式。',
        submit: '保存',
      },
      saveFailed: '无法保存这个服务。{message}',
      selectFailed: '无法更换服务。{message}',
      dialog: {
        title: '添加 AI 服务',
        description: '选择助理从哪里获得回答。',
        submit: '添加服务',
        cancel: '取消',
        saving: '正在保存…',
      },
      form: {
        kind: '服务',
        name: '名称',
        namePlaceholder: '可选',
        baseUrl: '地址',
        model: '模型',
        modelPlaceholder: '例如 {example}',
        apiKey: 'API 密钥',
        apiKeyHint: '保存在系统钥匙串里，不会写进 PathKeep 的设置文件。',
        apiKeyOptional:
          '可不填：只有这台电脑上要求密钥的服务器才需要。密钥保存在系统钥匙串里。',
        apiKeyKeep: '留空就保留已保存的密钥。',
        optionalLater:
          '之后可以在「设置 → AI」里完成设置，包括 API 密钥和连接测试。',
      },
      models: {
        browse: '浏览列表',
        search: '查找模型',
        empty: '没有相符的模型。',
        loading: '正在读取服务上的模型…',
        found_one: '服务上有 {count} 个模型。选一个，或输入名称。',
        found_other: '服务上有 {count} 个模型。选一个，或输入名称。',
        none: '服务上还没有模型。先在那里下载或加载一个，或输入名称。',
        noneFor: {
          llm: '服务上只有 embedding 模型，没有聊天模型。如果有，请输入名称。',
          embedding:
            '服务上没有 embedding 模型。LM Studio 和 Ollama 的 embedding 模型名称通常带有“embed”。',
        },
        failed: '读不到服务上的模型：{message} 仍然可以输入名称。',
        loaded: '已加载',
        context: '可读 {size}',
        upTo: '最多 {size}',
      },
      kinds: {
        ollama: 'Ollama（本机）',
        'lm-studio': 'LM Studio（本机）',
        openai: 'OpenAI 或兼容服务',
        anthropic: 'Anthropic',
        gemini: 'Google Gemini',
      },
      defaultName: {
        ollama: 'Ollama',
        'lm-studio': 'LM Studio',
        openai: 'OpenAI',
        anthropic: 'Anthropic',
        gemini: 'Gemini',
      },
    },
    mcp: {
      title: 'MCP 服务器',
      description: '让 Claude 等工具读取你的存档，只读。',
      how: 'PathKeep 不会一直运行服务器。需要时，你的工具会用下面的命令自行启动。这个开关只是允许它这样做。',
      details: '连接信息',
      copy: '复制',
      copied: '已复制',
      loadFailed: '无法加载连接信息。',
      failed: '无法更改这项设置。{message}',
    },
    mcpLog: {
      title: '其他应用读了什么',
      description: '应用通过 MCP 服务器发出的每个请求：问了什么、拿到多少。',
      label: '通过 MCP 服务器的请求',
      empty: '还没有应用来问过。',
      loadFailed: '无法载入请求：{message}',
      loadingMore: '正在载入更早的请求…',
      allShown: '所有请求都在这里了。',
      moreFailed: '载入更早的请求失败：{message}',
      tool: {
        'search-history': '搜索了你的历史',
        'archive-status': '查看了存档状态',
        'usage-guide': '读了 PathKeep 的用法',
        other: '使用了 {tool}',
      },
      sent_one: '交出 {count} 页（符合 {matches} 页）',
      sent_other: '交出 {count} 页（符合 {matches} 页）',
      matching_one: '符合 {count} 页',
      matching_other: '符合 {count} 页',
      onSite: '限 {site}',
      asked: '“{query}”',
    },
    rebuild: {
      title: '重建搜索索引',
      description:
        '重新读取你的全部历史并重建语义索引。期间可以照常使用 PathKeep。',
      action: '重建',
      needsSemantic: '请先开启本地语义搜索。',
      dialogTitle: '要重建搜索索引吗？',
      dialogBody_one:
        '这会在后台重新索引 {count} 条访问记录，期间可以照常使用 PathKeep，进度显示在这里。你的历史不会被更改。',
      dialogBody_other:
        '这会在后台重新索引 {count} 条访问记录，期间可以照常使用 PathKeep，进度显示在这里。你的历史不会被更改。',
      dialogBodyUnknown:
        '这会重新索引你的全部历史，可能要花一些时间。你的历史不会被更改。',
      confirm: '重建',
      started: '正在重建搜索索引。',
      failed: '无法开始重建。{message}',
    },
    tuning: {
      title: '搜索排序',
      description:
        '在“历史”中按意思搜索时，结果如何排序。它会合并两份列表：文字相符的网页和意思相近的网页，再把加星的网页稍微往前排。',
      needsSemantic: '只在开启本机语意搜索时使用。现在也可以先设置。',
      k: {
        label: '排名平滑',
        help: '每份列表前几名比其余结果重要多少。数值越小，最前面的结果越占优势；越大，两份列表混合得越平均。',
      },
      lexical: {
        label: '文字相符',
        help: '包含你输入文字的网页占多少分量。0 表示不计入。',
      },
      semantic: {
        label: '意思相近',
        help: '主题相同但不一定含有你输入文字的网页占多少分量。0 表示不计入。',
      },
      starred: {
        label: '加星网页加分',
        help: '加星的网页往前排多少。数值保持得很小，加星的网页不会压过明显更相关的结果。',
      },
      range: '默认 {value} · 范围 {min} 到 {max}',
      invalid: '请输入 {min} 到 {max} 之间的数字。',
      save: '保存',
      reset: '恢复默认值',
      saved: '搜索排序已保存。',
    },
  },
  'zh-TW': {
    title: 'AI',
    optionalNote:
      '這裡的一切都是選用的。沒有這些，PathKeep 也能備份、搜尋和分析你的歷史。',
    semantic: {
      title: '語意搜尋',
      descriptionService:
        '依意思而不只是關鍵字來尋找頁面。由 {name} 建立索引。',
      model: {
        label: '模型',
        inApp: 'App 內建靜態模型',
        inAppAbout:
          '{model}，{dims} 維。隨 PathKeep 提供，在這台電腦上執行。它是靜態嵌入（static embedding）模型：同一個字詞不論出現在哪裡，對它來說意思都一樣，所以分不出「蘋果」是水果還是公司。因此它非常快，但找意思相近的頁面不如一般的上下文嵌入（contextual embedding）模型，那種模型會看每個字詞的前後文。想用的話，新增一個嵌入服務，例如 LM Studio 或 Ollama。',
        add: '新增嵌入服務…',
      },
      serviceDialog: {
        title: '新增嵌入服務',
        description:
          '把文字轉成嵌入向量的服務，需相容 OpenAI。為了建立索引，語意搜尋會把每個頁面的標題、網站、網址裡的字詞，以及儲存的網頁內文（如果有）傳給它，也會傳送你搜尋過的每一組關鍵字；搜尋時也會把你的搜尋傳給它。',
      },
      serviceLocalNote: '在這台電腦上執行，頁面與搜尋都留在這裡。',
      serviceRemoteNote:
        '線上服務。每個頁面的標題、網站、網址裡的字詞與儲存的網頁內文，以及你的搜尋，都會傳給它。',
      serviceRemoveBody:
        '這個服務與它儲存的 API 金鑰會從這台電腦上移除，語意搜尋改回使用 App 內建靜態模型。',
      serviceRemoveBodyIndexed:
        '這個服務與它儲存的 API 金鑰會從這台電腦上移除。用它建立的索引少了它就不能用，會被刪除。語意搜尋改回使用 App 內建靜態模型，要先建好它自己的索引。',
      switch: {
        title: '把語意搜尋換成 {name}？',
        confirm: '換模型',
        inAppName: 'App 內建靜態模型',
        onSave: '儲存後，語意搜尋就換成這個模型：',
        ready: '用 {model} 建立的索引已經完整，語意搜尋會直接用它。',
        newIndex:
          '{model} 要有自己的索引：在背景讀取 {count} 次造訪，等你按「建立索引」才開始，在那之前什麼都不會做。',
        newIndexCounting:
          '{model} 要有自己的索引：在背景讀取你的歷史，等你按「建立索引」才開始，在那之前什麼都不會做。',
        newIndexLater:
          '{model} 要有自己的索引：開啟語意搜尋後在背景讀取 {count} 次造訪。',
        newIndexLaterCounting:
          '{model} 要有自己的索引：開啟語意搜尋後在背景讀取你的歷史。',
        replaces:
          '新索引完成後，用 {old} 建立的索引會被刪除。之後想換回 {old}，就得重新建它的索引。',
        dropsUnfinished_one:
          '用 {model} 建立、還沒完成的索引（{count} 次造訪）不會保留：下次建立或更新索引時就會刪除。',
        dropsUnfinished_other:
          '用 {model} 建立、還沒完成的索引（{count} 次造訪）不會保留：下次建立或更新索引時就會刪除。',
        service:
          '之後每次語意搜尋都要經過 {name}。它沒開或連不上時，語意搜尋就不能用，也沒有別的模型頂替；關鍵字搜尋照常可用。',
        static:
          '它是靜態嵌入模型：非常快，但找意思相近的頁面不如一般的上下文嵌入模型。',
        download: '建索引前要先下載一次，約 {size}。',
      },
      modelChange:
        '換成別的模型需要它自己的索引（{count} 次造訪）。要在「設定 → AI」裡建立才會開始；建好之前，語意搜尋繼續用 {old}。',
      modelChangeCounting:
        '換成別的模型需要它自己的索引。要在「設定 → AI」裡建立才會開始；建好之前，語意搜尋繼續用 {old}。',
      modelChangeLater:
        '換成別的模型需要它自己的索引（{count} 次造訪）。開啟語意搜尋後就能建立。',
      modelChangeLaterCounting:
        '換成別的模型需要它自己的索引。開啟語意搜尋後就能建立。',
      description:
        '依含義而不只是關鍵字來尋找頁面。模型在這台電腦上執行，只需下載一次，約 {size}。',
      offNote: '關閉後會保留已下載的模型。',
      downloading: '正在下載語言模型…',
      downloadProgress: '{done} / {total}',
      cancel: '取消',
      downloadFailed: '下載沒有完成。{message}',
      modelMissing: '模型還沒有下載。',
      download: '下載',
      retry: '重試',
      indexed_one: '已建立 {count} 次造訪的索引',
      indexed_other: '已建立 {count} 次造訪的索引',
      lastIndexed: '上次建立索引：{when}',
      nothingIndexed: '還沒有建立任何索引。',
      indexing: '正在建立索引…',
      indexProgress: '{done} / {total} 次造訪',
      speed: '每秒 {rate} 次',
      etaSoon: '剩不到一分鐘',
      etaMinutes_one: '大約還要 {count} 分鐘',
      etaMinutes_other: '大約還要 {count} 分鐘',
      etaHours: '大約還要 {hours} 小時 {minutes} 分鐘',
      indexWaiting: '正在等其他背景工作完成…',
      needsBuild:
        '{model} 還沒有索引。建立索引會在背景讀取 {count} 次造訪，隨時可以暫停。',
      needsBuildCounting:
        '{model} 還沒有索引。建立索引會在背景讀取你的歷史，隨時可以暫停。',
      unfinished_one: '用 {model} 建立的索引還沒完成：目前 {count} 次造訪。',
      unfinished_other: '用 {model} 建立的索引還沒完成：目前 {count} 次造訪。',
      meanwhilePrevious: '完成之前，語意搜尋繼續用 {model}。',
      meanwhilePartial: '完成之前，依意思搜尋只找得到已經建立索引的造訪。',
      olderText:
        '這份索引是用舊版的頁面文字建立的，裡面還夾著造訪時間和網頁程式碼殘片。它照樣能用；重建之後，依意思搜尋會找得更準。',
      olderTextUnfinished:
        '目前已經建立索引的造訪是用舊方式讀的。可以從頭重來，用現在的方式讀每一次造訪；也可以繼續，保留已經建好的部分。',
      rebuildIndex: '重建索引',
      startOver: '從頭重來',
      build: '建立索引',
      continue: '繼續',
      pause: '暫停',
      cancelBuild: '取消',
      inBackground: '背景工作',
      pausedByUser: '已暫停：{done} / {total} 次造訪。',
      pausedByService:
        '{name} 沒能執行這個模型，建立索引已暫停。它的回覆：{message}',
      pausedByQueue: '所有背景工作都已暫停，恢復後會繼續建立索引。',
      actionFailed: '沒能完成這個操作。{message}',
      autoUpdate: '每次備份後為新的造訪建立索引',
      autoUpdateNote: '索引完成後才會開始。',
      indexFailed: '建立索引失敗。請重建搜尋索引再試一次。',
      indexIncomplete: '索引不完整。請重建索引來修復。',
      failed: '無法變更這項設定。{message}',
    },
    provider: {
      title: 'AI 服務',
      description: '助理使用的模型。',
      select: 'AI 服務',
      none: '無',
      add: '新增服務…',
      localNote: '在這台電腦上執行，不會向任何地方傳送內容。',
      remoteNote: '這是線上服務。你提問時，助理找到的頁面會傳送給它。',
      keyMissing: '還沒有為這個服務儲存 API 金鑰。',
      checking: '正在檢查 {name}…',
      checkOk: '{name} 有回應，也有 {model}。「測試連線」會實際執行一次模型。',
      checkNoModel: '{name} 有回應，但沒有叫 {model} 的模型。請檢查模型名稱。',
      checkFailed: '沒能讀取 {name} 的模型：{message}',
      test: '測試連線',
      testing: '測試中…',
      testOk: '已連線到 {model}，耗時 {ms} 毫秒。',
      testFailed: '無法連線。{message}',
      testErrors: {
        'secret-missing': '服務不接受這個 API 金鑰。檢查金鑰後再測試一次。',
        'network-error': '連不上 {address}。檢查位址，以及服務是否在執行。',
        'rate-limited': '服務暫時限制了請求，稍後再試。',
        'bad-model': '服務上沒有叫 {model} 的模型。檢查模型名稱。',
        'bad-endpoint':
          '{address} 有服務在執行，但不是它的 API 位址。LM Studio 和 Ollama 的位址通常以 /v1 結尾，例如 http://localhost:1234/v1。',
        'unsupported-capability': '這個服務不能用在這裡。',
        'provider-disabled': '這個服務已關閉。',
        other: '服務回傳了錯誤：{message}',
      },
      remove: '移除',
      removeTitle: '要移除 {name} 嗎？',
      removeBody:
        '這個服務和已儲存的 API 金鑰會從這台電腦移除。你的對話會保留。',
      removeFailed: '無法移除這個服務。',
      saved: '{name} 已就緒。',
      updated: '{name} 已儲存。',
      edit: '編輯',
      editDialog: {
        title: '編輯 {name}',
        description: '變更 PathKeep 連線到這個服務的方式。',
        submit: '儲存',
      },
      saveFailed: '無法儲存這個服務。{message}',
      selectFailed: '無法更換服務。{message}',
      dialog: {
        title: '新增 AI 服務',
        description: '選擇助理要從哪裡取得回答。',
        submit: '新增服務',
        cancel: '取消',
        saving: '正在儲存…',
      },
      form: {
        kind: '服務',
        name: '名稱',
        namePlaceholder: '選填',
        baseUrl: '位址',
        model: '模型',
        modelPlaceholder: '例如 {example}',
        apiKey: 'API 金鑰',
        apiKeyHint: '儲存在系統鑰匙圈裡，不會寫進 PathKeep 的設定檔。',
        apiKeyOptional:
          '可不填：只有這台電腦上要求金鑰的伺服器才需要。金鑰儲存在系統鑰匙圈裡。',
        apiKeyKeep: '留空就保留已儲存的金鑰。',
        optionalLater:
          '之後可以在「設定 → AI」裡完成設定，包括 API 金鑰和連線測試。',
      },
      models: {
        browse: '瀏覽清單',
        search: '尋找模型',
        empty: '沒有相符的模型。',
        loading: '正在讀取服務上的模型…',
        found_one: '服務上有 {count} 個模型。選一個，或輸入名稱。',
        found_other: '服務上有 {count} 個模型。選一個，或輸入名稱。',
        none: '服務上還沒有模型。先在那裡下載或載入一個，或輸入名稱。',
        noneFor: {
          llm: '服務上只有 embedding 模型，沒有聊天模型。如果有，請輸入名稱。',
          embedding:
            '服務上沒有 embedding 模型。LM Studio 和 Ollama 的 embedding 模型名稱通常帶有「embed」。',
        },
        failed: '讀不到服務上的模型：{message} 仍然可以輸入名稱。',
        loaded: '已載入',
        context: '可讀 {size}',
        upTo: '最多 {size}',
      },
      kinds: {
        ollama: 'Ollama（本機）',
        'lm-studio': 'LM Studio（本機）',
        openai: 'OpenAI 或相容服務',
        anthropic: 'Anthropic',
        gemini: 'Google Gemini',
      },
      defaultName: {
        ollama: 'Ollama',
        'lm-studio': 'LM Studio',
        openai: 'OpenAI',
        anthropic: 'Anthropic',
        gemini: 'Gemini',
      },
    },
    mcp: {
      title: 'MCP 伺服器',
      description: '讓 Claude 等工具讀取你的存檔，唯讀。',
      how: 'PathKeep 不會一直執行伺服器。需要時，你的工具會用下面的指令自行啟動。這個開關只是允許它這樣做。',
      details: '連線資訊',
      copy: '複製',
      copied: '已複製',
      loadFailed: '無法載入連線資訊。',
      failed: '無法變更這項設定。{message}',
    },
    mcpLog: {
      title: '其他 App 讀了什麼',
      description: 'App 透過 MCP 伺服器發出的每個請求：問了什麼、拿到多少。',
      label: '透過 MCP 伺服器的請求',
      empty: '還沒有 App 來問過。',
      loadFailed: '無法載入請求：{message}',
      loadingMore: '正在載入更早的請求…',
      allShown: '所有請求都在這裡了。',
      moreFailed: '載入更早的請求失敗：{message}',
      tool: {
        'search-history': '搜尋了你的歷史',
        'archive-status': '查看了存檔狀態',
        'usage-guide': '讀了 PathKeep 的用法',
        other: '使用了 {tool}',
      },
      sent_one: '交出 {count} 頁（符合 {matches} 頁）',
      sent_other: '交出 {count} 頁（符合 {matches} 頁）',
      matching_one: '符合 {count} 頁',
      matching_other: '符合 {count} 頁',
      onSite: '限 {site}',
      asked: '「{query}」',
    },
    rebuild: {
      title: '重建搜尋索引',
      description:
        '重新讀取你的全部歷史並重建語意索引。期間可以照常使用 PathKeep。',
      action: '重建',
      needsSemantic: '請先開啟本機語意搜尋。',
      dialogTitle: '要重建搜尋索引嗎？',
      dialogBody_one:
        '這會在背景重新索引 {count} 次造訪，期間可以照常使用 PathKeep，進度顯示在這裡。你的歷史不會被變更。',
      dialogBody_other:
        '這會在背景重新索引 {count} 次造訪，期間可以照常使用 PathKeep，進度顯示在這裡。你的歷史不會被變更。',
      dialogBodyUnknown:
        '這會重新索引你的全部歷史，可能要花一些時間。你的歷史不會被變更。',
      confirm: '重建',
      started: '正在重建搜尋索引。',
      failed: '無法開始重建。{message}',
    },
    tuning: {
      title: '搜尋排序',
      description:
        '在「歷史」中依意思搜尋時，結果如何排序。它會合併兩份清單：文字相符的網頁和意思相近的網頁，再把加星的網頁稍微往前排。',
      needsSemantic: '只在開啟本機語意搜尋時使用。現在也可以先設定。',
      k: {
        label: '排名平滑',
        help: '每份清單前幾名比其餘結果重要多少。數值越小，最前面的結果越占優勢；越大，兩份清單混合得越平均。',
      },
      lexical: {
        label: '文字相符',
        help: '包含你輸入文字的網頁占多少分量。0 表示不計入。',
      },
      semantic: {
        label: '意思相近',
        help: '主題相同但不一定含有你輸入文字的網頁占多少分量。0 表示不計入。',
      },
      starred: {
        label: '加星網頁加分',
        help: '加星的網頁往前排多少。數值保持得很小，加星的網頁不會壓過明顯更相關的結果。',
      },
      range: '預設 {value} · 範圍 {min} 到 {max}',
      invalid: '請輸入 {min} 到 {max} 之間的數字。',
      save: '儲存',
      reset: '恢復預設值',
      saved: '搜尋排序已儲存。',
    },
  },
})
