# Dev Guides

> 這裡放 PathKeep 維護多瀏覽器 schema / extractor / evidence 保存策略的開發手冊。任何會影響 browser adapter、schema drift、field promotion、capability snapshot 的工作，都應先讀這裡，再動手改 parser / ingest / intelligence。

## Guides

- [browser-schema-evolution.md](browser-schema-evolution.md)
  - 當瀏覽器更新 schema 或加入新欄位時，標準的取樣、diff、fixture、benchmark、docs 同步流程
- [browser-adapter-guide.md](browser-adapter-guide.md)
  - 新瀏覽器 / 新 family adapter 的實作與 validation 指南
- [field-promotion-playbook.md](field-promotion-playbook.md)
  - 如何把 archived native field 升格為 typed evidence、canonical-derived contract，並安全重抽舊資料

## Core Rules

- browser/version metadata 必須保留，但 feature enablement 以 capability snapshot 為主
- 新欄位預設保留，不得為了省事直接丟棄
- raw artifact 只是一層 debug / diff / re-extract evidence，不是唯一的 long-term preservation story
- hot canonical query path 不得直接退化回「每次都掃 raw payload / native entity」
- docs sync 是 contract，不是補充說明：任何 extractor / evidence / capability 變動都要同步更新 source docs
