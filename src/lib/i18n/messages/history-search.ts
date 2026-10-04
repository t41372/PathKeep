/** History's search aids: the syntax cheat sheet, regex dialect errors and semantic status. */
import { defineMessages } from '../define'

export const historySearch = defineMessages({
  en: {
    help: {
      button: 'Search tips',
      title: 'Search tips',
      intro:
        'Mix these with ordinary words. Pick one to try it. A minus in front (-site:, -tag:) leaves those pages out.',
      site: 'Only pages on one site',
      exclude: 'Leave out pages with a word',
      phrase: 'These words together, in this order',
      or: 'Either word',
      intitle: 'A word in the page title',
      inurl: 'A word in the address',
      filetype: 'Addresses ending in a file type',
      dates: 'Visited between two dates (last month here)',
      tag: 'Pages you tagged',
      note: 'Pages whose note contains a word',
      regex: 'Slashes around it make it a regex',
      footer:
        'Regex follows Rust’s syntax: no look-ahead, look-behind or back-references. Everything is searched on this computer.',
    },
    regex: {
      syntax: 'Check the brackets, parentheses and backslashes.',
      lookAround:
        'Look-ahead and look-behind, like (?=…) or (?<!…), aren’t supported. To leave pages out, use full-text search with -word.',
      backreference:
        'Back-references, like \\1 or \\k<name>, aren’t supported.',
      group:
        'This kind of group isn’t supported. Use (…), (?:…) or (?<name>…).',
      escape:
        '{escape} isn’t an escape PathKeep knows. Use \\d, \\w, \\s or \\b, or put a backslash before punctuation.',
      brace:
        '{ starts a repeat count like {2} or {2,5}. Write \\{ for the character itself.',
      backendTitle: 'This regex can’t run',
      backendBody:
        'PathKeep uses Rust’s regex syntax, which refused this pattern: {message}',
    },
    semantic: {
      builtIn: 'the built-in model',
      builtInModel: 'the built-in model ({model})',
      using: 'Semantic search with {provider}',
      indexed_one: '{count} page indexed',
      indexed_other: '{count} pages indexed',
      updated: 'updated {when}',
      stale: 'pages added since then aren’t in it yet',
      rebuild: 'Rebuild in Settings → AI',
      fellBack: 'These are full-text results instead.',
      openSettings: 'Settings → AI',
      whereToFix: 'See Settings → AI.',
      reason: {
        off: 'Semantic search is off.',
        building: 'The semantic index is still being built.',
        queued: 'The semantic index is waiting to be built.',
        paused: 'Building the semantic index is paused.',
        empty: 'The semantic index is empty.',
        failed: 'Building the semantic index failed.',
        degraded:
          'The semantic index can’t be used right now: its model isn’t available or it holds no vectors.',
        blocked: 'Semantic search needs the archive to be set up first.',
      },
    },
  },
  'zh-CN': {
    help: {
      button: '搜索技巧',
      title: '搜索技巧',
      intro:
        '可以和普通关键词混用，点一条试试。前面加减号（-site:、-tag:）就是排除这些页面。',
      site: '只看某个网站的页面',
      exclude: '排除含有某个词的页面',
      phrase: '这几个词连在一起、按这个顺序',
      or: '任一个词',
      intitle: '标题里有这个词',
      inurl: '网址里有这个词',
      filetype: '网址以某种文件类型结尾',
      dates: '在两个日期之间浏览过（这里是上个月）',
      tag: '你加过这个标签的页面',
      note: '备注里有这个词的页面',
      regex: '前后加斜线就是正则表达式',
      footer:
        '正则使用 Rust 的语法：不支持前瞻、后顾和反向引用。所有搜索都在这台电脑上进行。',
    },
    regex: {
      syntax: '检查一下方括号、圆括号和反斜线。',
      lookAround:
        '不支持前瞻和后顾，例如 (?=…) 或 (?<!…)。想排除页面，可以用全文搜索加上 -关键词。',
      backreference: '不支持反向引用，例如 \\1 或 \\k<name>。',
      group: '不支持这种分组。可以用 (…)、(?:…) 或 (?<name>…)。',
      escape:
        'PathKeep 不认识 {escape} 这个转义。可以用 \\d、\\w、\\s、\\b，或在标点前加反斜线。',
      brace: '{ 表示重复次数，例如 {2} 或 {2,5}。要匹配这个字符本身请写 \\{。',
      backendTitle: '这个正则表达式无法执行',
      backendBody: 'PathKeep 使用 Rust 的正则语法，它拒绝了这个模式：{message}',
    },
    semantic: {
      builtIn: '内置模型',
      builtInModel: '内置模型（{model}）',
      using: '语义搜索使用{provider}',
      indexed_one: '已索引 {count} 个页面',
      indexed_other: '已索引 {count} 个页面',
      updated: '{when}更新',
      stale: '之后新增的页面还没有加进去',
      rebuild: '到“设置 → AI”重建',
      fellBack: '下面是全文搜索的结果。',
      openSettings: '设置 → AI',
      whereToFix: '请看“设置 → AI”。',
      reason: {
        off: '语义搜索没有开启。',
        building: '语义索引还在建立中。',
        queued: '语义索引正在等待建立。',
        paused: '语义索引的建立已暂停。',
        empty: '语义索引是空的。',
        failed: '语义索引建立失败。',
        degraded: '语义索引现在无法使用：模型不可用，或索引里没有向量。',
        blocked: '要先完成存档设置，才能使用语义搜索。',
      },
    },
  },
  'zh-TW': {
    help: {
      button: '搜尋技巧',
      title: '搜尋技巧',
      intro:
        '可以和一般關鍵字混用，點一條試試。前面加減號（-site:、-tag:）就是排除這些頁面。',
      site: '只看某個網站的頁面',
      exclude: '排除含有某個詞的頁面',
      phrase: '這幾個詞連在一起、按這個順序',
      or: '任一個詞',
      intitle: '標題裡有這個詞',
      inurl: '網址裡有這個詞',
      filetype: '網址以某種檔案類型結尾',
      dates: '在兩個日期之間瀏覽過（這裡是上個月）',
      tag: '你加過這個標籤的頁面',
      note: '備註裡有這個詞的頁面',
      regex: '前後加斜線就是正規表示式',
      footer:
        '正規表示式使用 Rust 的語法：不支援前瞻、後顧和反向參照。所有搜尋都在這台電腦上進行。',
    },
    regex: {
      syntax: '檢查一下方括號、圓括號和反斜線。',
      lookAround:
        '不支援前瞻和後顧，例如 (?=…) 或 (?<!…)。想排除頁面，可以用全文搜尋加上 -關鍵字。',
      backreference: '不支援反向參照，例如 \\1 或 \\k<name>。',
      group: '不支援這種群組。可以用 (…)、(?:…) 或 (?<name>…)。',
      escape:
        'PathKeep 不認得 {escape} 這個跳脫字元。可以用 \\d、\\w、\\s、\\b，或在標點前加反斜線。',
      brace: '{ 表示重複次數，例如 {2} 或 {2,5}。要比對這個字元本身請寫 \\{。',
      backendTitle: '這個正規表示式無法執行',
      backendBody:
        'PathKeep 使用 Rust 的正規表示式語法，它拒絕了這個模式：{message}',
    },
    semantic: {
      builtIn: '內建模型',
      builtInModel: '內建模型（{model}）',
      using: '語意搜尋使用{provider}',
      indexed_one: '已索引 {count} 個頁面',
      indexed_other: '已索引 {count} 個頁面',
      updated: '{when}更新',
      stale: '之後新增的頁面還沒有加進去',
      rebuild: '到「設定 → AI」重建',
      fellBack: '下面是全文搜尋的結果。',
      openSettings: '設定 → AI',
      whereToFix: '請看「設定 → AI」。',
      reason: {
        off: '語意搜尋沒有開啟。',
        building: '語意索引還在建立中。',
        queued: '語意索引正在等待建立。',
        paused: '語意索引的建立已暫停。',
        empty: '語意索引是空的。',
        failed: '語意索引建立失敗。',
        degraded: '語意索引現在無法使用：模型無法使用，或索引裡沒有向量。',
        blocked: '要先完成存檔設定，才能使用語意搜尋。',
      },
    },
  },
})
