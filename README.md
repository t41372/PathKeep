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

PathKeep is a local-first desktop app for long-term browser history archiving and intelligence. Built with Tauri 2, Rust, React 19, TypeScript, Vite, and Bun. Publicly validated support currently covers Google Chrome, ChatGPT Atlas on macOS, Perplexity Comet on macOS, and Safari on macOS with Full Disk Access; additional Chromium / Firefox-family adapters remain implemented but not yet publicly promised.

---

## Why PathKeep

Chrome keeps your browser history locally for only 90 days (or 18 months if synced with Google). Edge does the same. Safari defaults to a year. Almost none of the browsers keep your history forever by defaults — your browsing data expires on their schedule, not yours.

Browser history is not just something that can be thrown away. It's how we think, how we learn, how we get information, how we entertain, and how we live on the internet. For many of us, a significant chunk of our life plays out online. We should be able to keep our own history forever, and get insights out of it. We should own our data without having to live under the rule set of the browsers.

Browsers record **a lot** of information from your browsing history. If Google can analyze your browser history, you should be able to do it too — on your own machine, with full transparency.

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
- **Scheduled backups** — native scheduler support for macOS (`launchd`), Windows (Task Scheduler), and Linux (`systemd --user`)
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

| Status          | Browsers                                                                                                       |
| --------------- | -------------------------------------------------------------------------------------------------------------- |
| **Validated**   | Google Chrome; ChatGPT Atlas (macOS); Perplexity Comet (macOS); Safari (macOS, requires Full Disk Access)      |
| **Implemented** | Chromium, Microsoft Edge, Edge Dev, Brave, Vivaldi, Arc, Opera, Opera GX, Firefox, LibreWolf, Floorp, Waterfox |

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
