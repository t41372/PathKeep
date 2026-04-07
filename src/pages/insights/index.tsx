import { EmptyState } from '../../components/primitives/empty-state'

export function InsightsPage() {
  return (
    <section className="page-shell">
      <EmptyState
        action={
          <button className="ghost-button" type="button">
            Review insight cards
          </button>
        }
        description="On This Day, periodic summaries, and thread detection will plug into this reserved workspace."
        eyebrow="INSIGHTS"
        title="Insights"
      />
    </section>
  )
}
