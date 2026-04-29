<div align="center">

# PathKeep - Keep the path you've walked

<img alt="demo" src="https://github.com/user-attachments/assets/6118f6f4-80ee-4cd2-bf44-3989d26eb5e5" />

**Stop losing your browsing history**

<a href="https://github.com/t41372/PathKeep/actions/workflows/ci.yml">
  <img alt="CI" src="https://github.com/t41372/PathKeep/actions/workflows/ci.yml/badge.svg" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/release.yml">
  <img alt="Release" src="https://github.com/t41372/PathKeep/actions/workflows/release.yml/badge.svg" />
</a>
<a href="https://github.com/t41372/PathKeep/actions/workflows/mutation.yml">
  <img alt="Mutation" src="https://github.com/t41372/PathKeep/actions/workflows/mutation.yml/badge.svg" />
</a>
<a href="https://github.com/t41372/PathKeep/releases">
  <img alt="Latest release" src="https://img.shields.io/github/v/release/t41372/PathKeep?display_name=tag" />
</a>
<a href="./LICENSE">
  <img alt="License: GPL-3.0" src="https://img.shields.io/badge/license-GPL--3.0-1f6feb.svg" />
</a>

</div>

<br/>

PathKeep is a local-first desktop app for long-term browser history archiving and intelligence. Built with Tauri 2, Rust, React 19, TypeScript, Vite, and Bun. Publicly validated support currently covers Google Chrome, Microsoft Edge, Firefox, ChatGPT Atlas on macOS, Perplexity Comet on macOS, and Safari on macOS with Full Disk Access; additional Chromium / Firefox-family adapters remain implemented but not yet publicly promised.

---

## Why PathKeep

### Your browser is silently deleting your history

Most people assume their browsing history is always there when they need it. **It isn't.**

Every Chromium-based browser — Chrome, Edge, Brave, Arc, Opera, and others — **automatically deletes local browsing history after approximately 90 days**. This isn't a bug or a setting you accidentally turned on; it's [hardcoded in the Chromium source](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc). Safari caps local history at [one year](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac). Even Firefox, which is the most generous among major browsers, eventually prunes history based on [database size](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs).

Cloud sync doesn't save you either. Firefox Sync only uploads the [last 30 days and expires synced history after 60 days](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs). Brave Sync only covers [URLs you manually typed after linking devices](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ). Arc Sync [doesn't sync history at all](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync). "Syncing" your browser is not a backup — it's a convenience feature with a short memory.

**If you use Chrome and haven't taken action, chances are, every piece of browsing history older than three months is already gone.** The pages you visited last year, the searches you ran during that important moment in your life, the rabbit holes you went down while learning something new — all silently erased.

<details><summary><b>📋 How long does your browser actually keep history? (reference table)</b></summary>

Data sourced from official documentation, help centers, and source code as of April 2026. "Cloud sync" refers to the browser's own first-party sync service, not OS-level backups.

