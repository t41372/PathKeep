---
design:
  meta:
    name: PathKeep — Editorial Atlas
    version: 0.2.0-prototype
    status: prototype
    summary: >-
      A light-first, paper-textured editorial aesthetic for a local-first
      browsing-history archive. The app should feel less like a developer
      tool and more like a private library of one's own digital life — a
      contact sheet for the years one has spent online.
    primary_metaphor: contact-sheet
    voice: quiet, archival, considered, slightly literary
    do_not_resemble:
      - Linear
      - Vercel dashboard
      - Raycast
      - Generic Tailwind admin templates
    aspires_toward:
      - Are.na
      - iA Writer
      - Readwise Reader
      - Internet Archive
      - Folger Shakespeare Library
      - craigmod.com
      - Reeder 5

  themes:
    default: light
    secondary: dark
    light_mode_intent: >-
      Reading-room atmosphere. Unbleached paper, ink-warm blacks, oxblood
      accent. Used as the default because the emotional core is "reading
      one's own past", not "operating a control panel".
    dark_mode_intent: >-
      Darkroom under a single warm lamp. Espresso-black paper with cream
      ink. Never pure-black, never cool-grey. Reserved for late-night
      browsing of one's archive, never for "developer mode" feel.

  color:
    light:
      bg-paper:        "#FAF7F2"  # unbleached paper, app background
      bg-leaf:         "#F2EDE4"  # second surface, side panels, cards
      bg-deckle:       "#EDE6D8"  # third surface, hover, raised chips
      bg-vellum:       "#FFFEFB"  # highest surface, modals, search overlay
      ink-primary:     "#1A1612"  # printing ink — body text, titles
      ink-secondary:   "#3F3730"  # quieter copy, card body
      ink-muted:       "#6B6157"  # metadata, captions
      ink-faint:       "#9A9085"  # placeholder, disabled, dividers
      hairline:        "rgba(26, 22, 18, 0.10)"
      hairline-strong: "rgba(26, 22, 18, 0.18)"
      accent-oxblood:        "#7B1F2A"  # the single brand accent
      accent-oxblood-hover:  "#931F2D"
      accent-oxblood-press:  "#5E1620"
      accent-oxblood-tint:   "rgba(123, 31, 42, 0.10)"
      accent-oxblood-glow:   "rgba(123, 31, 42, 0.06)"
      bookmark-oxblood:      "#7B1F2A"  # filled bookmark
      heatmap-0:       "#F2EDE4"
      heatmap-1:       "#EDD7CE"
      heatmap-2:       "#DDA597"
      heatmap-3:       "#B86A5E"
      heatmap-4:       "#7B1F2A"
      success-ink:     "#3F6B4E"  # archived / fully local — never neon
      warning-ink:     "#8A6014"  # needs attention — bistre / amber-ink
      error-ink:       "#8B2E2E"  # destructive, kept inside oxblood family
      info-ink:        "#3A4F6B"  # rare, used only for inline help

    dark:
      bg-paper:        "#1A1714"  # warm near-black, never #000
      bg-leaf:         "#221E1A"
      bg-deckle:       "#2B2620"
      bg-vellum:       "#312B25"
      ink-primary:     "#E8E1D4"  # bone / printer's cream
      ink-secondary:   "#C9C1B3"
      ink-muted:       "#8E8678"
      ink-faint:       "#5C5447"
      hairline:        "rgba(232, 225, 212, 0.10)"
      hairline-strong: "rgba(232, 225, 212, 0.18)"
      accent-oxblood:        "#C8485A"
      accent-oxblood-hover:  "#D85E70"
      accent-oxblood-press:  "#A53848"
      accent-oxblood-tint:   "rgba(200, 72, 90, 0.16)"
      accent-oxblood-glow:   "rgba(200, 72, 90, 0.10)"
      bookmark-oxblood:      "#C8485A"
      heatmap-0:       "#221E1A"
      heatmap-1:       "#3A2A28"
      heatmap-2:       "#6B3035"
      heatmap-3:       "#9A3A47"
      heatmap-4:       "#C8485A"
      success-ink:     "#7FB28F"
      warning-ink:     "#D9B26A"
      error-ink:       "#D87680"
      info-ink:        "#7E9DBE"

  typography:
    families:
      serif:
        stack: 'Newsreader, "Source Serif 4", "Iowan Old Style", "Songti SC", "Songti TC", "Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK SC", "Noto Serif CJK TC", "Noto Serif KR", Georgia, serif'
        role: page titles, history-entry titles, body of long-form summaries, brand wordmark
        weight_range: 400, 500, 600
        feature_settings: '"kern", "liga", "onum"'
        tracking_intent: very slight negative tracking on display sizes
      sans:
        stack: 'Inter, "Inter Variable", -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "PingFang TC", "Hiragino Sans", "Yu Gothic", "Noto Sans CJK SC", "Noto Sans CJK TC", "Noto Sans KR", system-ui, sans-serif'
        role: UI chrome — buttons, tabs, sidebar labels, settings, body of short copy
        weight_range: 400, 500, 600
        feature_settings: '"kern", "liga", "ss01"'
      mono:
        stack: '"JetBrains Mono", "Commit Mono", "Berkeley Mono", "IBM Plex Mono", "Sarasa Mono SC", "Sarasa Mono TC", "Source Code Pro", ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace'
        role: timestamps, URLs, file paths, IDs, byte counts, command snippets, key shortcuts
        weight_range: 400, 500
        feature_settings: '"calt"'
    rendering:
      smoothing: '-webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale'
      text_rendering: optimizeLegibility
      cross_platform_note: >-
        Tauri WebView text rendering differs sharply between macOS, Windows
        and Linux. Hinting/letter-spacing must be calibrated per-platform
        before declaring this design complete. The serif must look like
        printer's serif, not 1998 IE serif.
    scale:
      display:    { size: "32px", line: "38px", weight: 600, family: serif, tracking: "-0.01em" }
      title:      { size: "24px", line: "30px", weight: 600, family: serif, tracking: "-0.005em" }
      heading:    { size: "20px", line: "26px", weight: 500, family: serif }
      entry:      { size: "17px", line: "24px", weight: 500, family: serif }  # history-entry title
      body:       { size: "15px", line: "24px", weight: 400, family: serif }
      ui:         { size: "14px", line: "20px", weight: 500, family: sans }
      label:      { size: "13px", line: "18px", weight: 500, family: sans }
      caption:    { size: "12px", line: "16px", weight: 400, family: sans }
      meta:       { size: "12px", line: "16px", weight: 400, family: mono }
      micro:      { size: "11px", line: "14px", weight: 500, family: mono, tracking: "0.04em", uppercase: true }
    typographic_rules:
      - body and history-entry titles are always serif; the editorial feel collapses if titles become sans
      - never use serif for buttons, tabs, or dense form labels
      - timestamps, IDs, and URLs are always mono — they are evidence, not prose
      - never use more than two weights of any one family on the same screen
      - long titles wrap with text-pretty / text-balance; never truncate aggressively
      - CJK readers must see real Songti / Mincho serif — not Latin serif with CJK fallbacks that flip into sans

  spacing:
    grid: 4px
    scale:
      "1":  "4px"
      "2":  "8px"
      "3":  "12px"
      "4":  "16px"
      "5":  "20px"
      "6":  "24px"
      "8":  "32px"
      "10": "40px"
      "12": "48px"
      "16": "64px"
      "20": "80px"
    density:
      sidebar-width:        "240px"
      detail-panel-width:   "360px"
      topbar-height:        "56px"
      statusbar-height:     "32px"
      content-max-measure:  "680px"   # reading measure for entry bodies
      page-gutter:          "32px"
      card-padding:         "20px"
      timeline-rail-gutter: "112px"   # left gutter that holds mono timestamps

  radii:
    none:    "0px"
    hair:    "2px"
    soft:    "4px"   # default for chips, inputs, cards
    panel:   "6px"
    pill:    "999px"  # only for tags and search input
    note: >-
      Square or near-square geometry. Avoid the 8–12px rounded-card look —
      it reads as 2024 SaaS template. Hairlines, not shadows, do the work
      of separating surfaces.

  elevation:
    rule: prefer hairlines over shadows
    none:    "none"
    hairline: "inset 0 0 0 1px var(--hairline)"
    raised:  "0 1px 0 var(--hairline)"
    overlay: "0 12px 32px rgba(26, 22, 18, 0.10), 0 2px 6px rgba(26, 22, 18, 0.06)"
    note: >-
      Only the global ⌘K search overlay and modal dialogs are allowed to
      use the overlay shadow. Cards, panels, popovers stay flat with
      hairline borders only.

  texture:
    paper-grain:
      asset: "noise SVG, monochrome"
      opacity_light: "3%"
      opacity_dark:  "5%"
      blend_mode_light: multiply
      blend_mode_dark:  screen
      role: applied as a global ::before layer on the app shell, never on individual cards
    film-grain:
      enabled_in: dark
      opacity:    "4%"
      role: deepens the darkroom feeling without becoming visible noise
    image-treatment:
      thumbnail_inner_stroke: "inset 0 0 0 1px rgba(26, 22, 18, 0.08)"
      thumbnail_vignette:     "radial subtle, 4% opacity"
      role: every captured page screenshot looks like a print mounted on cardstock
    note: >-
      Texture must be felt, not seen. If a casual user can describe the
      paper grain at a glance, the opacity is too high.

  motion:
    duration:
      fast:    "120ms"
      base:    "180ms"
      slow:    "260ms"
      page:    "320ms"
    easing:
      standard:  "cubic-bezier(0.2, 0.0, 0.0, 1.0)"
      enter:     "cubic-bezier(0.0, 0.0, 0.2, 1.0)"
      exit:      "cubic-bezier(0.4, 0.0, 1.0, 1.0)"
    rules:
      - no toast notifications; status changes are absorbed into the status bar text
      - no spring physics, no bouncy easing, no parallax
      - skeleton pulse uses a slow 1500ms ease-in-out fade
      - reduced-motion replaces all transitions with instant state changes
      - never animate text — opacity and transform only
      - hover tints settle in 120ms; route transitions cross-fade over 260ms

  iconography:
    style: "1.5px stroke, square caps, 20px nominal grid"
    set: lucide-react (subset, customised stroke)
    rule_against_emoji: emojis are never used as icons
    fill_policy: outline by default; the only filled icon is the bookmark when "saved"
    brand_favicons: >-
      Per-source favicons render inside a 28px square with 4px radius and a
      1px inner hairline; never bare PNGs. Missing favicons fall back to a
      monogram of the domain's first letter set in serif on bg-leaf.

  components:
    timeline-rail:
      description: >-
        A vertical 1px hairline running through the center column with 6px
        oxblood disc markers. Empty hours collapse; never pad the rail
        with empty time blocks.
    date-divider:
      description: >-
        A single line such as "Today · May 24, 2026". The relative label
        ("Today", "Yesterday", "Last Tuesday") is in serif italic oxblood;
        the absolute date is in sans grey. A 1px hairline runs to the
        right margin, broken by a small oxblood disc.
    history-entry:
      description: >-
        Source favicon + domain (mono, muted) on a top row alongside a
        type tag (Article, Repository, Search, Video, Doc). Below: serif
        title. Below: 2-line sans body excerpt. Below (optional): a "Note"
        or "Saved Snippet" inline block in oxblood ink on bg-leaf.
        Bookmark icon hits the right edge.
    contact-sheet-thumbnail:
      description: >-
        Used in Dashboard "On This Day" and in Intelligence weekly recap.
        A 4:3 capture of the page rendered with the thumbnail inner
        stroke and faint vignette, mono frame number ("034 / 412") below.
    chip:
      description: >-
        Pill 999px radius, sans 13px, hairline border, bg-leaf. Active
        chip swaps to oxblood-tint background and oxblood ink border.
    callout-localfirst:
      description: >-
        A 1px hairline card on bg-leaf with a small oxblood disc icon. No
        gradient. Body text in sans 14px ink-secondary.
    statusbar:
      description: >-
        Always-visible 32px row at the bottom of the shell. Mono 12px,
        ink-muted. Holds total pages, archive size, last-archived
        timestamp, and a single green tick if the last run succeeded.

  accessibility:
    contrast:
      body_text_on_paper: "WCAG AA at minimum; verified on bg-paper, bg-leaf, bg-deckle"
      muted_text_floor: "must reach 4.5:1 even on bg-deckle hover"
    focus_ring: "2px solid accent-oxblood, 2px offset, never replaced by color-only state"
    keyboard:
      - all interactive surfaces reachable in tab order
      - global ⌘K opens search overlay
      - / focuses the inline search input
      - g then t / s / i / d / o jumps to Timeline / Search / Intelligence / Dashboard / Sources
    reduced_motion: "all 1500ms+ pulses become static; cross-fades become instant"
    cjk_lang_attr: "html[lang] must reflect active locale so the OS picks correct CJK glyphs"

  copy:
    voice:
      - archival, not promotional
      - quiet, not exclamatory
      - precise about what the app does and does not do
    do_not_say:
      - "Awesome!"
      - "Boom!"
      - "✨ AI-powered ✨"
      - "Welcome aboard!"
    do_say:
      - "Last archived 14:23 · 1,847 entries"
      - "Nothing here yet. Memory is patient."
      - "These are yours."
      - "PathKeep has captured 7,842 pages across 4 years."

  tokens_implementation_note: >-
    The tokens above are the design contract. Any tokens.css or
    tailwind.config that ships PathKeep must derive from this file. New
    tokens are added here first, then mirrored into code. The codebase
    must never become the source of truth for visual values.

