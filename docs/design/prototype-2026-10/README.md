# PathKeep frontend redesign prototype (2026-10)

This is the Claude Design prototype the current frontend is built from. It is a
reference, not shipped code.

- `PathKeep App.dc.html` — the main app: Home, History, Insights, Ask, Backup,
  Settings, the command palette (⌘K), the lock screen and dialogs.
- `PathKeep Onboarding.dc.html` — the seven-step first-run wizard.
- `screenshots/` — renders of the main screens, for quick comparison.

To view it, serve this folder over HTTP (`python3 -m http.server`) and open the
HTML files. `support.js` loads React and fonts from public CDNs.

The data in the prototype is made up. The real app reads everything from the
local archive.
