/**
 * Paper-redesign chrome shown above the v0.2 Import workflow when the route
 * has `?layout=paper`. The body slot is intentionally minimal — the actual
 * wizard surface still belongs to ImportWorkflowPanel below — but the
 * literary intro, the three method cards, and the stepper give the route
 * its paper identity while we keep the existing workflow logic.
 *
 * ## Responsibilities
 * - Build the three method-card descriptors from the import i18n catalog.
 * - Build the five stepper labels.
 * - Forward method selection (browser / takeout / file) to the route.
 *
 * ## Not responsible for
 * - Running the wizard — the v0.2 ImportWorkflowPanel below owns scan /
 *   preview / confirm / done.
 */

import {
  PaperImportView,
  type PaperImportMethod,
} from '@/components/explorer-paper'
import type { ImportMethod } from './shared'

export type PaperImportMethodId = 'browser' | 'takeout' | 'file'

export interface PaperImportPanelProps {
  activeMethod: ImportMethod
  onSelectMethod: (id: PaperImportMethodId) => void
  stepIndex: number
  importT: (key: string, vars?: Record<string, string | number>) => string
}

export function PaperImportPanel({
  activeMethod,
  onSelectMethod,
  stepIndex,
  importT,
}: PaperImportPanelProps) {
  const methods: PaperImportMethod[] = [
    {
      id: 'browser',
      title: importT('paperMethodBrowserTitle'),
      description: importT('paperMethodBrowserDescription'),
      hint: importT('paperMethodBrowserHint'),
    },
    {
      id: 'takeout',
      title: importT('paperMethodTakeoutTitle'),
      description: importT('paperMethodTakeoutDescription'),
      hint: importT('paperMethodTakeoutHint'),
    },
    {
      id: 'file',
      title: importT('paperMethodFileTitle'),
      description: importT('paperMethodFileDescription'),
      hint: importT('paperMethodFileHint'),
    },
  ]
  const steps = [
    importT('paperStepSelect'),
    importT('paperStepScan'),
    importT('paperStepPreview'),
    importT('paperStepConfirm'),
    importT('paperStepDone'),
  ]
  return (
    <div data-testid="paper-import-panel" className="mb-6">
      <PaperImportView
        intro={importT('paperIntro')}
        methods={methods}
        activeMethodId={activeMethod}
        onSelectMethod={(id) =>
          onSelectMethod(id === 'browser' || id === 'file' ? id : 'takeout')
        }
        steps={steps}
        currentStep={Math.max(0, stepIndex)}
        bodySlot={null}
        testId="paper-import-view"
      />
    </div>
  )
}
