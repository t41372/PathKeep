import { PermissionGate } from '../../components/primitives/permission-gate'

export function SecurityPage() {
  return (
    <section className="page-shell">
      <PermissionGate
        detail="Encryption mode, keyring usage, and recovery tradeoffs need their own explicit review path."
        eyebrow="SECURITY"
        title="Key handling remains a first-class workflow"
      >
        <button className="ghost-button" type="button">
          Review keyring preview
        </button>
      </PermissionGate>
    </section>
  )
}
