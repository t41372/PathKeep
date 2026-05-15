<div align="center">

# PathKeep —— 留住你走过的路

<img alt="demo" src="https://github.com/user-attachments/assets/6118f6f4-80ee-4cd2-bf44-3989d26eb5e5" />

**别让浏览器悄悄替你删掉你的浏览记录**

<a href="./README.CN.md">
<img alt="zh-CN" src="https://img.shields.io/badge/lang-简体中文-red.svg" />
</a>
<a href="./README.md">
<img alt="en" src="https://img.shields.io/badge/lang-English-blue.svg" />
</a>

<a href="https://github.com/t41372/PathKeep/actions/workflows/ci.yml">
<img alt="CI" src="https://github.com/t41372/PathKeep/actions/workflows/ci.yml/badge.svg?branch=main&event=push" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/native-deps.yml">
<img alt="Native Dependencies" src="https://github.com/t41372/PathKeep/actions/workflows/native-deps.yml/badge.svg?branch=main" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/release.yml">
<img alt="Release" src="https://github.com/t41372/PathKeep/actions/workflows/release.yml/badge.svg?branch=main" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/mutation.yml">
<img alt="Mutation" src="https://github.com/t41372/PathKeep/actions/workflows/mutation.yml/badge.svg?branch=main" />
</a>
<a href="https://app.codecov.io/github/t41372/PathKeep">
<img alt="Codecov" src="https://codecov.io/github/t41372/PathKeep/branch/main/graph/badge.svg" />
</a>
<br />
<a href="https://github.com/t41372/PathKeep/releases">
<img alt="Latest release" src="https://img.shields.io/github/v/release/t41372/PathKeep?display_name=tag&label=release" />
</a>
<a href="https://www.rust-lang.org/">
<img alt="Rust 1.94.1" src="https://img.shields.io/badge/Rust-1.94.1-000000?logo=rust&logoColor=white" />
</a>
<a href="https://tauri.app/">
<img alt="Tauri v2" src="https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white" />
</a>
<a href="https://react.dev/">
<img alt="React 19" src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=000000" />
</a>
<a href="https://bun.sh/">
<img alt="Bun 1.3.11" src="https://img.shields.io/badge/Bun-1.3.11-000000?logo=bun&logoColor=white" />
</a>
<a href="https://www.typescriptlang.org/">
<img alt="TypeScript 5.9" src="https://img.shields.io/badge/TypeScript-5.9-3178C6?logo=typescript&logoColor=white" />
</a>
<a href="https://vite.dev/">
<img alt="Vite 8" src="https://img.shields.io/badge/Vite-8-646CFF?logo=vite&logoColor=white" />
</a>
<a href="./LICENSE">
<img alt="License: GPL-3.0" src="https://img.shields.io/github/license/t41372/PathKeep?label=license" />
</a>

</div>

中文 | [English](./README.md)

<br/>

PathKeep 是一款**本地优先**的桌面应用，用来长期归档浏览历史，并在归档之上做进一步的分析。基于 Tauri 2、Rust、React 19、TypeScript、Vite、Bun 构建。目前公开承诺支持的浏览器包括 Google Chrome、Microsoft Edge、Firefox，以及 macOS 上的 ChatGPT Atlas、Perplexity Comet 和 Safari（Safari 需要"完全磁盘访问权限"）。其他 Chromium 系 / Firefox 系浏览器的适配器其实已经实现，只是暂未纳入公开承诺。

---

## 为什么需要 PathKeep

### 你的浏览器，正在悄悄删掉你的历史记录

很多人默认以为浏览历史一直都在那里，需要的时候随时能翻出来。**其实并不是。**

绝大多数 Chromium 内核浏览器——Chrome、Edge、Brave、Arc 等等——都继承了上游的一个默认行为：**本地浏览历史超过大约 90 天就会被自动清除**。这不是 bug，是直接[硬编码在 Chromium 源码里](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)的。Safari 默认[只保留一年](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac)，除非你自己改设置。即使是相对宽松的 Firefox，最终也会根据[数据库大小](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)清理旧记录。

云同步也别指望。云同步是为了多设备体验，而不是为了长期归档。Firefox Sync [只上传最近 30 天，且云端 60 天后过期](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs)。Brave Sync [只同步你手动输入的网址](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ)。Arc [则根本不同步历史](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync)。

