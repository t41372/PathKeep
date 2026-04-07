import { PermissionGate } from '../../components/primitives/permission-gate'

export function AssistantPage() {
  return (
    <section className="page-shell">
      <PermissionGate
        detail="AI stays optional. Provider choice, index freshness, and the evidence path should all stay visible before asking questions."
        eyebrow="OPTIONAL INTELLIGENCE"
        title="AI stays optional"
      >
        <button className="ghost-button" type="button">
          Configure provider preview
        </button>
      </PermissionGate>
    </section>
  )
}
