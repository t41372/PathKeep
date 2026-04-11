interface SkeletonProps {
  width?: string
  height?: string
  variant?: 'text' | 'block' | 'stat-card' | 'table-row'
  count?: number
}

interface LabeledSkeletonProps {
  label: string
}

function SkeletonUnit({
  width,
  height,
  variant = 'text',
}: Omit<SkeletonProps, 'count'>) {
  if (variant === 'stat-card') {
    return (
      <div className="skeleton skeleton--stat-card">
        <div
          className="skeleton__line"
          style={{ width: '56%', height: '10px' }}
        />
        <div
          className="skeleton__line"
          style={{ width: '42%', height: '24px', marginTop: '8px' }}
        />
        <div
          className="skeleton__line"
          style={{ width: '48%', height: '10px', marginTop: '8px' }}
        />
      </div>
    )
  }

  if (variant === 'table-row') {
    return (
      <div className="skeleton skeleton--table-row">
        <div className="skeleton__line" style={{ width: '16%' }} />
        <div className="skeleton__line" style={{ width: '34%' }} />
        <div className="skeleton__line" style={{ width: '22%' }} />
        <div className="skeleton__line" style={{ width: '10%' }} />
      </div>
    )
  }

  if (variant === 'block') {
    return (
      <div
        className="skeleton skeleton--block"
        style={{ width: width ?? '100%', height: height ?? '120px' }}
      />
    )
  }

  return (
    <div
      className="skeleton skeleton--text"
      style={{ width: width ?? '100%', height: height ?? '12px' }}
    />
  )
}

export function Skeleton({
  width,
  height,
  variant = 'text',
  count = 1,
}: SkeletonProps) {
  return (
    <>
      {Array.from({ length: count }, (_, index) => (
        <SkeletonUnit
          key={`${variant}-${index}`}
          width={width}
          height={height}
          variant={variant}
        />
      ))}
    </>
  )
}

export function DashboardSkeleton({ label }: LabeledSkeletonProps) {
  return (
    <div className="page-shell" aria-busy="true" aria-label={label}>
      <div className="stats-row">
        <Skeleton variant="stat-card" count={4} />
      </div>
      <div className="dashboard-grid">
        <div className="dashboard-left">
          <Skeleton variant="block" height="260px" />
          <Skeleton variant="block" height="182px" />
        </div>
        <div className="dashboard-right">
          <Skeleton variant="block" height="196px" />
          <Skeleton variant="block" height="196px" />
        </div>
      </div>
    </div>
  )
}

export function TableSkeleton({
  label,
  rows = 5,
}: LabeledSkeletonProps & { rows?: number }) {
  return (
    <div className="skeleton-table" aria-busy="true" aria-label={label}>
      <Skeleton variant="table-row" count={rows} />
    </div>
  )
}

export function SkeletonExplorer({ label }: LabeledSkeletonProps) {
  return (
    <div className="page-shell" aria-busy="true" aria-label={label}>
      <div
        className="skeleton-block"
        style={{ height: '44px', marginBottom: 'var(--space-4)' }}
      />
      <div style={{ display: 'flex', gap: 'var(--space-4)' }}>
        <div style={{ flex: 1 }}>
          <Skeleton variant="block" height="32px" />
          {Array.from({ length: 6 }, (_, i) => (
            <div
              key={i}
              className="skeleton-block"
              style={{ height: '48px', marginBottom: 'var(--space-2)' }}
            />
          ))}
        </div>
        <div style={{ width: '320px' }}>
          <Skeleton variant="block" height="200px" />
        </div>
      </div>
    </div>
  )
}

export function SkeletonExplorerResults({ label }: LabeledSkeletonProps) {
  return (
    <div className="explorer-grid" aria-busy="true" aria-label={label}>
      <div className="record-list">
        <div className="record-group">
          <div className="record-group-header">
            <div
              className="skeleton-block"
              style={{ height: '18px', width: '42%' }}
            />
          </div>
          <div className="panel-body" style={{ padding: 0 }}>
            {Array.from({ length: 6 }, (_, index) => (
              <div
                key={index}
                className="skeleton-block"
                style={{
                  height: '70px',
                  marginBottom: 'var(--space-2)',
                }}
              />
            ))}
          </div>
        </div>
      </div>
      <div className="panel">
        <div className="panel-header">
          <div
            className="skeleton-block"
            style={{ height: '18px', width: '36%' }}
          />
        </div>
        <div className="panel-body intelligence-stack">
          <Skeleton variant="block" height="120px" />
          <Skeleton variant="block" height="80px" />
          <Skeleton variant="block" height="160px" />
        </div>
      </div>
    </div>
  )
}

export function SkeletonInsights({ label }: LabeledSkeletonProps) {
  return (
    <div className="page-shell" aria-busy="true" aria-label={label}>
      <div className="stats-row">
        <Skeleton variant="stat-card" count={4} />
      </div>
      <div
        style={{
          display: 'flex',
          gap: 'var(--space-4)',
          marginTop: 'var(--space-4)',
        }}
      >
        <div style={{ flex: 1 }}>
          <Skeleton variant="block" height="220px" />
        </div>
        <div style={{ flex: 1 }}>
          <Skeleton variant="block" height="220px" />
        </div>
      </div>
      <div style={{ marginTop: 'var(--space-4)' }}>
        <Skeleton variant="block" height="160px" />
      </div>
    </div>
  )
}
