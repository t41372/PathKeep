<div align="center">

<img alt="PathKeep" src="docs/design/brand-icon/source/icon-1024.png" width="112" />

# PathKeep - Keep the path you've walked

<img alt="Screenshot 2026-07-19 at 5 16 16 PM" src="https://github.com/user-attachments/assets/e587cee0-a68b-4c9b-97c7-acc31b27d4d0" />

**Stop losing your browsing history**

<a href="https://github.com/t41372/PathKeep/actions/workflows/ci.yml">
  <img alt="CI" src="https://github.com/t41372/PathKeep/actions/workflows/ci.yml/badge.svg?branch=main&event=push" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/native-deps.yml">
  <img alt="Native Dependencies" src="https://github.com/t41372/PathKeep/actions/workflows/native-deps.yml/badge.svg?branch=main" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/release.yml">
  <img alt="Release" src="https://github.com/t41372/PathKeep/actions/workflows/release.yml/badge.svg?branch=main" />
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

[中文](./README.CN.md) | English

</div>

<br/>

PathKeep is a local-first desktop app for long-term archiving and intelligence for browser history. Built with Tauri 2, Rust, React 19, TypeScript, Vite, and Bun. Publicly validated support currently covers Google Chrome, Microsoft Edge, Firefox, ChatGPT Atlas on macOS, Perplexity Comet on macOS, and Safari on macOS with Full Disk Access; additional Chromium / Firefox-family adapters remain implemented but not yet publicly promised.

---

## Why PathKeep

### Your browser is silently deleting your history

Most people assume their browsing history is always there when they need it. **It isn't.**

Most Chromium-based browsers — including Chrome, Edge, Brave, and Arc — inherit an upstream default that **automatically deletes local browsing history older than approximately 90 days**. This isn't a bug; it's [hardcoded in the Chromium source](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc). Safari only keeps local history for [one year by default](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac) unless you manually reconfigure it. Even Firefox, which is more generous, eventually prunes history based on [database size](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs).

Cloud sync doesn't save you either. Syncing is designed for multi-device convenience, not as a long-term archive. Firefox Sync only uploads the [last 30 days and expires synced history after 60 days](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs). Brave Sync only covers [URLs you manually typed](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ). Arc [doesn't sync history at all](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync).

**If you rely entirely on Chrome and never manually back up your browsing history, chances are high that your local browsing history older than three months is already gone.** The pages you visited last year, the searches you ran during that important moment in your life, the rabbit holes you went down while learning something new — all silently erased from your machine.

<details><summary><b>📋 How long does your browser actually keep history? (reference table)</b></summary>

Data sourced from official documentation, help centers, and source code as of April 2026. "Cloud sync" refers to the browser's first-party sync service. Retention often depends on user settings; defaults are shown below.

