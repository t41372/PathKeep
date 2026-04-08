interface SkeletonProps {
  width?: string
  height?: string
  variant?: 'text' | 'block' | 'stat-card' | 'table-row'
  count?: number
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

export function DashboardSkeleton() {
  return (
    <div className="page-shell" aria-busy="true" aria-label="Loading dashboard">
      <div className="stats-row">
        <Skeleton variant="stat-card" count={4} />
      </div>
      <div className="dashboard-grid">
        <div className="dashboard-left">
          <Skeleton variant="block" height="208px" />
          <Skeleton variant="block" height="160px" />
        </div>
        <div className="dashboard-right">
          <Skeleton variant="block" height="182px" />
          <Skeleton variant="block" height="182px" />
        </div>
      </div>
    </div>
  )
}

export function TableSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="skeleton-table" aria-busy="true" aria-label="Loading table">
      <Skeleton variant="table-row" count={rows} />
    </div>
  )
}
