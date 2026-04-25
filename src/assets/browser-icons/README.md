# Browser Icon Assets

These assets are bundled so PathKeep never fetches browser artwork at runtime.
They are vendor trademarks and should be refreshed from official vendor sources
instead of being redrawn by hand.

## Sources

| Asset           | Source                                                                                                                                                        |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `chrome.png`    | Extracted from `/Applications/Google Chrome.app/Contents/Resources/app.icns` on the dev host.                                                                 |
| `chromium.png`  | `https://chromium.googlesource.com/chromium/src/+/main/chrome/app/theme/chromium/product_logo_256.png?format=TEXT`                                            |
| `edge.png`      | `https://edgecdn-embza6g8cacagcbn.z01.azurefd.net/welcome/static/favicon.png`                                                                                 |
| `edge-dev.png`  | Same official Microsoft Edge Insider favicon source as `edge.png`; Microsoft does not expose a separate channel app icon in the public page assets used here. |
| `brave.svg`     | `https://brave.com/static-assets/images/brave-logo-sans-text.svg`                                                                                             |
| `vivaldi.png`   | `https://vivaldi.com/wp-content/uploads/cropped-favicon-192x192.png`                                                                                          |
| `arc.png`       | `https://arc.net/favicon.png`                                                                                                                                 |
| `atlas.png`     | Extracted from `/Applications/ChatGPT Atlas.app/Contents/Resources/AppIcon.icns` on the dev host.                                                             |
| `comet.png`     | Extracted from `/Applications/Comet.app/Contents/Resources/app.icns` on the dev host.                                                                         |
| `opera.png`     | `https://cdn-production-opera-website.operacdn.com/staticfiles/assets/images/favicon/opera/apple-touch-icon-180x180.00d9278d6de6.png`                         |
| `opera-gx.png`  | `https://cdn-production-opera-website.operacdn.com/staticfiles/assets/gx/images/welcomeGx/favicon/apple-touch-icon-180x180.16b2fb5b1ecc.png`                  |
| `firefox.svg`   | `https://www.mozilla.org/media/protocol/img/logos/firefox/browser/logo.svg`                                                                                   |
| `librewolf.svg` | `https://librewolf.net/icon.svg`                                                                                                                              |
| `floorp.svg`    | `https://floorp.app/favicon.svg`                                                                                                                              |
| `waterfox.png`  | `https://www.waterfox.com/favicons/apple-touch-icon.png`                                                                                                      |
| `safari.png`    | Extracted from `/Applications/Safari.app/Contents/Resources/AppIcon.icns` on the dev host.                                                                    |

## Refresh Notes

- Keep `src/lib/browser-icons.tsx` and
  `docs/architecture/browser-support-and-adapter-playbook.md` aligned when a
  browser is added or removed.
- Prefer app bundle icons when the official app is installed locally; otherwise
  use an official vendor site, official CDN, or upstream source repository.
- Do not edit logo geometry in PathKeep. If a vendor changes its mark, replace
  the source asset and update this file.
