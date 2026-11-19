/**
 * The landing page in English. Facts checked 2026-11-14: Chromium deletes
 * history older than 90 days (`kExpireDaysThreshold`); Apple says Safari can
 * keep it "for as long as a year"; Firefox Sync uploads 30 days and keeps
 * 60; Claude Code's `cleanupPeriodDays` defaults to 30. Codex has no
 * automatic cleanup, so it is not on the retention list. Privacy wording
 * follows the README's Privacy section.
 */

import type { Copy } from './copy'

export const en: Copy = {
  htmlLang: 'en',
  description:
    'Chrome deletes your history after 90 days. Claude Code deletes your conversations after 30. PathKeep keeps all of it on your own computer, and helps you find your way back.',
  skip: 'Skip to content',
  nav: {
    label: 'Sections',
    why: 'Why',
    keeps: 'What it keeps',
    privacy: 'Privacy',
    faq: 'Questions',
    github: 'GitHub',
    download: 'Download',
  },
  hero: {
    title: 'Keep the path |you’ve walked.',
    lede: '{chrome:Chrome} deletes your history after 90 days. {claudecode:Claude Code} deletes your conversations after 30. PathKeep keeps all of it on your own computer, and helps you find your way back.',
    primary: 'Download for macOS',
    secondary: 'Windows',
    demo: 'Try it in your browser →',
    note: 'Version 0.6 · free and open source · macOS and Windows',
  },
  shot: {
    tablist: 'Spaces',
    tabs: { browsing: 'Browsing history', agent: 'Agent conversations' },
    caption: 'A picture of the app. The working demo is further down.',
  },
  why: {
    title: 'Your history has an expiry date |you never agreed to.',
    body: 'Browsers and coding agents keep a short window and clean up the rest on their own. Sync doesn’t keep it either: it moves recent history between your devices, nothing more.',
    axis: ['90 days', 'A year', 'Forever'],
    rows: [
      { source: 'Chrome, Edge, Brave, Arc', keeps: '90 days', days: 90 },
      { source: 'Safari', keeps: 'up to a year', days: 365 },
      { source: 'Firefox Sync', keeps: 'sends 30 days, keeps 60', days: 60 },
      { source: 'Claude Code', keeps: '30 days', days: 30 },
      { source: 'PathKeep', keeps: 'as long as you want', days: null },
    ],
    source:
      'From each browser’s source code and documentation, and Claude Code’s settings reference.',
    closing:
      'If you use Chrome and have never backed up your history, everything older than three months is most likely gone already.',
  },
  keeps: {
    title: 'Two kinds of memory, |one archive.',
    body: 'PathKeep copies each kind into one archive on your disk, a little at a time, for as long as you want to keep it. In the app, each kind has a space of its own.',
    spaces: [
      {
        id: 'browsing',
        name: 'Browsing history',
        covers:
          'Every browser and profile on your computer: {chrome:Chrome}, {edge:Edge}, {firefox:Firefox}, {safari:Safari}, {arc:Arc}, {brave:Brave} and more. Older history comes back from Google Takeout.',
        pages: 'Home · History · Insights · Backup',
      },
      {
        id: 'agent',
        name: 'Agent conversations',
        covers:
          'Sessions from {claudecode:Claude Code}, {codex:Codex}, {pi:Pi} and {dsh:DeepSeek Harness}, with their projects, subagents, tokens and the hours your agents worked.',
        pages: 'Insights · History · Projects · Backup',
      },
    ],
    more: 'More things that quietly expire will follow, each in a space of its own.',
  },
  find: {
    title: 'Keeping it is |half the job.',
    body: 'PathKeep stays fast with millions of visits in it, so the page you read two years ago is still a few keystrokes away.',
    items: [
      {
        id: 'search',
        title: 'Search',
        text: 'Titles, addresses and page text, or a regular expression. Filter by date, browser and site.',
      },
      {
        id: 'history',
        title: 'History',
        text: 'Every visit by day and by session, in a list that stays smooth however long it gets.',
      },
      {
        id: 'insights',
        title: 'Insights',
        text: 'Worked out on your computer: your top sites, when you browse, what you search for, the pages you keep going back to.',
      },
      {
        id: 'agent',
        title: 'Agent insights',
        text: 'The hours your agents worked and the hours you were there, tokens and cost at list price, by project and by tool.',
      },
      {
        id: 'ask',
        title: 'Ask, if you want',
        text: 'Ask about your history in plain words, with a model on your computer or a service you set up. It shows the pages it used.',
      },
      {
        id: 'notes',
        title: 'Notes and export',
        text: 'Star pages, write notes you can search later, and export everything as HTML, Markdown, plain text or JSON Lines.',
      },
    ],
  },
  privacy: {
    title: 'Your history stays |on your computer.',
    body: 'There’s no account and no server. PathKeep reads your browsers’ and tools’ files, writes to its own archive, and never changes theirs.',
    points: [
      'AI is off until you turn it on. A model on your computer keeps everything on it.',
      'The archive can be encrypted with a password.',
      'Every backup and import leaves a record you can read, and every import can be undone.',
      'Free software under the GPL-3.0. Every line is on GitHub.',
    ],
    network:
      'Two features contact websites: link previews and page summaries. Settings shows both, and either can be off.',
  },
  demo: {
    title: 'Try PathKeep |in your browser.',
    body: 'A full copy of the app runs here on made-up history, nobody’s real browsing. Click anything; nothing you do leaves this page.',
    newTab: 'Open the demo in its own tab',
    narrow:
      'The demo needs a computer’s screen. Open this page on one to try it.',
    frame: 'PathKeep demo',
    start: 'Click to try it',
  },
  faq: {
    title: 'Questions',
    items: [
      {
        title: 'Does my history leave my computer?',
        text: 'No. PathKeep has no server and no account. Link previews and page summaries fetch from the websites directly, and both can be turned off. AI stays off until you choose a model.',
      },
      {
        title: 'Which browsers does it support?',
        text: 'Tested: Chrome, Edge, Firefox, and on macOS Safari, ChatGPT Atlas and Perplexity Comet. Brave, Vivaldi, Arc, Opera, Chromium, LibreWolf, Floorp and Waterfox are backed up too, but not yet tested for release.',
      },
      {
        title: 'Does it change my browser’s own history?',
        text: 'No. PathKeep only reads. Your browser works as before, and PathKeep keeps its own copy.',
      },
      {
        title: 'Can I get back history that’s already gone?',
        text: 'Some of it. Google Takeout holds your Google account’s Chrome history, often about 18 months of it, and PathKeep can import it. So can history files from other browsers or computers. Every import can be undone.',
      },
      {
        title: 'Do I need AI?',
        text: 'No. Backup, history, search and insights work without it. Ask and semantic search are there for those who want them.',
      },
      {
        title: 'Does it cost anything?',
        text: 'No. PathKeep is free software under the GPL-3.0, and all the source is on GitHub.',
      },
      {
        title: 'What is it built with?',
        text: 'A desktop app written in Rust with Tauri 2, not Electron, with an interface in React and TypeScript. The archive is a Turso database, SQLite-compatible and written in Rust, and full-text search runs on Tantivy.',
      },
    ],
  },
  download: {
    title: 'Start keeping |the path today.',
    body: 'Download it and go: no account. It finds your browsers and agent tools, and asks before it keeps anything.',
    platforms: [
      {
        name: 'macOS',
        detail: 'Disk image (.dmg)',
        action: 'Download for macOS',
      },
      {
        name: 'Windows',
        detail: 'Installer (.msi or .exe)',
        action: 'Download for Windows',
      },
      {
        name: 'Linux',
        detail: 'Coming later',
        action: 'Follow on GitHub →',
        soon: true,
      },
    ],
    note: 'The installers aren’t signed yet; the README says how to open them the first time.',
    source: 'Source on GitHub',
  },
  footer: {
    license: 'PathKeep is free software under the GPL-3.0.',
    links: {
      label: 'Project',
      github: 'GitHub',
      releases: 'Releases',
      readme: 'README',
    },
  },
  notFound: {
    title: 'Nothing here.',
    body: "This page doesn't exist, or it has moved.",
    home: 'Back to PathKeep',
  },
}
