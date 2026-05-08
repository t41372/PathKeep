# PathKeep Editorial Atlas — UI Prototype

A design prototype for PathKeep's new visual direction, following the "Editorial Atlas" aesthetic.

## Design Philosophy

This prototype embodies the vision from the Opus design brief:

> **一本你願意在週日下午獨自打開的、關於自己的相簿。**

### Core Principles

1. **Light mode first** — Paper-like warmth (`#FAF7F2`), not cold SaaS white
2. **Serif typography** — Newsreader for titles brings the "human quality" missing from dev tools
3. **Oxblood accent** — `#6B1F2A`, like library leather, not tech-startup amber
4. **Subtle texture** — 2.5% noise overlay for paper feel without obvious grain
5. **Low density, slow rhythm** — This is for browsing memories, not managing tasks

### Visual References

- Are.na / Cosmos / Readwise Reader — archival, contemplative
- iA Writer / Reeder — typographic precision
- Internet Archive — the institutional quality of preservation

## Font Strategy (CJK-friendly, No Bundle Bloat)

**Critical constraint:** Total binary ~10MB, app runs offline.

### Solution: Latin subset + system fallback

| Role | Latin | CJK Fallback |
|------|-------|--------------|
| Serif (titles) | Newsreader Variable (~45KB) | Songti SC, Noto Serif CJK SC, Hiragino Mincho, Batang |
| Sans (UI) | Inter Variable (~90KB) | PingFang SC, Hiragino Sans, Apple SD Gothic Neo |
| Mono (metadata) | JetBrains Mono (~40KB) | System monospace |

**Total font weight: ~175KB** (woff2, Latin subset only)

CJK characters fall through to system fonts, which are:
- macOS: Songti SC, PingFang SC, Hiragino (all pre-installed)
- Windows: SimSun, Microsoft YaHei (pre-installed)
- Linux: Noto CJK fonts (common in CJK locales)

This approach gives us:
- Beautiful Latin typography with Newsreader's editorial voice
- Native-feeling CJK rendering using OS-optimized fonts
- Zero additional download for CJK users
- Works fully offline

### Implementation

Fonts are imported via `@fontsource-variable/*` packages (Latin subset by default).

## Tech Stack

- React 19
- Vite 6
- TypeScript
- Tailwind CSS 3.4
- Lucide React (icons)

## Development

```bash
cd prototype/editorial-atlas
npm install
npm run dev
```

## Structure

```
src/
├── components/
│   ├── sidebar.tsx          # Left nav + browsing intensity + local-first card
│   ├── top-bar.tsx          # Search, date range, filters
│   ├── timeline.tsx         # Day-grouped entry list
│   ├── timeline-entry.tsx   # Individual history entry card
│   ├── detail-panel.tsx     # Right panel with metadata
│   ├── status-bar.tsx       # Bottom archive stats
│   ├── browsing-intensity.tsx
│   └── local-first-card.tsx
├── lib/
│   ├── mock-data.ts         # Multilingual sample data (en/zh-TW/zh-CN/ja/ko)
│   └── utils.ts             # cn() helper
├── index.css                # Tailwind + paper texture + ink-bleed typography
├── App.tsx
└── main.tsx
```

## Design Tokens

Colors defined in `tailwind.config.ts`:

```
paper:      #FAF7F2  (warm off-white background)
paper-card: #F2EDE4  (card surfaces)
ink:        #1A1612  (primary text, warm near-black)
ink-secondary: #6B6157
ink-tertiary:  #9A9186
oxblood:    #6B1F2A  (accent, like library leather)
```

## Next Steps

After prototype approval:

1. Port design tokens to main PathKeep codebase
2. Implement dark mode variant (warm museum lighting, `#1A1714` base)
3. Add real Tauri window chrome integration
4. Connect to actual browser history data
5. Build Contact Sheet view (alternative to timeline)
