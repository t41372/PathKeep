/**
 * @file settings-content-fetch.ts
 * @description Settings translation owner for the site-content-fetch consent surface (W-ENRICH-1).
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide the consent + network-policy disclosure copy for en, zh-CN, zh-TW.
 * - Keep the egress disclosure HONEST and identical in meaning across locales:
 *   the target host learns only IP, a desktop browser UA, Accept-Language, and
 *   the URL path the user already visited — NO cookies, Referer, or account.
 *
 * ## Not responsible for
 * - Detail-panel / search enrichment copy (those live in the explorer namespace).
 * - Translator creation, flattening, or language resolution.
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` merges this into the `settings`
 *   namespace during catalog assembly.
 *
 * ## Performance notes
 * - Static dictionary data only; keep this file side-effect free.
 */

/**
 * Keeps the content-fetch consent subsection aligned across shipping locales.
 *
 * Privacy is the load-bearing concern here: this is the consent boundary for the
 * only PathKeep feature that reaches out to the sites the user visited. The
 * disclosure copy must never overstate what is sent (it is deliberately minimal)
 * and never understate it (no "anonymous" hand-waving — IP + a generic desktop
 * UA + Accept-Language + the path ARE sent).
 */
export const settingsContentFetchNamespace = {
  en: {
    contentFetchNavLabel: 'Site content',
    contentFetchTitle: 'SITE CONTENT',
    contentFetchIntro:
      'Optionally enrich pages you already visited with structured detail — a GitHub repo’s description and topics, or a short readable summary of an article. Everything stays on this device.',

    contentFetchMasterLabel: 'Fetch site content',
    contentFetchMasterHelp:
      'Off by default. While off, PathKeep never contacts any site — search and browsing only read content that was already fetched. Turning this on is your consent to reach out to the sites below.',
    contentFetchMasterOn: 'On',
    contentFetchMasterOff: 'Off',

    contentFetchDisclosureTitle: 'What a site learns when PathKeep fetches',
    contentFetchDisclosureBody:
      'Only the minimum a normal request reveals: your IP address, a generic desktop browser identity, your Accept-Language header, and the path of the page you already opened.',
    contentFetchDisclosureNotSent:
      'Never sent: cookies, your Referer, any account or sign-in, and nothing that fingerprints you beyond a plain request.',
    contentFetchDisclosureOffline:
      'Offline-first: fetching never runs during backup or import, and search and browsing never wait on the network.',
    contentFetchDisclosureRateLimit:
      'Rate-limited per host (GitHub allows about 60 requests an hour to one IP), so PathKeep fetches slowly and never floods a site.',

    contentFetchExtractorsLabel: 'Sources',
    contentFetchExtractorsHelp:
      'Choose which kinds of content PathKeep may fetch once the switch above is on.',
    contentFetchExtractorGithubRepo: 'GitHub repositories',
    contentFetchExtractorGithubRepoHint:
      'Public repo description, topics, and README summary via the GitHub API.',
    contentFetchExtractorGenericReadable: 'Page summaries',
    contentFetchExtractorGenericReadableHint:
      'A short readable summary extracted from a normal web page.',
    contentFetchExtractorOn: 'Allowed',
    contentFetchExtractorOff: 'Off',
    contentFetchExtractorUnknown: 'Source',

    contentFetchLimitedTitle: 'Limited or unavailable',
    contentFetchLimitedBody:
      'Video captions (YouTube, Bilibili) are best-effort and often empty without sign-in; X / Twitter posts are not available without an account. These are not fetched in this version.',

    contentFetchDomainsLabel: 'Per-site rules',
    contentFetchDomainsHelp:
      'Block specific sites even when fetching is on. One host per line, e.g. example.com.',
    contentFetchDomainsPlaceholder: 'blocked-site.com',
    contentFetchDomainsSave: 'Save blocked sites',
    contentFetchDomainsReset: 'Reset',
    contentFetchDomainsEmpty: 'No sites are blocked.',

    contentFetchStatusLabel: 'Activity',
    contentFetchStatusSummary:
      '{stored} enriched · {queued} queued · {running} fetching · {failed} failed',
    contentFetchStatusEmpty: 'Nothing fetched yet.',
    contentFetchStatusOff:
      'Turn on fetching to enrich the pages you care about.',

    contentFetchPrimeLabel: 'Enrich now',
    contentFetchPrimeHelp:
      'Queue a prioritized batch of pages you care about most (starred, recent, tagged) for enrichment.',
    contentFetchPrimeAction: 'Enrich top pages',
    contentFetchPriming: 'Queuing…',
    contentFetchPrimeSummary: 'Queued {count} pages for enrichment.',
    contentFetchPrimeNone: 'Nothing new to enrich right now.',
    contentFetchSaving: 'Saving…',
    contentFetchSaveError: 'Could not save. Your change was not applied.',
    contentFetchUnavailable:
      'Site content fetching only works in the desktop app.',
  },
  'zh-CN': {
    contentFetchNavLabel: '站点内容',
    contentFetchTitle: '站点内容',
    contentFetchIntro:
      '可选：为你已经访问过的页面补充结构化信息——GitHub 仓库的描述和主题，或文章的简短可读摘要。所有内容都留在本机。',

    contentFetchMasterLabel: '抓取站点内容',
    contentFetchMasterHelp:
      '默认关闭。关闭时 PathKeep 绝不会联系任何站点——搜索和浏览只读取已抓取过的内容。打开它即表示你同意去访问下面这些站点。',
    contentFetchMasterOn: '开',
    contentFetchMasterOff: '关',

    contentFetchDisclosureTitle: 'PathKeep 抓取时，站点会知道什么',
    contentFetchDisclosureBody:
      '仅限一次普通请求会暴露的最少信息：你的 IP 地址、一个通用的桌面浏览器标识、你的 Accept-Language 头，以及你本就打开过的页面路径。',
    contentFetchDisclosureNotSent:
      '绝不发送：Cookie、你的 Referer、任何账号或登录信息，以及任何超出普通请求、能用来识别你的指纹。',
    contentFetchDisclosureOffline:
      '离线优先：抓取绝不会在备份或导入期间运行，搜索和浏览也绝不会等待网络。',
    contentFetchDisclosureRateLimit:
      '按主机限速（GitHub 对单个 IP 大约每小时允许 60 次请求），因此 PathKeep 会缓慢抓取，绝不会冲击站点。',

    contentFetchExtractorsLabel: '来源',
    contentFetchExtractorsHelp:
      '选择在上面的开关打开后，PathKeep 可以抓取哪些内容。',
    contentFetchExtractorGithubRepo: 'GitHub 仓库',
    contentFetchExtractorGithubRepoHint:
      '通过 GitHub API 获取公开仓库的描述、主题和 README 摘要。',
    contentFetchExtractorGenericReadable: '页面摘要',
    contentFetchExtractorGenericReadableHint:
      '从普通网页中提取的简短可读摘要。',
    contentFetchExtractorOn: '允许',
    contentFetchExtractorOff: '关',
    contentFetchExtractorUnknown: '来源',

    contentFetchLimitedTitle: '受限或不可用',
    contentFetchLimitedBody:
      '视频字幕（YouTube、Bilibili）尽力而为，未登录时常为空；X / Twitter 帖子没有账号无法获取。本版本不会抓取这些内容。',

    contentFetchDomainsLabel: '按站点规则',
    contentFetchDomainsHelp:
      '即使抓取已打开，也可屏蔽特定站点。每行一个主机，例如 example.com。',
    contentFetchDomainsPlaceholder: 'blocked-site.com',
    contentFetchDomainsSave: '保存屏蔽站点',
    contentFetchDomainsReset: '重置',
    contentFetchDomainsEmpty: '没有屏蔽任何站点。',

    contentFetchStatusLabel: '活动',
    contentFetchStatusSummary:
      '已补充 {stored} · 排队 {queued} · 抓取中 {running} · 失败 {failed}',
    contentFetchStatusEmpty: '尚未抓取任何内容。',
    contentFetchStatusOff: '打开抓取，为你关心的页面补充内容。',

    contentFetchPrimeLabel: '立即补充',
    contentFetchPrimeHelp:
      '为你最关心的一批页面（已加星、近期、已打标签）排队补充内容。',
    contentFetchPrimeAction: '补充重点页面',
    contentFetchPriming: '排队中…',
    contentFetchPrimeSummary: '已为 {count} 个页面排队补充。',
    contentFetchPrimeNone: '当前没有需要补充的新页面。',
    contentFetchSaving: '保存中…',
    contentFetchSaveError: '无法保存。你的更改未生效。',
    contentFetchUnavailable: '站点内容抓取仅在桌面应用中可用。',
  },
  'zh-TW': {
    contentFetchNavLabel: '網站內容',
    contentFetchTitle: '網站內容',
    contentFetchIntro:
      '可選：為你已經造訪過的頁面補充結構化資訊——GitHub 儲存庫的描述和主題，或文章的簡短可讀摘要。所有內容都留在本機。',

    contentFetchMasterLabel: '擷取網站內容',
    contentFetchMasterHelp:
      '預設關閉。關閉時 PathKeep 絕不會聯絡任何網站——搜尋和瀏覽只會讀取已擷取過的內容。打開它即表示你同意去造訪下面這些網站。',
    contentFetchMasterOn: '開',
    contentFetchMasterOff: '關',

    contentFetchDisclosureTitle: 'PathKeep 擷取時，網站會知道什麼',
    contentFetchDisclosureBody:
      '僅限一次普通請求會揭露的最少資訊：你的 IP 位址、一個通用的桌面瀏覽器識別、你的 Accept-Language 標頭，以及你本就開啟過的頁面路徑。',
    contentFetchDisclosureNotSent:
      '絕不傳送：Cookie、你的 Referer、任何帳號或登入資訊，以及任何超出普通請求、能用來識別你的指紋。',
    contentFetchDisclosureOffline:
      '離線優先：擷取絕不會在備份或匯入期間執行，搜尋和瀏覽也絕不會等待網路。',
    contentFetchDisclosureRateLimit:
      '依主機限速（GitHub 對單一 IP 大約每小時允許 60 次請求），因此 PathKeep 會緩慢擷取，絕不會衝擊網站。',

    contentFetchExtractorsLabel: '來源',
    contentFetchExtractorsHelp:
      '選擇在上面的開關打開後，PathKeep 可以擷取哪些內容。',
    contentFetchExtractorGithubRepo: 'GitHub 儲存庫',
    contentFetchExtractorGithubRepoHint:
      '透過 GitHub API 取得公開儲存庫的描述、主題和 README 摘要。',
    contentFetchExtractorGenericReadable: '頁面摘要',
    contentFetchExtractorGenericReadableHint:
      '從普通網頁中擷取的簡短可讀摘要。',
    contentFetchExtractorOn: '允許',
    contentFetchExtractorOff: '關',
    contentFetchExtractorUnknown: '來源',

    contentFetchLimitedTitle: '受限或不可用',
    contentFetchLimitedBody:
      '影片字幕（YouTube、Bilibili）盡力而為，未登入時常為空；X / Twitter 貼文沒有帳號無法取得。本版本不會擷取這些內容。',

    contentFetchDomainsLabel: '依網站規則',
    contentFetchDomainsHelp:
      '即使擷取已打開，也可封鎖特定網站。每行一個主機，例如 example.com。',
    contentFetchDomainsPlaceholder: 'blocked-site.com',
    contentFetchDomainsSave: '儲存封鎖網站',
    contentFetchDomainsReset: '重設',
    contentFetchDomainsEmpty: '沒有封鎖任何網站。',

    contentFetchStatusLabel: '活動',
    contentFetchStatusSummary:
      '已補充 {stored} · 排隊 {queued} · 擷取中 {running} · 失敗 {failed}',
    contentFetchStatusEmpty: '尚未擷取任何內容。',
    contentFetchStatusOff: '打開擷取，為你在意的頁面補充內容。',

    contentFetchPrimeLabel: '立即補充',
    contentFetchPrimeHelp:
      '為你最在意的一批頁面（已加星、近期、已標籤）排隊補充內容。',
    contentFetchPrimeAction: '補充重點頁面',
    contentFetchPriming: '排隊中…',
    contentFetchPrimeSummary: '已為 {count} 個頁面排隊補充。',
    contentFetchPrimeNone: '目前沒有需要補充的新頁面。',
    contentFetchSaving: '儲存中…',
    contentFetchSaveError: '無法儲存。你的變更未生效。',
    contentFetchUnavailable: '網站內容擷取僅在桌面應用程式中可用。',
  },
} as const
