/**
 * @file operation-workflow.tsx
 * @description Canonical step-by-step workflow explainer for trust and import review surfaces.
 * @module components/review
 *
 * ## Responsibilities
 * - Render ordered workflow steps with files, commands, checklist items, and optional actions.
 * - Keep PME/trust workflow grammar out of the generic shell primitive bucket.
 *
 * ## Not responsible for
 * - Defining route-specific workflow steps or localized copy.
 * - Executing copy side effects directly beyond delegating to caller-owned handlers.
 *
 * ## Dependencies
 * - Depends on `StatusTag` for stable current/complete/pending status styling.
 * - Depends on callers to provide already-derived step data and copy handlers.
 *
 * ## Performance notes
 * - Pure render-only workflow list; callers should continue to keep step arrays bounded.
 */

import type { ReactNode } from 'react'
import { StatusTag } from '../ui'

/**
 * Shared workflow-step contract used by import and other trust review panels.
 *
 * The type lives next to the renderer so route owners can share one stable
 * step grammar instead of re-inventing ad-hoc step objects.
 */
export type WorkflowStep = {
  id: string
  title: string
  status: 'pending' | 'complete'
  summary: string
  reason: string
  files?: string[]
  commands?: string[]
  checklist?: string[]
  actions?: ReactNode
}

interface OperationWorkflowProps {
  actionLabel: string
  labels: {
    why: string
    files: string
    commands: string
    checklist: string
    copy: string
    current: string
    complete: string
    pending: string
    command: (index: number) => string
  }
  onCopy: (value: string) => Promise<void>
  steps: WorkflowStep[]
}

/**
 * Renders the canonical workflow explainer used by import/trust surfaces.
 *
 * This keeps step rendering, command copy affordances, and status styling in
 * one review owner while route modules stay focused on orchestration.
 */
export function OperationWorkflow({
  actionLabel,
  labels,
  onCopy,
  steps,
}: OperationWorkflowProps) {
  const currentIndex = steps.findIndex((step) => step.status !== 'complete')

  return (
    <ol className="workflowList" aria-label={actionLabel}>
      {steps.map((step, index) => {
        const displayStatus =
          step.status === 'complete'
            ? 'complete'
            : currentIndex === index
              ? 'current'
              : 'pending'

        return (
          <li
            aria-current={displayStatus === 'current' ? 'step' : undefined}
            className={`workflowStep ${displayStatus}`}
            key={step.id}
          >
            <div className="workflowMarker">
              <span>{index + 1}</span>
            </div>
            <div className="workflowCard">
              <div className="workflowHeader">
                <div>
                  <p className="sectionEyebrow">
                    {displayStatus === 'complete'
                      ? labels.complete
                      : displayStatus === 'current'
                        ? labels.current
                        : labels.pending}
                  </p>
                  <h3>{step.title}</h3>
                </div>
                <StatusTag
                  ariaLabel={
                    displayStatus === 'complete'
                      ? labels.complete
                      : displayStatus === 'current'
                        ? labels.current
                        : labels.pending
                  }
                  tone={
                    displayStatus === 'complete'
                      ? 'success'
                      : displayStatus === 'current'
                        ? 'info'
                        : 'neutral'
                  }
                >
                  {displayStatus === 'complete'
                    ? labels.complete
                    : displayStatus === 'current'
                      ? labels.current
                      : labels.pending}
                </StatusTag>
              </div>
              <p className="workflowSummary">{step.summary}</p>
              <div className="workflowSection">
                <strong>{labels.why}</strong>
                <p>{step.reason}</p>
              </div>
              {step.files?.length ? (
                <div className="workflowSection">
                  <strong>{labels.files}</strong>
                  <div className="artifactList">
                    {step.files.map((file) => (
                      <article className="artifactCard compactCard" key={file}>
                        <strong>{file}</strong>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.commands?.length ? (
                <div className="workflowSection">
                  <strong>{labels.commands}</strong>
                  <div className="generatedList">
                    {step.commands.map((command) => (
                      <article className="codeArtifact" key={command}>
                        <div className="artifactHeader">
                          <strong>{labels.command(index + 1)}</strong>
                          <button
                            className="ghostButton"
                            type="button"
                            onClick={() => void onCopy(command)}
                          >
                            {labels.copy}
                          </button>
                        </div>
                        <pre>{command}</pre>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.checklist?.length ? (
                <div className="workflowSection">
                  <strong>{labels.checklist}</strong>
                  <ol className="stepList">
                    {step.checklist.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ol>
                </div>
              ) : null}
              {step.actions ? (
                <div className="workflowActions">{step.actions}</div>
              ) : null}
            </div>
          </li>
        )
      })}
    </ol>
  )
}