---

# PathKeep — Editorial Atlas

## One-line brand

> A reading-room for one's own browsing history. Quiet, paper-feeling, and built to be opened on a Sunday afternoon.

PathKeep is not a productivity tool, not a developer dashboard, and not a privacy badge. It is a private archive of the websites a person has thought through, learned from, and lived alongside — and it should feel like one. The default emotion is *recognition*, not *operation*.

## Why this look

Every visual decision is a reaction to one observation: most local-first, privacy-respecting tools default to dark-mode, mono-typography, neon-accent dashboards that look like Linear, Cursor, or Warp. That language signals "tool for working", not "place for remembering". PathKeep deliberately moves in the other direction. The reference tradition is editorial — the *atlas*, the *commonplace book*, the *contact sheet*, the *library catalogue* — translated into a 2026 desktop app.

Concretely:

- **Paper before screen.** The base color is unbleached paper (`#FAF7F2`), not white and not grey. A 3% noise SVG sits over the entire shell so the surface is felt, never seen. Cards are separated by hairlines, not by shadows or rounded card chrome.
- **Serif is the soul.** History-entry titles, page titles and the brand wordmark are set in Newsreader (with proper CJK Songti / Mincho fallbacks). UI chrome stays in Inter; evidence — timestamps, URLs, IDs — stays in JetBrains Mono. The three-family system is deliberate: take any one away and the editorial feel collapses.
- **One accent only.** Oxblood (`#7B1F2A`) — the colour of library leather and rubber-stamped archive numbers. It marks the active state, the timeline disc, the bookmark, the brand wordmark. It is never used as a fill on more than 5% of any screen. There is no second accent and no gradient.
- **Square, not rounded.** Default radius is 4px. The 8–12px rounded-card look is forbidden because it reads as 2024 SaaS template. The only fully-pill element is the tag chip and the search input.
- **Light by default, darkroom by night.** The default theme is light. The dark theme is not an inverted palette — it is a darkroom: warm espresso paper with cream ink and a faint film grain. Both modes are first-class; neither is the "real" theme.

