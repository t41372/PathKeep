import { EmptyState } from '../../components/primitives/empty-state'

export function ExplorerPage() {
  return (
    <section className="page-shell">
      <EmptyState
        action={
          <button className="ghost-button" type="button">
            Preview filters
          </button>
        }
        description="Time-travel and full-text search land here next. The shell is ready for timeline, filters, and record detail."
        eyebrow="EXPLORER"
        title="History Explorer"
      />
    </section>
  )
}
