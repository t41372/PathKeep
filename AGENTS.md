# Repository Guidelines

PathKeep — A local-first desktop app for browser history archiving and intelligence. Built with Tauri 2, Rust, Bun, React, and Vite.

## Vision & Requirements

**所有新功能開發必須遵循 `docs/vision-and-requirements.md` 中定義的產品願景和需求。** 這是產品的 single source of truth。

- 開始任何工作前，先讀 `docs/vision-and-requirements.md`（hub 文檔），理解產品定位、核心原則和功能架構。
- 根據 hub 文檔底部的索引，定位到相關的子文檔（`docs/features/`、`docs/architecture/`、`docs/design/` 等）閱讀具體需求和設計決策。
- 做出的技術決策必須符合文檔中記載的核心原則（Trust、Data Sovereignty、Longevity、Intelligence Is Optional、Recoverability）。
- 如果在開發中發現需要修改或擴展需求，先更新對應的文檔，再實現代碼。
- 設計師做了一版設計 prototype，非常好看，放在了 `reference/PathKeep — Desktop UI Design` 目錄下，在做 UI 相關的動作之前，先去看看。舊版本的 UI 完全打掉，我們走設計師做的新版設計。

## General Rules

- 遵守 conventional commit 規範，你寫完了代碼，就主動提交原子化的 commit。
- 做新介面前，把需求丟給 stitch 工具，讓他出幾版設計稿，幫你設計介面和交互。
- 提交 commit 前，保持 100% test coverage 和 mutation test 通過。
- 在代碼庫中寫致死量的注釋。我們不單獨做開發者文檔和決策文檔了。如果做出了重要決策或 trade off，比如數據庫的時間格式，某些技術決策，在相關代碼處寫充足的注釋。
- 

## Project Structure & Module Organization

`src/` contains the React 19 + TypeScript desktop UI. Keep UI tests next to the code they cover as `*.test.ts` or `*.test.tsx`; shared frontend helpers live in `src/lib/`, and Vitest setup lives in `src/test/setup.ts`. End-to-end smoke coverage lives in `tests/e2e/`.

`src-tauri/` contains the Tauri shell and Rust workspace. Use `src-tauri/src/` for desktop entrypoints, session state, and bridge code. Core archive logic lives in `src-tauri/crates/vault-core`, platform integrations in `src-tauri/crates/vault-platform`, and shared worker flows in `src-tauri/crates/vault-worker`. Static assets live in `public/` and `src/assets/`; verification helpers live in `scripts/`.

## Build, Test, and Development Commands

Use Bun for JavaScript tasks and the pinned Rust toolchain for native work.

- `bun run dev`: browser-only Vite preview on `127.0.0.1:1420`
- `bun run desktop:dev`: full Tauri desktop app
- `bun run build`: TypeScript build plus Vite bundle
- `bun run check`: frontend, Rust, and supply-chain quality gates
- `bun run verify`: local CI sweep including coverage, e2e, and debug desktop build
- `bun run desktop:build:debug`: local desktop binary without release bundling

## Coding Style & Naming Conventions

Follow `.editorconfig`: 2-space indentation for `ts`/`tsx`, 4 spaces for Rust, LF endings, and no trailing whitespace. Prettier is authoritative for frontend formatting: no semicolons, single quotes, trailing commas. Run `bun run format` or `bun run format:check`.

Name React components with `PascalCase`, utility modules with lowercase or kebab-case names such as `browser-icons.tsx`, and Rust modules/functions with `snake_case`. ESLint enforces type-only imports where possible and rejects floating promises.

## Testing Guidelines

Frontend unit and integration tests use Vitest with `jsdom`; name them `src/**/*.test.ts(x)`. Playwright specs belong in `tests/e2e/*.spec.ts`. `bun run coverage:js` requires 100% statements, branches, functions, and lines. Rust tests are inline `#[test]` modules, and `bun run coverage:rust` fails if any workspace Rust lines or functions are uncovered.

## Commit & Pull Request Guidelines

Use Conventional Commits, as in `feat(ui): bundle browser icons` or `chore(ci): harden local verification`. Before opening a PR, run `bun run check`, `bun run build`, and `bun run desktop:build:debug`. Update docs in the same branch for user-visible behavior changes, keep commits logically grouped, and note any intentional gaps or follow-up work in the PR description.