## What is on the canvas

The application is a three-pane desktop shell:

- **Left rail** holds the brand wordmark, primary navigation, a compact browsing-intensity heatmap, and a "Local First" reassurance card. The rail is calm and never grows in density as the archive grows.
- **Main column** is whatever surface the user is reading right now: most often the Timeline, sometimes the Dashboard, sometimes an Intelligence page. It is the widest of the three columns and is the only place serif body type appears at length.
- **Right detail panel** mirrors the user's current selection. It holds a summary, metadata, tags, connections, and the page's local-first status. It can collapse on smaller windows.

A 32px **status bar** is always present along the bottom: archive size, total pages, last-archived timestamp, and a single tick. No toasts. No popups. No "saved!" confirmations. Background work appears here and only here.

## Where the metaphors are

- **Timeline as printed page.** The center column should feel like a printed editorial page: serif titles, generous measure, mono marginalia for timestamps. Day dividers ("Today · May 24, 2026") look like the headers of a journal.
- **Browsing intensity as contact sheet.** The left-rail heatmap and the Intelligence calendar both treat the user's browsing history as a darkroom contact sheet — every cell is a print of a day's activity, the darkest cells the days they could not put the screen down.
- **Bookmarks as letterpress stamps.** The bookmark icon, when filled, is solid oxblood — the only solid hit of accent on a typical row. It should feel like a wet-ink stamp, not a UI badge.
- **Settings as a colophon.** Settings, Sources and Maintenance pages are styled as the back matter of a book — a colophon page — rather than as a tool's preference screen.

## What is deliberately absent

- No toasts, no growls, no system bells. Every status change resolves into the status-bar text.
- No accent-color picker, no density slider, no font picker. There is one design and the user is trusted to inhabit it. Two themes (Day / Darkroom) and two density modes (Reading / Compact) are the entire personalisation surface.
- No emoji-as-icon. No gradient. No glassmorphism. No neon. No purple. No "AI sparkle" iconography even on AI surfaces.
- No marketing copy in product. Empty states and errors are written like archive labels — short, factual, occasionally literary.

## How this file is used

This file is the design contract. Tokens declared here are mirrored into the project's `tokens.css` and Tailwind configuration. New tokens are added to this file first, then implemented. Components and pages may not invent visual values that are not represented here. When a future contributor or AI agent asks "what color is this?", the answer is in this file or it does not exist yet.