**如果你只用 Chrome，又从来没手动备份过，那么三个月之前的本地浏览历史，基本可以认定已经没了。** 去年看过的网页、人生关键节点搜过的关键词、深夜钻研某个新领域留下的全部足迹——都被你的浏览器替你"决定"删掉了。

<details><summary><b>📋 各家浏览器到底会保留多久？（参考表格）</b></summary>

下表基于截至 2026 年 4 月的官方文档、帮助中心和源代码。"云同步"指各浏览器自家的同步服务。保留时长通常受用户设置影响，下面列出的是默认值。

| 浏览器               | 内核        | 本地默认                                                                                                                                      | 本地上限                                                                                                                                       | 云同步                                                                                                                                                                                  | 实际上限                     |
| -------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| **Google Chrome**    | Chromium    | 约 3 个月（Chromium 默认）[\[1\]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | 未提供内置设置                                                                                                                                 | Google 账号可能根据"Web 与应用活动"保留更久 [\[2\]](https://support.google.com/chrome/answer/165139)                                                                                    | 本地约 3 个月；同步可能更久  |
| **Microsoft Edge**   | Chromium    | 约 3 个月（Chromium 默认）[\[1\]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | 企业策略只能进一步*缩短* [\[7\]](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/browsingdatalifetime)            | 隐私面板保留部分同步数据，TTL 未公开 [\[5\]](https://support.microsoft.com/en-us/microsoft-edge/view-and-delete-browser-history-in-microsoft-edge-00cf7943-a9e1-975a-a33d-ac10ce454ca4) | 本地约 3 个月；云端 TTL 未知 |
| **Brave**            | Chromium    | 约 3 个月（Chromium 默认）[\[1\]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | 未找到永久保留设置                                                                                                                             | 仅同步手动输入的 URL；12 个月未活动后服务器删除 [\[8\]](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ)                                                              | 本地约 3 个月                |
| **Arc**              | Chromium    | 约 3 个月（Chromium 默认）[\[1\]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | 未找到永久保留设置                                                                                                                             | **不同步历史** [\[12\]](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync)                                                                                            | 本地约 3 个月                |
| **Opera / Opera GX** | Chromium    | 约 3 个月（Chromium 默认）[\[1\]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | 未找到永久保留设置                                                                                                                             | 同步历史，但云端 TTL 未公开 [\[17\]](https://help.opera.com/en/latest/features/#sync)                                                                                                   | 本地约 3 个月                |
| **Vivaldi**          | Chromium    | 3 个月 [\[9\]](https://help.vivaldi.com/desktop/navigation/history/)                                                                          | **永久**（用户可设置）[\[9\]](https://help.vivaldi.com/desktop/navigation/history/)                                                            | 同步历史，但云端 TTL 未公开 [\[10\]](https://help.vivaldi.com/desktop/tools/sync/)                                                                                                      | 永久（取决于配置）           |
| **Dia**              | Chromium 系 | 未披露（大概约 3 个月）                                                                                                                       | 未找到永久保留设置                                                                                                                             | 提供端到端加密同步，但未公开历史 TTL [\[28\]](https://diabrowser.com/privacy)                                                                                                           | 未披露                       |
| **Safari**           | WebKit      | 1 年（默认）[\[14\]](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac)                                    | 可配置（未找到硬上限）                                                                                                                         | iCloud 跨设备同步，云端 TTL 未公开 [\[15\]](https://support.apple.com/guide/icloud/what-you-can-do-with-icloud-and-safari-mm9b8da4f328/icloud)                                          | 可配置                       |
| **Firefox**          | Gecko       | 按容量回收（无固定天数）[\[18\]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                | 可通过 `max_pages` 配置，可保留**数年** [\[18\]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs) | 仅上传最近 30 天，云端 60 天后过期 [\[19\]](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs)                                                          | 本地数年；Sync 约 60 天      |
| **LibreWolf**        | Firefox 系  | 类似 Firefox（除非开启"关闭即清空"）[\[21\]](https://librewolf.net/docs/faq/)                                                                 | 与 Firefox 相同 [\[18\]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                         | 可手动接入 Firefox Sync [\[21\]](https://librewolf.net/docs/faq/)                                                                                                                       | 类似 Firefox                 |
| **Floorp**           | Firefox 系  | 推测类似 Firefox                                                                                                                              | 未公开                                                                                                                                         | 未公开                                                                                                                                                                                  | 推测类似 Firefox             |
| **Waterfox**         | Firefox 系  | 推测类似 Firefox                                                                                                                              | 未公开                                                                                                                                         | 未公开                                                                                                                                                                                  | 推测类似 Firefox             |
| **Zen Browser**      | Firefox 系  | 推测类似 Firefox                                                                                                                              | 未公开                                                                                                                                         | 历史走 Firefox Sync，Window Sync 仅本地 [\[27\]](https://docs.zen-browser.app/user-manual/window-sync)                                                                                  | 本地推测类似 Firefox         |
| **ChatGPT Atlas**    | Chromium    | 未披露（大概约 3 个月）                                                                                                                       | 未披露                                                                                                                                         | 提供 Web History 控制项，云端 TTL 未公开 [\[30\]](https://help.openai.com/en/articles/12625059-web-browsing-settings-on-chatgpt-atlas)                                                  | 未披露                       |
| **Perplexity Comet** | Chromium 系 | 本地保存历史，TTL 未公开                                                                                                                      | 未披露                                                                                                                                         | 历史走端到端加密同步，云端 TTL 未公开 [\[32\]](https://comet-help.perplexity.ai/en/articles/12658050-log-in-to-perplexity)                                                              | 未披露                       |

**一句话总结：** 除非你用的是 Firefox（按容量回收），或者手动调过 Safari、Vivaldi 这种浏览器，否则你的浏览器几乎一定在按一个你从未同意过的时间表删除历史。

_注：约 3 个月的 Chromium 上限是基于上游 Chromium 源码推断出来的。下游浏览器有权修改这个行为，但绝大多数并未公开声明不同的保留策略。_

</details>

### 浏览历史比你以为的有价值

你可能会想：_这真的有那么重要吗？_

不妨想想浏览历史到底是什么。它记录了你**怎么学会一件事**、在**做重大决定前看了什么资料**、人生发生重要变化的那一周你**在读什么**。它记下了你做学生时关心的话题、入职那份工作之前的搜索、搬到新城市之前的功课。对越来越多人来说，相当一部分人生发生在线上——而浏览历史，是离一份"网络生活日志"最近的东西。

那些年的记录消失了，不是因为你选择删除，而是因为浏览器替你判断："不值得留。"

_在不久之前，浏览器删旧历史问题并不大_，因为几千万条原始 URL 摆在面前，也没什么实用方法从中提炼意义。但情况正在变。本地 AI 推理已经足够快、足够便宜，可以直接在你自己的机器上对**多年**的浏览数据跑 agentic 分析。Andrej Karpathy 提出的 ["LLM Knowledge Bases"](https://x.com/karpathy/status/2039805659525644595) 概念——让大模型为每个人维护一套个人知识库——正在变成现实。

所以问题已经不是"几十年的浏览历史能不能挖出价值"。问题是：等那一天真的到来，你**手里还有没有数据**。

**今天保住的数据，是明天智能的原料。** 但已经丢掉的，再也分析不出来了。

### PathKeep 在解决什么

PathKeep 安静地跑在你的电脑上，**按计划自动、增量地备份所有浏览器的历史**——不需要你动手。它从不直接读取正在运行的浏览器数据库，而是先复制出一份安全副本，去重后追加进一个**完全归你所有、归你控制**的本地归档。

在这份归档之上，PathKeep 提供强大的回溯能力（全文检索、正则、时间线、过滤器、导出），以及基于归档事实的确定性 Core Intelligence。语义搜索、AI 助手、MCP 等 AI 相关能力被规划在 v0.3 路线图，不属于 v0.2.0 的承诺范围。

> 用 Chrome 且开了 Google Sync？PathKeep 支持 **Google Takeout 导入**，可以把延伸到云端的那部分历史（通常约 18 个月，具体取决于你的 Google 账号设置）一并恢复回来，而不仅仅是本地的约 3 个月。

---

## 安装

从 [GitHub Releases](https://github.com/t41372/PathKeep/releases) 下载最新版本。

- **macOS：** 打开 `.dmg`，把 `PathKeep.app` 拖进 `/Applications`，再启动。如果要扫描 Safari 历史，需要先给 PathKeep 授予"完全磁盘访问权限"。
- **Windows：** 安装未签名的 `.msi` 或 `-setup.exe` 安装包。Windows 会提示 `Unknown Publisher`，SmartScreen 可能需要点击 **更多信息 → 仍要运行**，直到 PathKeep 积累出发行商信誉。计划任务备份走 Windows Task Scheduler。当系统缺少 WebView2 时，安装器会启动 WebView2 下载引导，因此首次安装可能需要联网；Windows Server Core / 无头 Server 不在 GUI 验收目标内。
- **Linux：** 正在路上。想现在就试用请从源码构建。~~下载 `.AppImage`、`.deb` 或 `.rpm` 包。Linux 上的计划备份仍处于预览/人工审核阶段，因为桌面 keyring 和 `systemd --user` 的行为因发行版而异。~~

## 卸载

- 在 **系统 → 计划备份设置** 里移除任何已安装的计划任务。
- 退出 PathKeep，删除应用本体，或用系统包管理器卸载。
- 可选清理本地数据：仅在你确实想连归档、配置、审计产物和派生索引一起清掉时，删除 PathKeep 的应用数据目录。macOS 上该目录是 `~/Library/Application Support/com.yi-ting.pathkeep`。

---

## 它具体做了什么

PathKeep 围绕三个功能支柱构建，按优先级从下到上：

```

┌─────────────────────────────────────────────────────┐

│               INTELLIGENCE 智能                      │

│   核心洞察现已可用 · AI 能力进入 v0.3 路线图          │

├─────────────────────────────────────────────────────┤

│               RECALL 回溯                            │

│   全文检索 · 时间线 · 过滤器 · 导出                  │

├─────────────────────────────────────────────────────┤

│               ARCHIVE 归档                           │

│   增量备份 · 计划任务 · 安全 · 导入 · 审计 · 加密     │

└─────────────────────────────────────────────────────┘

```

### Archive 归档

地基。其他一切都建立在一份可信的归档之上。

- **增量备份**——先把数据库复制到暂存区（绝不直接读正在运行的浏览器 DB），归档只增不改，自动去重
- **多浏览器发现**——自动识别已安装的浏览器与配置文件，备份哪些由你选择
- **计划备份**——macOS（`launchd`）和 Windows（Task Scheduler）原生支持安装/查询/移除；Linux `systemd --user` 仍保留为人工审核的预览状态
- **Google Takeout 导入**——预览、导入、回退、恢复、修复一整套流程，全部支持 dry-run
- **加密**——明文或 SQLCipher 加密归档可选，配套改密预览和审计轨迹
- **审计账本**——每一次备份、导入、回滚、恢复都留下不可变的运行记录，清单和产物以哈希链相连
- **回滚**——任何写操作都可逆；面向用户可见的事实采用软隐藏，而非破坏性删除

### Recall 回溯

在多年历史中找回你"见过的那个东西"。

- **全文检索**——基于 FTS5，在 URL、标题、搜索词上做关键词检索
- **正则搜索**——可选的正则模式，针对规范化结果做后置过滤
- **交互式时间线**——按年 → 月 → 日逐层下钻，配合密度可视化和虚拟滚动，能扛住百万级记录
- **复合过滤**——按浏览器、配置文件、域名、时间范围、页面类型、访问来源、导入批次组合过滤
- **导出**——筛选结果可导出为 HTML、Markdown、纯文本或 JSONL

### Intelligence 智能

在扎实的归档之上理解你的浏览模式。**v0.2.0 暂不包含 AI 功能**——PathKeep 在不接入任何 AI 提供商的情况下，也能从本地归档事实中产出价值。

- **确定性洞察**——浏览节律日历热力图、搜索活动、域名深钻、会话、搜索轨迹、查询家族、回访页面、活动构成、周期摘要——全部基于归档事实计算，不依赖 AI
- **语义搜索**——延后到 v0.3 路线图；v0.2.0 只提供关键词和正则两种回溯
- **AI 助手**——延后到 v0.3 路线图，等 provider、检索、证据和锁定状态等环节都经过真值校验后再上
- **MCP 服务器**——延后到 v0.3 路线图；v0.2.0 不会把浏览历史暴露给外部 AI 工具
- **洞察卡片**——主题时间线、任务/线程识别、浏览节律、探索 vs 利用、信息源效用、对比式摘要，全部由本地确定性模型产出
- **远程备份**——面向 S3 兼容存储的"预览 → 手动 → 执行"三段式流程，配套校验和与可恢复性验证

---

## 浏览器支持

PathKeep 严格区分"已实现的适配器"和"公开承诺的支持"。README 只承诺**已经独立验证过**的那一部分。

| 状态       | 浏览器                                                                                                                                |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **已验证** | Google Chrome；Microsoft Edge / Edge Dev；Firefox；ChatGPT Atlas（macOS）；Perplexity Comet（macOS）；Safari（macOS，需完全磁盘访问） |
| **已实现** | Chromium、Brave、Vivaldi、Arc、Opera、Opera GX、LibreWolf、Floorp、Waterfox                                                           |

已实现但未列入公开支持承诺的浏览器，会出现在发现与归档数据里。准入门槛见[适配器手册](./docs/architecture/browser-support-and-adapter-playbook.md)。

---

## 平台支持

| 平台    | 状态 | 备注                                                                        |
| ------- | ---- | --------------------------------------------------------------------------- |
| macOS   | 主力 | 签名 / 公证构建；Touch ID 会话解锁；Safari 支持需完全磁盘访问               |
| Windows | 预览 | 未签名 MSI / NSIS 安装包；`Unknown Publisher` 属于预期；支持 Task Scheduler |
| Linux   | 预览 | 提供 AppImage / `.deb` / `.rpm`；keyring 行为因桌面环境而异                 |

---

## 技术栈

| 层          | 选型                                                             | 选它的理由                           |
| ----------- | ---------------------------------------------------------------- | ------------------------------------ |
| 桌面框架    | Tauri 2                                                          | 跨平台、Rust 内核、轻量              |
| 核心逻辑    | Rust workspace（`vault-core`、`vault-worker`、`vault-platform`） | 高性能、安全、跨平台                 |
| 浏览器解析  | `browser-history-parser`（独立 Rust crate）                      | 可复用、可独立发布                   |
| 前端        | React 19 + TypeScript + Vite                                     | 现代、类型安全                       |
| 工具链      | Bun                                                              | 包管理与脚本                         |
| 主存储      | SQLite（可选 SQLCipher 加密）                                    | 20 年级别耐用性、本地优先            |
| 全文检索    | SQLite FTS5                                                      | 核心回溯，不依赖外部服务             |
| 向量 / 语义 | v0.2.0 暂缓                                                      | 未来作为可替换 sidecar，不进默认构建 |
| AI 推理     | v0.2.0 暂缓                                                      | 未来由用户自行配置 provider          |

---

## 上手

### 前置条件

- [Bun](https://bun.sh/)
- Rust `1.94.1`，包含 `clippy`、`rustfmt`、`llvm-tools-preview`
- Git
- [Tauri 2 平台先决条件](https://v2.tauri.app/distribute/)

Linux（Debian / Ubuntu）开发依赖：

```

sudo apt-get update

sudo apt-get install -y \

pkg-config libglib2.0-dev libgtk-3-dev \

libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \

librsvg2-dev patchelf rpm

```

### 安装与运行

```

bun install

bun run dev              # 仅浏览器端的 Vite 预览（127.0.0.1:1420）

bun run desktop:dev      # 完整 Tauri 桌面应用

```

### 构建

```

bun run build            # TypeScript + Vite 打包

bun run desktop:build    # 桌面 release 包

```

---

## 质量与测试

```

bun run check            # 全部主线质量门

bun run build            # TypeScript + Vite 打包

bun run test:unit        # Vitest 单元测试

bun run test:e2e         # Playwright 端到端测试

bun run coverage:js      # JS 覆盖率门

bun run coverage:rust    # Rust 覆盖率门

bun run mutation:js      # 桌面契约 JS 变异测试门

bun run mutation:js:full # JS 全量变异深扫

bun run mutation:rust    # Rust 全量变异深扫

bun run verify           # check + debug 桌面构建演练

```

完整的门矩阵、深度检查和发版命令见 [TESTING.md](./TESTING.md)。

---

## 文档

| 需求                   | 去哪看                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| 贡献者工作流           | [CONTRIBUTING.md](./CONTRIBUTING.md)                                                                                     |
| 本地环境与仓库布局     | [DEVELOPMENT.md](./DEVELOPMENT.md)                                                                                       |
| 测试面与命令矩阵       | [TESTING.md](./TESTING.md)                                                                                               |
| 发版手册与产物矩阵     | [RELEASE.md](./RELEASE.md)                                                                                               |
| 面向用户的故障排查     | [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)                                                                               |
| 支持与 bug 报告        | [SUPPORT.md](./SUPPORT.md)                                                                                               |
| 浏览器支持与适配器准入 | [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md) |
| 产品愿景、特性与设计   | [docs/](./docs/)                                                                                                         |

---

## 贡献

PathKeep 采用 Conventional Commits、就近测试、文档优先更新。请先看 [CONTRIBUTING.md](./CONTRIBUTING.md)。

---

## 许可证

[GNU General Public License v3.0](./LICENSE)
