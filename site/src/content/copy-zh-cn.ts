/**
 * The landing page in Simplified Chinese. Same facts as `copy-en.ts`;
 * product nouns (浏览记录, Agent 对话, 首页, 记录, 洞察, 助理, 备份, 项目)
 * are the app's own.
 */

import type { Copy } from './copy'

export const zhCN: Copy = {
  htmlLang: 'zh-Hans-CN',
  description:
    'Chrome 会删掉 90 天前的浏览记录，Claude Code 会删掉 30 天前的对话。PathKeep 把这些全部留在你自己的电脑上，也帮你找回来。',
  skip: '跳到正文',
  nav: {
    label: '章节',
    why: '为什么',
    keeps: '保存什么',
    privacy: '隐私',
    faq: '常见问题',
    github: 'GitHub',
    download: '下载',
  },
  hero: {
    title: '留住你|走过的路。',
    lede: '{chrome:Chrome} 会删掉 90 天前的浏览记录，|{claudecode:Claude Code} 会删掉 30 天前的对话。|PathKeep 把这些全部留在|你自己的电脑上，|也帮你找回来。',
    primary: '下载 macOS 版',
    secondary: 'Windows 版',
    demo: '在浏览器里试用 →',
    note: '0.6 版 · 免费、开源 · macOS 与 Windows',
  },
  shot: {
    tablist: '空间',
    tabs: { browsing: '浏览记录', agent: 'Agent 对话' },
    caption: 'app 的画面。可以操作的 demo 在下面。',
  },
  why: {
    title: '你的记录有保存期限，|而你从没同意过。',
    body: '浏览器和 coding agent 只留最近一小段，其余的自己清掉。同步也不会替你保存：它只是把最近的记录搬到你别的设备上。',
    axis: ['90 天', '一年', '永远'],
    rows: [
      { source: 'Chrome、Edge、Brave、Arc', keeps: '90 天', days: 90 },
      { source: 'Safari', keeps: '最多一年', days: 365 },
      { source: 'Firefox 同步', keeps: '只发送 30 天，保留 60 天', days: 60 },
      { source: 'Claude Code', keeps: '30 天', days: 30 },
      { source: 'PathKeep', keeps: '想留多久就留多久', days: null },
    ],
    source: '数字来自各浏览器的源代码与文档，以及 Claude Code 的设置说明。',
    closing: '如果你用 Chrome、从没备份过，|三个月前的记录|多半已经不在了。',
  },
  keeps: {
    title: '两种记忆，|一个存档。',
    body: 'PathKeep 把每一种数据一点一点备份进你硬盘上的同一个存档，想留多久就留多久。在 app 里，每一种都有自己的空间。',
    spaces: [
      {
        id: 'browsing',
        name: '浏览记录',
        covers:
          '电脑上每个浏览器、每个 profile：{chrome:Chrome}、{edge:Edge}、{firefox:Firefox}、{safari:Safari}、{arc:Arc}、{brave:Brave} 等等。更早的记录可以从 Google Takeout 找回来。',
        pages: '首页 · 记录 · 洞察 · 备份',
      },
      {
        id: 'agent',
        name: 'Agent 对话',
        covers:
          '{claudecode:Claude Code}、{codex:Codex}、{pi:Pi}、{dsh:DeepSeek Harness} 的对话，连同项目、subagent、token 用量，以及 Agent 工作了多少小时。',
        pages: '洞察 · 记录 · 项目 · 备份',
      },
    ],
    more: '之后还会加入更多会被悄悄删掉的东西，每一种都有自己的空间。',
  },
  find: {
    title: '留下来，|只是一半。',
    body: '记录有几百万条，PathKeep 照样很快。两年前看过的那一页，敲几个字就找得到。',
    items: [
      {
        id: 'search',
        title: '搜索',
        text: '搜标题、网址和页面内容，也能用正则表达式。按日期、浏览器、网站筛选。',
      },
      {
        id: 'history',
        title: '记录',
        text: '每一次访问都按日期、按同一段浏览排好，列表再长也滑得顺。',
      },
      {
        id: 'insights',
        title: '洞察',
        text: '在你的电脑上算出来：最常去的网站、什么时候上网、搜索了什么、一再回去的页面。',
      },
      {
        id: 'agent',
        title: 'Agent 洞察',
        text: 'Agent 工作了多久、你在场多久、用了多少 token、按标价算花了多少，按项目、按工具看。',
      },
      {
        id: 'ask',
        title: '助理（可选）',
        text: '用平常的话问你的记录，用你电脑上的模型或你自己配置的服务。它会列出用到的页面。',
      },
      {
        id: 'notes',
        title: '笔记与导出',
        text: '给页面加星标、写之后搜得到的笔记，全部导出成 HTML、Markdown、纯文本或 JSON Lines。',
      },
    ],
  },
  privacy: {
    title: '你的记录|只留在你的电脑上。',
    body: '没有账号，也没有服务器。PathKeep 读取浏览器和工具的文件，写进自己的存档，从不改动它们的文件。',
    points: [
      'AI 默认关闭，要你自己打开。用电脑上的模型，数据就不会离开这台电脑。',
      '存档可以用密码加密。',
      '每次备份和导入都留下看得懂的记录，每次导入都能撤销。',
      '以 GPL-3.0 授权的自由软件，每一行都在 GitHub 上。',
    ],
    network:
      '只有两个功能会连到网站：链接预览和页面摘要。设置里看得到这两项，都可以关掉。',
  },
  demo: {
    title: '在浏览器里|试用 PathKeep。',
    body: '这里跑的是完整的 app，里面是编出来的记录，不是任何人真实的浏览记录。随便点，你做的事都不会离开这一页。',
    newTab: '在新标签页打开 demo',
    narrow: 'demo 需要电脑的屏幕，用电脑打开这一页就能试。',
    frame: 'PathKeep demo',
    start: '点一下开始试用',
  },
  faq: {
    title: '常见问题',
    items: [
      {
        title: '我的记录会离开我的电脑吗？',
        text: '不会。PathKeep 没有服务器，也没有账号。链接预览和页面摘要会直接连到那些网站，两项都可以关掉。AI 在你选好模型之前都是关着的。',
      },
      {
        title: '支持哪些浏览器？',
        text: '已测试：Chrome、Edge、Firefox，以及 macOS 上的 Safari、ChatGPT Atlas、Perplexity Comet。Brave、Vivaldi、Arc、Opera、Chromium、LibreWolf、Floorp、Waterfox 也会备份，只是还没为正式版测试过。',
      },
      {
        title: '它会改动浏览器自己的记录吗？',
        text: '不会。PathKeep 只读不写。浏览器照常运行，PathKeep 另外留一份。',
      },
      {
        title: '已经被删掉的记录，还找得回来吗？',
        text: '能找回一部分。Google Takeout 里有你 Google 账号的 Chrome 记录，常常有 18 个月左右，PathKeep 可以导入。别的浏览器或别的电脑的记录文件也行。每次导入都能撤销。',
      },
      {
        title: '一定要用 AI 吗？',
        text: '不用。备份、记录、搜索、洞察都不需要 AI。助理和语义搜索是给想用的人用的。',
      },
      {
        title: '要钱吗？',
        text: '不要。PathKeep 是以 GPL-3.0 授权的自由软件，源代码都在 GitHub 上。',
      },
      {
        title: '它是用什么做的？',
        text: '用 Rust 和 Tauri 2 写的桌面 app，不是 Electron；界面用 React 和 TypeScript。存档用 Turso，一个用 Rust 写、兼容 SQLite 的数据库；全文搜索用 Tantivy。',
      },
    ],
  },
  download: {
    title: '从今天起，|留住走过的路。',
    body: '下载就能用，不用账号。它会找出你的浏览器和 Agent 工具，保存任何东西之前都先问你。',
    platforms: [
      { name: 'macOS', detail: '磁盘映像（.dmg）', action: '下载 macOS 版' },
      {
        name: 'Windows',
        detail: '安装程序（.msi 或 .exe）',
        action: '下载 Windows 版',
      },
      {
        name: 'Linux',
        detail: '准备中',
        action: '在 GitHub 上关注 →',
        soon: true,
      },
    ],
    note: '安装包还没有签名，第一次打开的方法写在 README。',
    source: '源代码在 GitHub',
  },
  footer: {
    license: 'PathKeep 是以 GPL-3.0 授权的自由软件。',
    links: {
      label: '项目',
      github: 'GitHub',
      releases: '下载页',
      readme: 'README',
    },
  },
  notFound: {
    title: '这里什么都没有。',
    body: '这一页不存在，或者已经搬走了。',
    home: '回到 PathKeep',
  },
}
