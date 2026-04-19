# Desktop Truth Audit Context

- Date: 2026-04-18
- Host: current macOS desktop session
- App runtime: `bun run desktop:dev`
- App pid sampled: `34169`
- Archive root: `~/Library/Application Support/com.yi-ting.pathkeep`
- Audit focus:
  - locked-archive startup truth
  - Security unlock flow
  - pre-import desktop usability blockers

## Known constraints for this bundle

- The user explicitly asked not to use browser tools / DevTools.
- This bundle therefore uses a process sample instead of a Web Inspector trace.
- The archive never reached a stable unlocked state during this pass, so this is a **pre-unlock** performance artifact, not a full post-import profile.
- A later fresh `bun run desktop:dev` restart still surfaced the old generic shell bundle on this host, so real-app validation remains entangled with current-host stale WebView / bundle cache drift.
