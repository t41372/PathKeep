/**
 * The landing page in Traditional Chinese. Same facts as `copy-en.ts`;
 * product nouns (瀏覽紀錄, Agent 對話, 首頁, 紀錄, 洞察, 助理, 備份, 專案)
 * are the app's own.
 */

import type { Copy } from './copy'

export const zhTW: Copy = {
  htmlLang: 'zh-Hant-TW',
  description:
    'Chrome 會刪掉 90 天前的瀏覽紀錄，Claude Code 會刪掉 30 天前的對話。PathKeep 把這些全部留在你自己的電腦上，也幫你找回來。',
  skip: '跳到內容',
  nav: {
    label: '章節',
    why: '為什麼',
    keeps: '保存什麼',
    privacy: '隱私',
    faq: '常見問題',
    github: 'GitHub',
    download: '下載',
  },
  hero: {
    title: '留住你|走過的路。',
    lede: '{chrome:Chrome} 會刪掉 90 天前的瀏覽紀錄，|{claudecode:Claude Code} 會刪掉 30 天前的對話。|PathKeep 把這些全部留在|你自己的電腦上，|也幫你找回來。',
    primary: '下載 macOS 版',
    secondary: 'Windows 版',
    demo: '在瀏覽器裡試用 →',
    note: '0.6 版 · 免費、開源 · macOS 與 Windows',
  },
  shot: {
    tablist: '空間',
    tabs: { browsing: '瀏覽紀錄', agent: 'Agent 對話' },
    caption: 'app 的畫面。可以操作的 demo 在下面。',
  },
  why: {
    title: '你的紀錄有保存期限，|而你從沒同意過。',
    body: '瀏覽器和 coding agent 只留最近一小段，其餘的自己清掉。同步也不會替你保存：它只是把最近的紀錄搬到你別的裝置上。',
    axis: ['90 天', '一年', '永遠'],
    rows: [
      { source: 'Chrome、Edge、Brave、Arc', keeps: '90 天', days: 90 },
      { source: 'Safari', keeps: '最多一年', days: 365 },
      { source: 'Firefox 同步', keeps: '只送 30 天，保留 60 天', days: 60 },
      { source: 'Claude Code', keeps: '30 天', days: 30 },
      { source: 'PathKeep', keeps: '想留多久就留多久', days: null },
    ],
    source: '數字來自各瀏覽器的原始碼與文件，以及 Claude Code 的設定說明。',
    closing: '如果你用 Chrome、從沒備份過，|三個月前的紀錄|多半已經不在了。',
  },
  keeps: {
    title: '兩種記憶，|一個存檔。',
    body: 'PathKeep 把每一種資料一點一點備份進你硬碟上的同一個存檔，想留多久就留多久。在 app 裡，每一種都有自己的空間。',
    spaces: [
      {
        id: 'browsing',
        name: '瀏覽紀錄',
        covers:
          '電腦上每個瀏覽器、每個 profile：{chrome:Chrome}、{edge:Edge}、{firefox:Firefox}、{safari:Safari}、{arc:Arc}、{brave:Brave} 等等。更早的紀錄可以從 Google Takeout 找回來。',
        pages: '首頁 · 紀錄 · 洞察 · 備份',
      },
      {
        id: 'agent',
        name: 'Agent 對話',
        covers:
          '{claudecode:Claude Code}、{codex:Codex}、{pi:Pi}、{dsh:DeepSeek Harness} 的對話，連同專案、subagent、token 用量，和 Agent 工作了多少小時。',
        pages: '洞察 · 紀錄 · 專案 · 備份',
      },
    ],
    more: '之後還會加入更多會被悄悄刪掉的東西，每一種都有自己的空間。',
  },
  find: {
    title: '留下來，|只是一半。',
    body: '紀錄有幾百萬筆，PathKeep 照樣很快。兩年前看過的那一頁，敲幾個字就找得到。',
    items: [
      {
        id: 'search',
        title: '搜尋',
        text: '搜標題、網址和頁面內容，也能用正規表示式。依日期、瀏覽器、網站篩選。',
      },
      {
        id: 'history',
        title: '紀錄',
        text: '每一次造訪都依日期、依同一段瀏覽排好，清單再長也滑得順。',
      },
      {
        id: 'insights',
        title: '洞察',
        text: '在你的電腦上算出來：最常去的網站、什麼時候上網、搜尋了什麼、一再回去的頁面。',
      },
      {
        id: 'agent',
        title: 'Agent 洞察',
        text: 'Agent 工作了多久、你在場多久、用了多少 token、按牌價算花了多少，依專案、依工具看。',
      },
      {
        id: 'ask',
        title: '助理（選用）',
        text: '用平常的話問你的紀錄，用你電腦上的模型或你自己設定的服務。它會列出用到的頁面。',
      },
      {
        id: 'notes',
        title: '筆記與匯出',
        text: '幫頁面加星號、寫之後搜得到的筆記，全部匯出成 HTML、Markdown、純文字或 JSON Lines。',
      },
    ],
  },
  privacy: {
    title: '你的紀錄|只留在你的電腦上。',
    body: '沒有帳號，也沒有伺服器。PathKeep 讀瀏覽器和工具的檔案，寫進自己的存檔，從不改動它們的檔案。',
    points: [
      'AI 預設關閉，要你自己打開。用電腦上的模型，資料就不會離開這台電腦。',
      '存檔可以用密碼加密。',
      '每次備份和匯入都留下看得懂的紀錄，每次匯入都能復原。',
      '以 GPL-3.0 授權的自由軟體，每一行都在 GitHub 上。',
    ],
    network:
      '只有兩個功能會連到網站：連結預覽和頁面摘要。設定裡看得到這兩項，都可以關掉。',
  },
  demo: {
    title: '在瀏覽器裡|試用 PathKeep。',
    body: '這裡跑的是完整的 app，裡面是編出來的紀錄，不是任何人真的瀏覽紀錄。隨便點，你做的事都不會離開這一頁。',
    newTab: '在新分頁打開 demo',
    narrow: 'demo 需要電腦的螢幕，用電腦打開這一頁就能試。',
    frame: 'PathKeep demo',
    start: '點一下開始試用',
  },
  faq: {
    title: '常見問題',
    items: [
      {
        title: '我的紀錄會離開我的電腦嗎？',
        text: '不會。PathKeep 沒有伺服器，也沒有帳號。連結預覽和頁面摘要會直接連到那些網站，兩項都可以關掉。AI 在你選好模型之前都是關著的。',
      },
      {
        title: '支援哪些瀏覽器？',
        text: '已測試：Chrome、Edge、Firefox，以及 macOS 上的 Safari、ChatGPT Atlas、Perplexity Comet。Brave、Vivaldi、Arc、Opera、Chromium、LibreWolf、Floorp、Waterfox 也會備份，只是還沒為正式版測過。',
      },
      {
        title: '它會改動瀏覽器自己的紀錄嗎？',
        text: '不會。PathKeep 只讀不寫。瀏覽器照常運作，PathKeep 另外留一份。',
      },
      {
        title: '已經被刪掉的紀錄，還找得回來嗎？',
        text: '找得回一部分。Google Takeout 裡有你 Google 帳號的 Chrome 紀錄，常常有 18 個月左右，PathKeep 可以匯入。別的瀏覽器或別台電腦的紀錄檔也行。每次匯入都能復原。',
      },
      {
        title: '一定要用 AI 嗎？',
        text: '不用。備份、紀錄、搜尋、洞察都不需要 AI。助理和語意搜尋是給想用的人用的。',
      },
      {
        title: '要錢嗎？',
        text: '不用。PathKeep 是以 GPL-3.0 授權的自由軟體，原始碼都在 GitHub 上。',
      },
      {
        title: '它是用什麼做的？',
        text: '用 Rust 和 Tauri 2 寫的桌面 app，不是 Electron；介面用 React 和 TypeScript。存檔用 Turso，一個用 Rust 寫、相容 SQLite 的資料庫；全文搜尋用 Tantivy。',
      },
    ],
  },
  download: {
    title: '從今天起，|留住走過的路。',
    body: '下載就能用，不用帳號。它會找出你的瀏覽器和 Agent 工具，保存任何東西之前都先問你。',
    platforms: [
      { name: 'macOS', detail: '磁碟映像檔（.dmg）', action: '下載 macOS 版' },
      {
        name: 'Windows',
        detail: '安裝程式（.msi 或 .exe）',
        action: '下載 Windows 版',
      },
      {
        name: 'Linux',
        detail: '準備中',
        action: '在 GitHub 上關注 →',
        soon: true,
      },
    ],
    note: '安裝檔還沒有簽章，第一次開啟的方法寫在 README。',
    source: '原始碼在 GitHub',
  },
  footer: {
    license: 'PathKeep 是以 GPL-3.0 授權的自由軟體。',
    links: {
      label: '專案',
      github: 'GitHub',
      releases: '下載頁',
      readme: 'README',
    },
  },
  notFound: {
    title: '這裡什麼都沒有。',
    body: '這一頁不存在，或已經搬走了。',
    home: '回到 PathKeep',
  },
}
