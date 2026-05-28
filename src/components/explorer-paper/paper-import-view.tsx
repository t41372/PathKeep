/**
 * PaperImportView — composed Import wizard shell.
 *
 * Layout mirrors `pk-import.jsx` ImportView:
 *
 *   italic intro line
 *   ┌── method cards ──┐ ┌── method cards ──┐ ┌── method cards ──┐
 *   └──────────────────┘ └──────────────────┘ └──────────────────┘
 *
 *   ┌── wizard shell ─────────────────────────────────────────┐
 *   │   stepper                                               │
 *   ├─────────────────────────────────────────────────────────┤
 *   │   body slot (route owns the preview / confirm content)  │
 *   └─────────────────────────────────────────────────────────┘
 *
 * ## Responsibilities
 * - Render the intro paragraph, the method cards grid, and the wizard
 *   shell with stepper + body slot.
 * - Surface method selection via onSelectMethod.
 *
 * ## Not responsible for
 * - The body content of each step — the route renders Preview / Confirm /
 *   etc. inside the supplied `bodySlot` because those are tightly coupled
 *   to import-preview backend data.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperImportMethodCard,
  type PaperImportMethodCardProps,
} from './paper-import-method-card'
import {
  PaperImportStepper,
  type PaperImportStepperProps,
} from './paper-import-stepper'

export type PaperImportMethod = Pick<
  PaperImportMethodCardProps,
  'id' | 'title' | 'description' | 'hint' | 'icon'
>

export interface PaperImportViewProps {
  intro: ReactNode
  methods: readonly PaperImportMethod[]
  activeMethodId: string | null
  onSelectMethod?: (id: string) => void
  steps: PaperImportStepperProps['steps']
  currentStep: PaperImportStepperProps['currentStep']
  bodySlot: ReactNode
  className?: string
  testId?: string
}

export function PaperImportView({
  intro,
  methods,
  activeMethodId,
  onSelectMethod,
  steps,
  currentStep,
  bodySlot,
  className,
  testId,
}: PaperImportViewProps) {
  return (
    <section
      data-testid={testId}
      className={cn('flex w-full flex-col', className)}
    >
      <p
        data-testid="paper-import-intro"
        className="text-ink-muted m-0 mb-[14px] max-w-[580px] font-serif text-[14px] italic leading-[1.5]"
      >
        {intro}
      </p>

      <div
        data-testid="paper-import-methods"
        className="mb-6 grid grid-cols-1 gap-3 md:grid-cols-3"
      >
        {methods.map((method) => (
          <PaperImportMethodCard
            key={method.id}
            id={method.id}
            title={method.title}
            description={method.description}
            hint={method.hint}
            icon={method.icon}
            active={method.id === activeMethodId}
            onSelect={onSelectMethod}
          />
        ))}
      </div>

      <div
        data-testid="paper-import-wizard"
        className="border-border-light rounded-paper bg-card-paper border"
      >
        <PaperImportStepper steps={steps} currentStep={currentStep} />
        <div className="p-6">{bodySlot}</div>
      </div>
    </section>
  )
}
