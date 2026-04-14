# Browser Adapter Guide

> 新瀏覽器適配時，先判斷它是既有 family 的變體，還是真的需要新 family。

## Decision Order

1. 先判斷是否可重用既有 family：
   - `chromium`
   - `firefox`
   - `safari`
   - `takeout`
2. 只有 discovery、schema、evidence families 都明顯不同時，才新增 family

## Adapter Procedure

1. **Name and family**
   - 決定 `browser_family`、`browser_product`
   - 決定 public promise 狀態：validated / implemented-not-promised / candidate
2. **Discovery**
   - host path candidates
   - unreadable / missing access story
3. **Staging**
   - 定義需要一起複製的 DB / sidecar / export files
4. **Extractor**
   - registry entry
   - required / optional tables
   - canonical mappings
   - typed evidence mappings
   - native entity preservation rules
   - capability tags
5. **Archive ingest**
   - `source_profiles` identity
   - `source_batches` provenance
   - watermark / checkpoint / schema drift behavior
6. **Validation**
   - parser tests
   - ingest tests
   - capability snapshot tests
   - local validation evidence
7. **Public promise gate**
   - docs / onboarding / README / release docs 只能在 validation evidence 齊全後更新

## Required Checklist

- schema diff artifact
- capability snapshot
- fixture coverage
- i18n caveat copy
- storage / performance impact note
- docs sync

## Honesty Rules

- 只要 capability / access / validation 缺一塊，就不能升級成 public promise
- degraded / missing access state 必須可見，不能因為看起來不好看就藏起來
