# Browsing Rhythm 主图改成真实日期日历热力图 — Trade-off 决策

> **状态：Accepted**
> **日期：2026-04-19**
> **范围：** `/intelligence` 的 `Browsing Rhythm` 主卡  
> **关联文档：**
>
> - [screens-and-nav.md](screens-and-nav.md)
> - [ui-review-guardrails.md](ui-review-guardrails.md)
> - [../features/intelligence-current-state.md](../features/intelligence-current-state.md)
> - [../features/core-intelligence-ultimate-design.md](../features/core-intelligence-ultimate-design.md)

---

## 1. 问题定义

之前 accepted docs 把 `Browsing Rhythm` 主图定义成 **周 × 小时** 热力图。  
这在“看一周内大致时段”时勉强成立，但一旦放进 `/intelligence` 的真实 `week / month / quarter / year / custom` 时间窗，就暴露出两个问题：

1. **日期不真实**
   - 用户点到某个格子时，前端只能拿 `dow + hour` 去猜一个“最近同 weekday 的日期”。
   - 结果是 UI 看起来像日期热图，实际却不是按真实日期取值。
2. **长时间窗不可解释**
   - 月 / 季 / 年 / custom 下，用户首先关心的是“哪几天活跃、哪几天有新站点”，而不是把所有访问先压回一张 7×24 桶图。
   - 当前实现甚至会出现“无论点哪里，都只在几个近期日期之间跳”的假交互。

这已经不是视觉偏好问题，而是 **数据可视化 truth drift**。

---

## 2. 约束

- **不新增** Tauri / Rust command。
- 继续复用现有 deterministic API：
  - `getDiscoveryTrend(dateRange, profileId, 'day')`
  - `getDigestSummary(singleDayRange)`
  - `getTopSites(singleDayRange)`
  - `getBrowsingRhythm(singleDayRange)`
- `/intelligence` 仍要遵守：
  - desktop 真机 truth gate
  - 卡片限高 + 内滚动
  - 文案讲人话
  - 低信号 section 让位，不占主阅读顺序

---

## 3. 候选方案

### 方案 A — 保持周 × 小时主图

**做法**

- 继续以 `getBrowsingRhythm(dateRange)` 返回的 `dow × hour` bucket 做主图。
- 再额外补一个日期 chooser / 当天 detail。

**优点**

- 复用现有后端输出最直接。
- 如果用户的问题是“我通常在星期几几点活跃”，这个视角本身有价值。

**缺点**

- 主图和 detail 的“日期”不是同一个 truth model。
- 在长时间窗里，用户仍然先看到一个跟真实日期脱钩的抽象桶图。
- 需要继续做不诚实的 bucket→date 映射，或者在主图上明确承认“这不是日期图”。

**结论**

- 不能满足“每个方块必须对应真实日期”的要求。

### 方案 B — 改成 GitHub 式真实日期热力图，小时分布退到单日 detail

**做法**

- 主图改用 `getDiscoveryTrend(..., 'day')` 的真实日期点。
- 渲染成 GitHub contributions 风格的日历热力图。
- 点击某一天后，再用：
  - `getDigestSummary(singleDayRange)`
  - `getTopSites(singleDayRange)`
  - `getBrowsingRhythm(singleDayRange)`
    显示当天 digest / top sites / 24 小时分布。

**优点**

- 主图每格都对应一个真实日期，语义直接成立。
- `week / month / quarter / year / custom` 都能用同一种视觉语法。
- “哪一天值得看” 与 “这一天具体几点活跃” 被拆成主视图与 detail，信息层级更清楚。

**缺点**

- 失去“整段时间的一张总时段桶图”这个首屏视角。
- 需要在前端补日期日历布局与缺失日期补零逻辑。

**结论**

- 最符合用户刚刚明确确认的方向，也最符合数据真实度要求。

### 方案 C — 双主图并存（日历热图 + 周 × 小时时段图）

**做法**

- 在同一卡片或上下两张卡里同时放日历热图和时段桶图。

**优点**

- 两种视角都保留。

**缺点**

- 首屏信息密度过高。
- 会重新占掉用户已经明确批评过的黄金版面。
- 同一张卡既要解释日期，又要解释桶图，学习成本更高。