| Browser              | Engine         | Local Default                                   | Local Max                                                                    | Cloud Sync                                                                        | Effective Max                             |
| -------------------- | -------------- | ----------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------- |
| **Google Chrome**    | Chromium       | ~90 days [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                                        | No user setting to extend                                                    | Google Account may retain longer to about 18 months [[2]](https://support.google.com/chrome/answer/165139)         | ~90 days locally                          |
| **Microsoft Edge**   | Chromium       | ~90 days [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                                        | Enterprise policy can _shorten_ only [[7]](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/browsingdatalifetime)                                         | Privacy dashboard: up to 180 days [[5]](https://support.microsoft.com/en-us/microsoft-edge/view-and-delete-browser-history-in-microsoft-edge-00cf7943-a9e1-975a-a33d-ac10ce454ca4)                                                 | ~90 days locally; ~180 days via dashboard |
| **Brave**            | Chromium       | ~90 days [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                                        | No keep-forever setting found                                                | Only typed URLs after joining Sync chain; server deletes after 12 months inactive [[8]](https://support.brave.com/hc/en-us/articles/360047642371-Sync-FAQ) | ~90 days locally                          |
| **Arc**              | Chromium       | ~90 days (inferred) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                             | No keep-forever setting found                                                | **Does not sync history** [[12]](https://resources.arc.net/hc/en-us/articles/20272860828823-Arc-Sync)                                                         | ~90 days locally                          |
| **Opera / Opera GX** | Chromium       | ~90 days [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                                        | No keep-forever setting found                                                | Syncs history, but no published cloud TTL [[17]](https://help.opera.com/en/latest/features/#sync)                                         | ~90 days locally                          |
| **Vivaldi**          | Chromium       | 3 months [[9]](https://help.vivaldi.com/desktop/navigation/history/)                                        | **Forever** (user setting) [[9]](https://help.vivaldi.com/desktop/navigation/history/)                                                   | Syncs history, but no published cloud TTL [[10]](https://help.vivaldi.com/desktop/tools/sync/)                                         | Forever (if configured)                   |
| **Dia**              | Chromium-based | Undisclosed (likely ~90 days) [[1]](https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc)                   | No keep-forever setting found                                                | E2E encrypted sync exists, but no published history TTL [[28]](https://diabrowser.com/privacy)                           | Undisclosed                               |
| **Safari**           | WebKit         | Up to **1 year** [[14]](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac)                                | 1 year (Apple's published max) [[14]](https://support.apple.com/guide/safari/search-your-web-browsing-history-ibrw1114/mac)                                               | iCloud syncs history across devices, but no published cloud TTL [[15]](https://support.apple.com/guide/icloud/what-you-can-do-with-icloud-and-safari-mm9b8da4f328/icloud)                   | 1 year                                    |
| **Firefox**          | Gecko          | No fixed day limit; capacity-driven pruning [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)     | Configurable via `places.history.expiration.max_pages`; can retain **years** [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs) | Only last 30 days uploaded; expires after 60 days in cloud [[19]](https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs)                        | Years locally; ~60 days via Sync          |
| **LibreWolf**        | Firefox-based  | Firefox-like (unless clear-on-close is enabled) [[21]](https://librewolf.net/docs/faq/) | Same as Firefox [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                                                              | No first-party cloud; Firefox Sync available manually [[21]](https://librewolf.net/docs/faq/)                             | Firefox-like                              |
| **Floorp**           | Firefox-based  | Likely Firefox-like [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                             | Not documented                                                               | Not documented                                                                    | Likely Firefox-like                       |
| **Waterfox**         | Firefox-based  | Likely Firefox-like [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                             | Not documented                                                               | Not documented                                                                    | Likely Firefox-like                       |
| **Zen Browser**      | Firefox-based  | Likely Firefox-like [[18]](https://searchfox.org/firefox-main/source/toolkit/components/places/PlacesExpiration.sys.mjs)                             | Not documented                                                               | Same-device window sync only; no cross-device history sync found [[27]](https://docs.zen-browser.app/user-manual/window-sync)                  | Likely Firefox-like locally               |
| **ChatGPT Atlas**    | Undisclosed    | Undisclosed                                     | Undisclosed                                                                  | Web History exists (with "All time" delete range), but no published TTL [[30]](https://help.openai.com/en/articles/12625059-web-browsing-settings-on-chatgpt-atlas)           | Undisclosed                               |
| **Perplexity Comet** | Undisclosed    | Undisclosed                                     | Undisclosed                                                                  | Account-level search history (not full browser history); no published TTL [[32]](https://comet-help.perplexity.ai/en/articles/12658050-log-in-to-perplexity)         | Undisclosed                               |

**Key takeaway:** Unless you use Firefox (capacity-driven, not time-driven) or have manually configured Vivaldi to "Forever", your browser is almost certainly deleting history on a schedule you never agreed to.

The research was conducted on April 21, 2026. May contain mistakes. Open an issue or pr if corrections need to be made.

</details>

### Your browsing history is worth more than you think

You might wonder: _does it actually matter?_

Think about what browser history really is. It's a record of how you learned things, how you researched a major decision, what you were reading the week something changed your life. It captures what you cared about as a student, before you took that job, before you moved cities, before things were different. For many of us, a significant chunk of life plays out online — and browser history is the closest thing we have to a journal of that experience.

Those years of history aren't gone because you chose to delete them. They're gone because your browser decided they weren't worth keeping.

_Until recently, it didn't matter much that browsers deleted old history_, because there was no practical way to extract meaning from literally tens of millions of raw URLs anyway. But that's no longer true. Local AI inference is now fast and cheap enough to run agentic analysis on years of browsing data, right on your own machine. Concepts like Andrej Karpathy's ["LLM Knowledge Bases"](https://x.com/karpathy/status/2039805659525644595) — where LLMs compile and maintain a personal knowledge base — are becoming reality.

The question is no longer whether you can extract meaning from decades of history. The question is whether you’ll still have the data when that future arrives.

**The data you save today is the raw material for intelligence tomorrow.** But you can't analyze what you've already lost.

### What PathKeep does about it

PathKeep runs quietly on your machine and **incrementally backs up browsing history from all your browsers** — automatically, on a schedule, without manual effort. It never reads live browser databases directly; instead it stages safe copies, deduplicates, and appends to a local archive that you fully own and control.

On top of that archive, PathKeep gives you powerful recall (full-text search, regex, timeline, filters, export) and optional AI-powered intelligence (semantic search, natural-language Q&A, insight cards) — all running locally, all off by default, all under your control.

> Use Chrome with Google Sync on? PathKeep supports **Google Takeout import** to recover up to 18 months of history instead of just 3.

---

## Installation

## Uninstall

---

## What It Does

PathKeep is organized around three functional pillars, built in order of priority:

```
┌─────────────────────────────────────────────────────┐
│               INTELLIGENCE                          │
│   Core insights · Semantic search · AI assistant   │
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
- **Interactive timeline** — year → month → day drill-down with density visualization and virtual scrolling for millions of records
- **Composite filters** — by browser, profile, domain, time range, page type, visit source, or import batch
- **Export** — filtered result sets exportable to HTML, Markdown, plain text, or JSONL

### Intelligence

Understanding your browsing patterns, built on top of a solid archive. **All AI features are off by default** — PathKeep works fully without any AI provider.

- **Deterministic insights** — browsing rhythm calendar heatmap, search activity, domain deep-dive, sessions, search trails, query families, refind pages, activity mix, and periodic summaries — all computed from archive facts, no AI required
- **Semantic search** — embedding-based vector similarity via LanceDB sidecar and rig.rs, with honest fallback to keyword recall when unavailable
- **AI assistant** — ask questions about your browsing history in natural language; agentic RAG retrieval with evidence citations
- **MCP server** — expose your history to external AI tools (Cursor, Copilot, Gemini CLI, etc.) via a localhost-only Model Context Protocol server
- **Insight cards** — topic timelines, task/thread detection, browsing rhythm, explore-vs-exploit patterns, source effectiveness, and contrastive summaries
- **Remote backup** — Preview → Manual → Execute flow for S3-compatible remote bundles, with checksum and restore-readiness verification

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

| Platform | Status  | Notes                                                                                        |
| -------- | ------- | -------------------------------------------------------------------------------------------- |
| macOS    | Primary | Signed / notarized builds; Touch ID session unlock; Safari support requires Full Disk Access |
| Windows  | Preview | MSI / NSIS builds available; code signing is maintainer-operated                             |
| Linux    | Preview | AppImage / `.deb` / `.rpm` builds available; keyring behavior varies by desktop environment  |

---

## Tech Stack

| Layer             | Choice                                                          | Why                                         |
| ----------------- | --------------------------------------------------------------- | ------------------------------------------- |
| Desktop framework | Tauri 2                                                         | Cross-platform, Rust core, lightweight      |
| Core logic        | Rust workspace (`vault-core`, `vault-worker`, `vault-platform`) | High performance, safe, cross-platform      |
| Browser parsing   | `browser-history-parser` (standalone Rust crate)                | Reusable, community-publishable parser      |
| Frontend          | React 19 + TypeScript + Vite                                    | Modern, type-safe                           |
| Toolchain         | Bun                                                             | Package management and scripts              |
| Canonical storage | SQLite (optional SQLCipher encryption)                          | 20-year durability, local-first             |
| Full-text search  | SQLite FTS5                                                     | Core recall, no external service            |
| Vector / semantic | LanceDB sidecar + rig.rs                                        | Embedded, Rust-native, disk-based ANN index |
| AI inference      | Local (Ollama / LM Studio) or cloud API                         | Optional, user-configured                   |

---

## Getting Started

### Prerequisites

- [Bun](https://bun.sh)
- Rust `1.94.1` with `clippy`, `rustfmt`, and `llvm-tools-preview`
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
bun run dev              # Browser-only Vite preview (127.0.0.1:1420)
bun run desktop:dev      # Full Tauri desktop app
```

### Build

```bash
bun run build            # TypeScript + Vite bundle
bun run desktop:build    # Release desktop bundle
```

---

## Quality & Testing

```bash
bun run check            # All mainline quality gates
bun run build            # TypeScript + Vite bundle
bun run test:unit        # Vitest unit tests
bun run test:e2e         # Playwright end-to-end tests
bun run coverage:js      # JS coverage gate
bun run coverage:rust    # Rust coverage gate
bun run mutation:js      # Desktop-contract JS mutation gate
bun run mutation:js:full # Full JS mutation deep sweep
bun run mutation:rust    # Full Rust mutation deep sweep
bun run verify           # check + debug desktop build rehearsal
```

For the full gate matrix, deep checks, and release signoff commands, see [TESTING.md](./TESTING.md).

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
