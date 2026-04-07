import { ErrorState } from '../../components/primitives/error-state'

export function AuditPage() {
  return (
    <section className="page-shell">
      <ErrorState
        description="Manifest chains, diff views, and rollback records belong here. The shell is ready; the canonical archive data plane lands next."
        title="Audit artifacts are waiting for the M1 engine"
      />
    </section>
  )
}
