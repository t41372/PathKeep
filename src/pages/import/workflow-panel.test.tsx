/**
 * @file workflow-panel.test.tsx
 * @description Focused coverage for the Import workflow explainer composition.
 * @module pages/import
 *
 * ## Responsibilities
 * - Verify workflow command labels and copy actions are wired through the shared review primitive.
 * - Keep Import route tests focused on orchestration while this file owns the extracted render module.
 *
 * ## Not responsible for
 * - Re-testing the full import wizard body.
 * - Re-testing backend import execution or progress subscriptions.
 *
 * ## Dependencies
 * - Mocks the wizard panel so the workflow explainer can be tested in isolation.
 *
 * ## Performance notes
 * - Pure render and click coverage only.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { ImportWorkflowPanel } from './workflow-panel'

vi.mock('./wizard-panel', () => ({
  ImportWizardPanel: () => <div>wizard body</div>,
}))

describe('ImportWorkflowPanel', () => {
  test('renders workflow command labels and delegates command copy', async () => {
    const user = userEvent.setup()
    const onCopyWorkflowValue = vi.fn().mockResolvedValue(undefined)

    render(
      <I18nProvider>
        <ImportWorkflowPanel
          detectedBrowserProfiles={[]}
          importing={false}
          importTask={null}
          importResult={null}
          inspection={null}
          language="en"
          manualPathExpanded={false}
          method="takeout"
          selectedBrowserProfile={null}
          selectedBrowserProfileId={null}
          sourcePath=""
          step="select"
          stepIndex={0}
          wizardSteps={[]}
          workflowExpanded
          workflowSteps={[
            {
              id: 'scan',
              title: 'Scan source',
              status: 'pending',
              summary: 'Preview files before importing.',
              reason: 'Trust review stays ahead of execution.',
              commands: ['pathkeep scan /tmp/takeout'],
            },
          ]}
          onBrowseSource={vi.fn()}
          onCopyWorkflowValue={onCopyWorkflowValue}
          onImport={vi.fn()}
          onImportAnother={vi.fn()}
          onManualPathExpandedChange={vi.fn()}
          onMethodChange={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onScan={vi.fn()}
          onSelectBrowserProfile={vi.fn()}
          onSourcePathChange={vi.fn()}
          onStepChange={vi.fn()}
          onWorkflowExpandedChange={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Command 1')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopyWorkflowValue).toHaveBeenCalledWith(
      'pathkeep scan /tmp/takeout',
    )
  })
})