| Browser              | Engine         | Local Default                                                                                                                                  | Local Max                                                                                                                                               | Cloud Sync                                                                                                                                                                                                    | Effective Max                        |
| -------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| **Google Chrome**    | Chromium       | ~3 months (Chromium default) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | No built-in setting found                                                                                                                               | Google Account may retain longer depending on Web & App Activity settings [[2]](https://support.google.com/chrome/answer/165139)                                                                              | ~3 months locally; longer if synced  |
| **Microsoft Edge**   | Chromium       | ~3 months (Chromium default) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | Enterprise policy can _shorten_ only [[7]](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/browsingdatalifetime)           | Privacy dashboard retains some synced data; TTL undocumented [[5]](https://support.microsoft.com/en-us/microsoft-edge/view-and-delete-browser-history-in-microsoft-edge-00cf7943-a9e1-975a-a33d-ac10ce454ca4) | ~3 months locally; cloud TTL unknown |
| **Brave**            | Chromium       | ~3 months (Chromium default) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | No keep-forever setting found                                                                                                                           | Only typed URLs; server deletes after 12 months inactive [[8]](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ)                                                                             | ~3 months locally                    |
| **Arc**              | Chromium       | ~3 months (Chromium default) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | No keep-forever setting found                                                                                                                           | **Does not sync history** [[12]](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync)                                                                                                         | ~3 months locally                    |
| **Opera / Opera GX** | Chromium       | ~3 months (Chromium default) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc) | No keep-forever setting found                                                                                                                           | Syncs history, but cloud TTL is undocumented [[17]](https://help.opera.com/en/latest/features/#sync)                                                                                                          | ~3 months locally                    |
| **Vivaldi**          | Chromium       | 3 months [[9]](https://help.vivaldi.com/desktop/navigation/history/)                                                                           | **Forever** (user setting) [[9]](https://help.vivaldi.com/desktop/navigation/history/)                                                                  | Syncs history, but cloud TTL is undocumented [[10]](https://help.vivaldi.com/desktop/tools/sync/)                                                                                                             | Forever (if configured)              |
| **Dia**              | Chromium-based | Undisclosed (likely ~3 months)                                                                                                                 | No keep-forever setting found                                                                                                                           | E2E encrypted sync exists, but no published history TTL [[28]](https://diabrowser.com/privacy)                                                                                                                | Undisclosed                          |
| **Safari**           | WebKit         | 1 year (default) [[14]](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac)                                  | Configurable (no hard cap found)                                                                                                                        | iCloud syncs history across devices; cloud TTL undocumented [[15]](https://support.apple.com/guide/icloud/what-you-can-do-with-icloud-and-safari-mm9b8da4f328/icloud)                                         | Configurable                         |
| **Firefox**          | Gecko          | Capacity-driven (no fixed day limit) [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)      | Configurable via `max_pages`; can retain **years** [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs) | Only last 30 days uploaded; expires after 60 days in cloud [[19]](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs)                                                          | Years locally; ~60 days via Sync     |
| **LibreWolf**        | Firefox-based  | Firefox-like (unless clear-on-close is on) [[21]](https://librewolf.net/docs/faq/)                                                             | Same as Firefox [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                                    | Firefox Sync available manually [[21]](https://librewolf.net/docs/faq/)                                                                                                                                       | Firefox-like                         |
| **Floorp**           | Firefox-based  | Likely Firefox-like                                                                                                                            | Not documented                                                                                                                                          | Not documented                                                                                                                                                                                                | Likely Firefox-like                  |
| **Waterfox**         | Firefox-based  | Likely Firefox-like                                                                                                                            | Not documented                                                                                                                                          | Not documented                                                                                                                                                                                                | Likely Firefox-like                  |
| **Zen Browser**      | Firefox-based  | Likely Firefox-like                                                                                                                            | Not documented                                                                                                                                          | Supports Firefox Sync for history; Window Sync is local only [[27]](https://docs.zen-browser.app/user-manual/window-sync)                                                                                     | Likely Firefox-like locally          |
| **ChatGPT Atlas**    | Chromium       | Undisclosed (likely ~3 months)                                                                                                                 | Undisclosed                                                                                                                                             | Web History controls exist; cloud TTL undocumented [[30]](https://help.openai.com/en/articles/12625059-web-browsing-settings-on-chatgpt-atlas)                                                                | Undisclosed                          |
| **Perplexity Comet** | Chromium-based | Local history stored; TTL undocumented                                                                                                         | Undisclosed                                                                                                                                             | E2E encrypted sync for history; cloud TTL undocumented [[32]](https://comet-help.perplexity.ai/en/articles/12658050-log-in-to-perplexity)                                                                     | Undisclosed                          |

**Key takeaway:** Unless you use Firefox (capacity-driven) or have manually configured browsers like Safari or Vivaldi, your browser is almost certainly deleting history on a schedule you never agreed to.

_Note: The ~3 month Chromium limit is inferred from the upstream Chromium source code. While downstream browsers can alter this, most do not document a different retention policy._

</details>

### Your browsing history is worth more than you think

You might wonder: _does it actually matter?_

Think about what browser history really is. It's a record of how you learned things, how you researched a major decision, what you were reading the week something changed your life. It captures what you cared about as a student, before you took that job, before you moved cities, before things were different. For many of us, a significant chunk of life plays out online — and browser history is the closest thing we have to a journal of that experience.

Those years of history are gone, not because you have chosen to delete them. They're gone because your browser decided they weren't worth keeping.

_Until recently, it didn't matter much that browsers delete old history_, because there was no practical way to extract meaning from literally tens of millions of raw URLs anyway. But that's no longer true. Local AI inference is now fast and cheap enough to run agentic analysis on years of browsing data, right on your own machine. Concepts like Andrej Karpathy's ["LLM Knowledge Bases"](https://x.com/karpathy/status/2039805659525644595) — where LLMs compile and maintain a personal knowledge base — are becoming reality.

The question is no longer whether you can extract meaning from decades of history. The question is whether you’ll still have the data when that future arrives.

**The data you save today is the raw material for intelligence tomorrow.** But you can't analyze what you've already lost.

### What PathKeep does about it

PathKeep runs quietly on your machine and **incrementally backs up browsing history from all your browsers** on a schedule. On macOS and Windows, scheduled backup installs and runs automatically; on Linux, you generate the systemd files in-app and enable the timer manually. If your archive is encrypted, save the password to the system keychain so the background worker can unlock it — otherwise scheduled runs will fail silently. PathKeep never reads live browser databases directly; instead, it stages safe copies, deduplicates, and appends to a local archive that you fully own and control.

On top of that archive, PathKeep gives you powerful recall (full-text search, regex, timeline, filters, export) and deterministic Core Intelligence from local archive facts. Semantic search, the Ask assistant and an MCP server are there too; they are optional and off until you turn them on.

> Use Chrome with Google Sync enabled? PathKeep supports **Google Takeout import**, letting you recover extended history (often ~18 months, depending on your Google account settings) instead of just the local ~3 months.

---

## Installation

Download the latest release from [GitHub Releases](https://github.com/t41372/PathKeep/releases).

- **macOS:** open the `.dmg`, move `PathKeep.app` to `/Applications`, then open it. Backing up Safari requires granting Full Disk Access to PathKeep before it can read Safari history.
- **Windows:** install the unsigned `.msi` or `-setup.exe` release package. Windows will show `Unknown Publisher`, and SmartScreen may require **More info -> Run anyway** until PathKeep has publisher reputation. Scheduled backups use Windows Task Scheduler. The installer uses the WebView2 download bootstrapper when the runtime is missing, so first install may need internet access; Windows Server Core / headless Server environments are not valid GUI acceptance hosts.
- **Linux:** Support is on the way. Build from source if you want to try it out right now. ~~install the `.AppImage`, `.deb`, or `.rpm` artifact. Linux scheduled-backup support is still preview/manual-review because desktop keyring and `systemd --user` behavior varies by distribution.~~

## Uninstall

- Remove the installed schedule by setting **Backup → Automatic backup** to **Off**. (Settings → Storage → Delete all data does not remove it.)
- Quit PathKeep, then delete the app or uninstall it with your OS package manager.
- Optional local data removal: delete the PathKeep app-data directory only if you also want to remove the archive, config, audit artifacts, and derived indexes. On macOS that directory is `~/Library/Application Support/com.yi-ting.pathkeep`.

---

## What It Does

PathKeep is organized around three functional pillars, built in order of priority:

```
┌─────────────────────────────────────────────────────┐
│               INTELLIGENCE                          │
│   Local insights · Optional AI (Ask, semantic)     │
├─────────────────────────────────────────────────────┤
│               RECALL                                │
│   Full-text search · Timeline · Filters · Export   │
├─────────────────────────────────────────────────────┤
│               ARCHIVE                               │
│   Incremental backup · Schedule · Security ·        │
│   Import · Audit · Encryption                      │
└─────────────────────────────────────────────────────┘
```

### Archive

The foundation. Everything else depends on a trustworthy archive.

- **Incremental backup** — staged database copies (never reads live browser DBs), append-only archive, automatic deduplication
- **Multi-browser discovery** — auto-detects installed browsers and profiles; you choose which to back up
- **Scheduled backups** — native install / status / remove support for macOS (`launchd`) and Windows (Task Scheduler), with Linux `systemd --user` preview kept manual-review
- **Google Takeout import** — preview, import, revert, restore, and repair flows with full dry-run
- **Encryption** — plaintext or SQLCipher-encrypted archive, with re-key preview and audit trail
- **Audit ledger** — every backup, import, rollback, and restore leaves an immutable run record with manifests and artifacts forming a hash chain
- **Rollback** — any write operation is reversible; user-visible facts use soft-hide visibility, not destructive delete

### Recall

Finding what you've seen before, across years of history.

- **Full-text search** — FTS5-powered keyword search across URLs, titles, and search terms
- **Regex search** — optional regex mode for advanced pattern matching, post-filtered on canonical results
- **Timeline** — every visit grouped by day and browsing session, in a virtualized list that stays fast with millions of records
- **Filters** — by date (presets or a custom range), browser and site
- **Starred pages and notes** — star a page or a whole site, and write a note you can search later
- **Export** — your whole history to HTML, Markdown, plain text, or JSONL

### Intelligence

Understanding your browsing patterns, built on top of a solid archive. Everything here works without an AI provider.

- **Insights** — page visits, distinct sites, searches and active time over 7 days to a year; daily activity, top sites, a weekday × hour rhythm, frequent searches, and pages you keep reopening — all computed locally from archive facts
- **Home** — a year calendar heatmap, On this day, and topics you keep coming back to
- **Semantic search** — optional; downloads a small model once and builds the index on your computer
- **Ask** — optional chat over your history, with a local model (Ollama, LM Studio) or a provider you configure; answers cite the pages they used
- **MCP server** — optional, read-only, localhost only, off by default

---

## Browser Support

PathKeep separates implemented adapters from publicly validated support. The README only promises what has been independently verified.

| Status          | Browsers                                                                                                                                      |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Validated**   | Google Chrome; Microsoft Edge / Edge Dev; Firefox; ChatGPT Atlas (macOS); Perplexity Comet (macOS); Safari (macOS, requires Full Disk Access) |
| **Implemented** | Chromium, Brave, Vivaldi, Arc, Opera, Opera GX, LibreWolf, Floorp, Waterfox                                                                   |

Implemented browsers appear in discovery and archive data but are not yet in the public support promise. See the [adapter playbook](./docs/architecture/browser-support-and-adapter-playbook.md) for the promotion gate.

---

## Platform Support

| Platform | Status  | Notes                                                                                       |
| -------- | ------- | ------------------------------------------------------------------------------------------- |
| macOS    | Primary | Signed / notarized builds; Safari support requires Full Disk Access                         |
| Windows  | Preview | Unsigned MSI / NSIS installers; `Unknown Publisher` is expected; Task Scheduler supported   |
| Linux    | Preview | AppImage / `.deb` / `.rpm` builds available; keyring behavior varies by desktop environment |

---

## Tech Stack

| Layer             | Choice                                                          | Why                                           |
| ----------------- | --------------------------------------------------------------- | --------------------------------------------- |
| Desktop framework | Tauri 2                                                         | Cross-platform, Rust core, lightweight        |
| Core logic        | Rust workspace (`vault-core`, `vault-worker`, `vault-platform`) | High performance, safe, cross-platform        |
| Browser parsing   | `browser-history-parser` (standalone Rust crate)                | Reusable, community-publishable parser        |
| Frontend          | React 19 + TypeScript + Vite                                    | Modern, type-safe                             |
| Toolchain         | Bun                                                             | Package management and scripts                |
| Canonical storage | SQLite (optional SQLCipher encryption)                          | 20-year durability, local-first               |
| Full-text search  | SQLite FTS5                                                     | Core recall, no external service              |
| Frontend UI       | shadcn/ui + Tailwind CSS 4, EvilCharts (Recharts)               | Accessible primitives, one token set          |
| Vector / semantic | Own flat vector index in a rebuildable sidecar                  | Optional, off by default, no external service |
| AI inference      | Ollama / LM Studio or your own provider; built-in embeddings    | Optional, user-configured                     |

---

## Getting Started

### Prerequisites

- [Bun](https://bun.sh)
- Rust `1.94.1` with `clippy` and `rustfmt`
- Git
- [Tauri 2 platform prerequisites](https://v2.tauri.app/distribute/)

Linux (Debian / Ubuntu) development packages:

```bash
sudo apt-get update
sudo apt-get install -y \
  pkg-config libglib2.0-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf rpm
```

### Install & Run

```bash
bun install
bun run desktop:dev      # Full Tauri desktop app
bun run dev:demo         # Real backend + synthetic browser data, open 127.0.0.1:1420 in a browser
                         # (--first-run --fresh starts on onboarding)
```

### Build

```bash
bun run build            # TypeScript + Vite bundle
bun run desktop:build    # Release desktop bundle
```

---

## Quality & Testing

```bash
bun run check            # Lint, build, Rust checks, and the E2E suite
bun run build            # TypeScript + Vite bundle
bun run test:e2e         # E2E against the real Rust backend; artifacts in artifacts/e2e/
bun run test:unit        # Vitest (few tests, E2E is the primary mechanism)
bun run verify           # check + debug desktop build rehearsal
```

For the gate definition, E2E artifacts, and release signoff commands, see [TESTING.md](./TESTING.md).

---

## Documentation

| What you need                         | Where to look                                                                                                            |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Contributor workflow                  | [CONTRIBUTING.md](./CONTRIBUTING.md)                                                                                     |
| Local environment and repo layout     | [DEVELOPMENT.md](./DEVELOPMENT.md)                                                                                       |
| Test surfaces and command matrix      | [TESTING.md](./TESTING.md)                                                                                               |
| Release runbook and artifact matrix   | [RELEASE.md](./RELEASE.md)                                                                                               |
| User-facing troubleshooting           | [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)                                                                               |
| Support and bug-report expectations   | [SUPPORT.md](./SUPPORT.md)                                                                                               |
| Browser support and adapter promotion | [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md) |
| Product vision, features, and design  | [docs/](./docs/)                                                                                                         |

---

## Contributing

PathKeep uses Conventional Commits, colocated tests, and doc-first updates. Start with [CONTRIBUTING.md](./CONTRIBUTING.md).

---

## License

[GNU General Public License v3.0](./LICENSE)
