# Agent Tool Icon Assets

These assets are bundled so PathKeep never fetches tool artwork at runtime.
They are vendor trademarks and should be refreshed from official vendor
sources instead of being redrawn by hand.

## Sources

| Asset             | Source                                                                                              |
| ----------------- | --------------------------------------------------------------------------------------------------- |
| `claude-code.svg` | Claude Code VS Code extension 2.1.322, `resources/claude-logo.svg` (the extension's own icon).      |
| `codex.svg`       | Codex (`openai.chatgpt`) VS Code extension 26.1106.51428, `resources/blossom-black.svg`.            |
| `codex-dark.svg`  | Same extension as `codex.svg`, `resources/blossom-white.svg`; shown in the dark theme.              |
| `pi.svg`          | `https://pi.dev/logo-auto.svg`, the logo the Pi README links.                                       |
| `deepseek.png`    | `/Applications/DeepSeek Harness.app/Contents/Resources/icon.png` on the dev host, scaled to 128 px. |

## Refresh Notes

- Keep `src/features/data-kinds/tools.ts` aligned when a tool is added or
  removed; the tool ids are the backend's (`claude-code`, `codex`, `pi`,
  `deepseek`).
- Prefer the icon the vendor ships with the tool itself (an app bundle or an
  official editor extension); otherwise use the vendor's site or repository.
- Do not edit logo geometry in PathKeep. If a vendor changes its mark, replace
  the source asset and update this file.
