# Screen 07 — Settings

> The bindery. Where the book is bound, the paper chosen, the ink mixed. Every preference treated as a deliberate craft choice.

---

## Purpose

A calm, scannable preferences screen that reinforces the local-first, editorial values of PathKeep. No tabs — a single long, sectioned page like a typeset manual.

---

## Layout

Two columns inside the main canvas (left rail with `Settings` active stays as always):

- **Left sub-nav (200px, sticky)** — a vertical anchor list of section names. Acts as a table of contents.
- **Main column (~700px wide, centered with max-width)** — single scrolling document of settings cards.

The right rail is hidden on this screen (Settings does not need a contextual panel). The canvas widens.

### Sub-nav items

1. General
2. Local-First & Storage
3. Capture
4. Privacy
5. Intelligence (AI)
6. Sync (Optional)
7. Appearance
8. Keyboard Shortcuts
9. About

The active section is marked with a 2px claret left-border and a deep-ink label; inactive items are `--text-soft`.

---

## Section anatomy

Each section is a **stacked card list** with hairline dividers between rows; not boxed, not zebra-striped. Section header in `font-serif` 22px with a thin 1px `--border-hairline` underline.

### Row pattern

```
[ Label (sans 14px, weight 500) ]            [ Control ]
[ Help text (sans 13px, --text-soft) ]
```

Controls right-aligned. Help text wraps below label. 56px+ per row.

---

## Section content (samples)

### General

- **Display name** → text input `Yan`
- **Default landing screen** → segmented `Timeline · Dashboard · Search`
- **Default date range** → dropdown `Last 7 days`

### Local-First & Storage

(One of the most prominent sections — given top placement.)

- **Storage location** → path display `/Users/yan/Library/PathKeep` + `Reveal in Finder` button
- **Database size** → `1.3 GB` (with a thin claret usage bar showing `1.3 / 50 GB allocated`)
- **Page archiving** → toggle on, with sub-option `Compress after 30 days`
- **Data retention** → dropdown `Keep forever`
- **Export full archive** → button `Export as .pathkeep` (single-file portable archive)
- **Reset & re-index** → ghost button (claret on hover)

A small inset card at the top of this section reads:
> *"PathKeep keeps a complete copy of your reading on this device. Sync is opt-in, never required, and never the source of truth."*

### Capture

- **Browser extensions** → list of installed integrations (Chrome, Arc, Safari) with status dots
- **Auto-capture rules** → list of patterns (e.g. exclude domains, capture only over 30s reading time)
- **Block sources** → a small chip cloud of muted domains
- **Save snippets shortcut** → key recorder showing `⌘ ⇧ S`

### Privacy

- **Encryption at rest** → toggle, default on, with `Change passphrase` link
- **Telemetry** → toggle, default off, with line "We collect zero analytics. This switch does nothing — it is here so you can verify."
- **Network access** → status row showing PathKeep is in *offline mode* with a faint green dot

### Intelligence (AI)

- **Local model** → status `llama-3.2-3b · 1.8 GB · downloaded`
- **Use cloud models for summaries** → toggle, off by default, with explanatory copy about data leaving the device
- **Auto-summarize new pages** → toggle
- **Auto-extract tags** → toggle
- **Embedding index** → status `up to date · 7,842 vectors`

### Sync (Optional)

- A subdued grey card explaining sync is optional, with connect buttons for `iCloud Drive · Syncthing · Dropbox · Custom WebDAV`. Currently `Synced never (local only)`.

### Appearance

- **Theme** → segmented `Light · Dark · Auto`
- **Density** → segmented `Comfortable · Compact`
- **Accent colour** → swatch row of 4 muted choices (Claret default, Olive, Slate, Ink-Blue) — note: kept understated
- **Display fonts** → dropdown showing the current pair (Editorial New + Söhne) with a small preview line

### Keyboard Shortcuts

- A two-column table: action / keystroke, in mono. ~20 rows.

### About

- Version `0.4.2 (private beta)` in mono
- Build hash, last update, license
- A small line: *"Made with care, in the open. github.com/pathkeep"*

---

## Visual texture

Settings is the **calmest** screen in the app. No gradients, no avatars, no images other than three small icon plates: a folder on a desk (Local-First section), an unplugged ethernet cable (Privacy section), and a quill ink bottle (Appearance section), each rendered as 64px hand-line drawings in claret on cream — placed at the top-right of their respective section header.

---

## Why this works

Settings is where trust is either built or broken. By giving Local-First & Storage prominence, by writing telemetry copy that admits the toggle does nothing, and by keeping the visual language of a typeset manual, the page makes a non-trivial promise legible: *the data is yours, here, and PathKeep would rather show you the file path than spin a logo.*
