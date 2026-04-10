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

- [ ] `M5-DI-A-001` 凍結 visit normalization contract：tracking-param policy、IDN handling、registrable domain extraction、search-engine parameter parsing
- [ ] `M5-DI-A-002` 建立 evidence tier contract，區分 `tier_a` / `tier_b` / `tier_c`
- [ ] `M5-DI-A-003` 定義 `domain_category` / `page_category` / `interaction_kind` 與 precedence
- [ ] `M5-DI-A-004` 建立 China Mainland / US 核心 rule packs，並預留台灣 / 日本 / 韓國 / 歐洲 / 俄羅斯 / international packs
- [ ] `M5-DI-A-005` 建立 user override、unknown fallback、versioning 與 top-unmatched review surface
- [ ] `M5-DI-A-006` 決定 baseline tokenization、optional language hint、bundle-size 與 supply-chain review gate
- [ ] `M5-DI-A-007` 建立 fixture / acceptance：同 host 不同 page type、CJK query/title、ambiguous / unknown cases、tracking-URL normalization

## Exit Artifacts

- normalized visit feature contract
- taxonomy v2 rule-pack format
- tokenizer / language-hint decision note
- rule / lexicon / unknown fixtures