**结论**

- 不是当前问题的最小修复；会把 truth repair 扩成信息架构膨胀。

---

## 4. 方案比较

| 维度               | 方案 A：周×小时主图 | 方案 B：真实日期主图 | 方案 C：双主图 |
| ------------------ | ------------------- | -------------------- | -------------- |
| 日期语义是否真实   | 差                  | **好**               | 好             |
| 长时间窗可解释性   | 差                  | **好**               | 中             |
| 实现复杂度         | 低                  | 中                   | 高             |
| 首屏占用           | 中                  | **中**               | 高             |
| 与用户最新要求一致 | 否                  | **是**               | 否             |

---

## 5. 最终决定

采用 **方案 B**：

- `Browsing Rhythm` 主图正式改成 **真实日期日历热力图**
- **小时热力图不再做主图**
- 小时分布改成 **选中某一天后的 detail 区**

---

## 6. 风险与缓解

### 风险 1：失去“整段时间的一眼看时段”的视角

**缓解**

- 单日 detail 保留 24 小时分布。
- 如果未来证据显示用户真的需要一个独立的“全窗口时段分布”视图，再单独立项，不在这次 truth repair 里偷偷长回来。

### 风险 2：`getDiscoveryTrend('day')` 不包含无访问日期

**缓解**

- 前端按当前 date range 补齐缺失日期为 0。
- 这样热力图仍然按真实日历连续呈现。

### 风险 3：长时间窗横向过宽

**缓解**

- 允许卡片内横向滚动。
- 但卡片整体仍遵守限高 + 内滚动规则。

---

## 7. 回滚策略

如果未来发现日历主图在真实用户数据下反而更难理解，回滚不应直接恢复旧的假日期映射实现。

正确回滚路径只能二选一：

1. 明确把 `Browsing Rhythm` 改名并定义成“时段分布图”，不再伪装成日期热图。
2. 保留日历主图，同时把“时段分布”独立成次级视图或 drill-down。

不允许回滚到“主图是桶图、但用户又以为它在代表真实日期”的中间状态。

---

## 8. 2026-04-20 shipped override

2026-04-20，用户进一步明确指出：

- `/intelligence` overview 与 Dashboard 的 `Browsing Rhythm` 卡片，不应该在点日格时直接跳走
- 点日格后应先在卡片下方展示 compact day preview
- 只有用户再按明确的 `查看详情` CTA，才进入 `/intelligence/day/:date`
- Dashboard 若 archive 横跨多年，还要用 bounded pager 浏览不同年份，并明确显示当前年份
- 顶部摘要文案必须诚实使用 `totalVisits` 口径；Dashboard 用 calendar-year wording，`/intelligence` overview 则按实际 date range 显示 exact-range wording
- compact day preview 的信息层级也要收紧成：全宽 24 小时分布、全宽重点网站列、proportion bar 活动构成

这次 override **没有** 推翻「day 是 first-class shared route」这条更高层 contract。  
真正被修正的是 `Browsing Rhythm` 卡片自己的 click grammar：

- `Browsing Rhythm` 卡片：**preview-first**
- Explorer detail rail / 其他 day entry surface：**route-first**

这样既保留了 shared `day insights` route 作为唯一完整 read model，也把卡片内本来就约定好的同日摘要 / 小时分布 / 重点网站恢复回来。

这一轮之后，还多了两个明确的 shipped contract：

- summary line 不再写成 `pages visited` 这种容易误导成 unique pages 的句式；当前 truth 明确是 `visits`
- `/intelligence/day/:date` 会重用同一套 flat 24 小时分布与 proportion bar 活动构成，但 richer `Standout Sites` section 继续留在 detail route，不被压回 overview preview 的 chip row

## 9. 用户确认记录

2026-04-19，用户已明确确认：

- **accepted docs 里的“周 × 小时主图”应被推翻**
- **改成 GitHub 式真实日期热力图**
- **小时热力图放进选中某一天后的 detail**

因此本文件作为 accepted docs 更新的前置 trade-off 记录，后续相关 source-of-truth 文档应以本决定为准。
