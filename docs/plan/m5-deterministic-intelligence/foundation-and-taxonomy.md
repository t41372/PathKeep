# M5-DI-A — Foundation And Taxonomy

> 讀這份文檔的時機：你要先把 deterministic intelligence 的輸入、normalization、taxonomy precedence、多語 tokenization 與 rule-pack 治理做對，再去碰 query groups / threads / cards。

## Source Inputs

- [../../features/deterministic-intelligence.md](../../features/deterministic-intelligence.md)
- [../../architecture/decisions/006-deterministic-intelligence-boundary.md](../../architecture/decisions/006-deterministic-intelligence-boundary.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../../architecture/tech-stack.md](../../architecture/tech-stack.md)

## 本工作包要交付什麼

- canonical visit normalization contract
- deterministic evidence tiers
- taxonomy v2 precedence
- rule-pack layout 與治理規則
- script-aware tokenization / optional language-hint strategy
- fixture family 與 acceptance baseline

## WBS

- [x] `M5-DI-A-001` 凍結 visit normalization contract：tracking-param policy、IDN handling、registrable domain extraction、search-engine parameter parsing
- [x] `M5-DI-A-002` 建立 evidence tier contract，區分 `tier_a` / `tier_b` / `tier_c`
- [x] `M5-DI-A-003` 定義 `domain_category` / `page_category` / `interaction_kind` 與 precedence
- [x] `M5-DI-A-004` 建立 China Mainland / US 核心 rule packs，並預留台灣 / 日本 / 韓國 / 歐洲 / 俄羅斯 / international packs
- [x] `M5-DI-A-005` 建立 user override、unknown fallback、versioning 與 top-unmatched review surface
- [x] `M5-DI-A-006` 決定 baseline tokenization、optional language hint、bundle-size 與 supply-chain review gate（2026-04-10：shipping runtime 明確固定為 checked-in heuristic + script-aware tokenization；任何 external asset 仍需先過 `PG-RD-AI-010`）
- [x] `M5-DI-A-007` 建立 fixture / acceptance：同 host 不同 page type、CJK query/title、ambiguous / unknown cases、tracking-URL normalization

## 2026-04-10 開始實作註記

- `vault-core::visit_taxonomy` 已新增第一版 URL normalization / search-query extraction foundation，先把 tracking-param filtering、常見 multi-label registrable-domain heuristic 與 CJK query fixture 帶進 repo，避免後續仍把 query parser 繼續散落在 `insights.rs`。
- 這個 baseline **刻意不引入新的 external registrable-domain / tokenizer dependency**；在 `PG-RD-AI-010` 完成 license / bundle-size / supply-chain review 前，先以 checked-in heuristic + fixture 誠實前進，而不是偷渡未審核 runtime wheel。
- 2026-04-10 continued：`vault-core::visit_taxonomy` 現在也負責 `tier_a/tier_b/tier_c` evidence assessment、taxonomy v2 precedence、China Mainland / US core packs、user override、script-aware tokenization 與 unknown-domain review baseline；`insights.rs` 已開始持久化 `domain_category` / `page_category` / `interaction_kind` / `evidence_tier` / taxonomy trace，且移除 deterministic importance 對 `duration_ms` 的依賴。
- 2026-04-10 closeout：first-party-only enrichment runtime、dual built-in plugin defaults、Settings / Insights queue review 與 retry / cancel operability 已補齊，`WORK-M5-A` 因此不再卡在「plugin runtime / queue family 尚未誠實落地」。
- `PG-RD-AI-010` 仍保持 open：目前 shipping runtime 明確只用 checked-in heuristic rule packs，不 bundle external registrable-domain / tokenizer / language-ID / optional model assets；若要升級成 runtime dependency，必須先補完 license、bundle-size、supply-chain 與 fallback/removal review。

## Exit Artifacts

- normalized visit feature contract
- taxonomy v2 rule-pack format
- tokenizer / language-hint decision note
- rule / lexicon / unknown fixtures
