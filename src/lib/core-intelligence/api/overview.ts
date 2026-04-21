/**
 * @file overview.ts
 * @description Barrel module that preserves the public overview API path while delegating ownership to smaller overview load, peek, and read modules.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 維持 `src/lib/core-intelligence/api/overview` 這條 public import path 穩定。
 * - 重新導出 overview loaders、peek helpers、與 section reads。
 *
 * ## 不負責
 * - 不實作任何 cache 或 request 邏輯。
 * - 不新增第二套 API contract。
 *
 * ## 依賴關係
 * - 依賴 `overview-loaders.ts`、`overview-peek.ts`、`overview-read.ts`。
 *
 * ## 性能備注
 * - barrel 本身沒有執行成本；拆分是為了降低維護時的心智負擔，而不是改變 runtime 行為。
 */

export * from './overview-loaders'
export * from './overview-peek'
export * from './overview-read'
