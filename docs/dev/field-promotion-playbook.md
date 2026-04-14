# Field Promotion Playbook

> 當 archived native field 被證明對新 intelligence 功能有價值時，用這份流程把它升格。

## Promotion Targets

- **typed evidence**
  - 適合跨 browser family 對齊的訊號
  - 例：search query、navigation edge、engagement metric、context signal
- **canonical-derived contract**
  - 高頻、跨功能、需要 hot query / filter / sort 的欄位
  - 要非常保守
- **module capability**
  - 不一定需要成為欄位，但要能作為 feature gate / degrade contract

## Procedure

1. **定義使用場景**
   - 哪個 intelligence module 要用
   - 是 required 還是 optional capability
2. **確認來源**
   - 目前在 `native_entities`
   - 還是只能從 retained raw artifact 重抽
3. **設計映射**
   - source path / entity kind
   - target typed evidence table / canonical-derived field / capability tag
4. **補 extractor registry**
   - mapping
   - coverage rules
   - warnings / fallback
5. **建立 re-extract / promote job**
   - dry-run
   - backfill existing archived evidence
   - 更新 capability snapshot / coverage stats
6. **更新 explainability**
   - module 要顯示這個 field 來自哪種 evidence tier / capability
7. **補測試**
   - promotion roundtrip
   - old data backfill
   - missing field graceful degradation

## Output Checklist

- capability before / after
- migration or reset note
- re-extract benchmark
- docs sync

## Guardrails

- 不要因為某個欄位「看起來有趣」就直接 promotion 到 canonical
- canonical-derived contract 必須證明它值得 hot-path 成本
- promotion 不能破壞 `unknown` / partial-coverage 的 honest fallback
