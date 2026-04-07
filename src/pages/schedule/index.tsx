import { PermissionGate } from '../../components/primitives/permission-gate'

export function SchedulePage() {
  return (
    <section className="page-shell">
      <PermissionGate
        detail="Scheduler install still follows the same trust rule: preview the artifact, show manual instructions, and only apply after explicit approval."
        eyebrow="SCHEDULE"
        title="Preview the native scheduler first"
      >
        <button className="ghost-button" type="button">
          Preview native schedule
        </button>
      </PermissionGate>
    </section>
  )
}
