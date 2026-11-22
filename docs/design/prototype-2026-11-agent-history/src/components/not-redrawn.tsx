/**
 * Prototype-only banner for a page that exists in today's app and is not
 * being redesigned. What the prototype shows there is a few cards to place the
 * page in its space; the real page is the baseline and nothing is dropped.
 */
export function NotRedrawn({ page }: { page: string }) {
  return (
    <div className="rounded-xl border border-dashed border-foreground/30 px-4 py-2.5 text-[13px] leading-relaxed text-muted-foreground">
      原型：這是今天的「{page}」頁，這次沒有重畫。這裡只放了幾張卡，示意它在這個空間裡的位置；做進 app 時以現在的頁面為準，一項都不少。
    </div>
  )
}
