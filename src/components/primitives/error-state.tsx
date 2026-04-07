interface ErrorStateProps {
  title: string
  description: string
}

export function ErrorState({ title, description }: ErrorStateProps) {
  return (
    <section className="utility-block utility-block--danger" role="alert">
      <span className="mono-kicker">ATTENTION</span>
      <h2>{title}</h2>
      <p>{description}</p>
    </section>
  )
}
