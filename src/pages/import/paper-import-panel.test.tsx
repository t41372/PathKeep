import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { PaperImportPanel } from './paper-import-panel'

function importT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

describe('PaperImportPanel', () => {
  test('renders intro + three method cards + five stepper steps', () => {
    render(
      <PaperImportPanel
        activeMethod="takeout"
        onSelectMethod={() => {}}
        stepIndex={2}
        importT={importT}
      />,
    )
    expect(screen.getByTestId('paper-import-panel')).toBeInTheDocument()
    expect(screen.getByTestId('paper-import-view')).toBeInTheDocument()
    expect(screen.getByText('paperIntro')).toBeInTheDocument()
    // three method cards each tagged with their id
    expect(
      screen.getByTestId('paper-import-method-browser'),
    ).toBeInTheDocument()
    expect(
      screen.getByTestId('paper-import-method-takeout'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('paper-import-method-file')).toBeInTheDocument()
  })

  test('forwards file selection through onSelectMethod', () => {
    const onSelectMethod = vi.fn()
    render(
      <PaperImportPanel
        activeMethod="takeout"
        onSelectMethod={onSelectMethod}
        stepIndex={0}
        importT={importT}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-import-method-file'))
    expect(onSelectMethod).toHaveBeenCalledWith('file')
  })

  test('forwards browser selection through onSelectMethod', () => {
    const onSelectMethod = vi.fn()
    render(
      <PaperImportPanel
        activeMethod="takeout"
        onSelectMethod={onSelectMethod}
        stepIndex={0}
        importT={importT}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-import-method-browser'))
    expect(onSelectMethod).toHaveBeenCalledWith('browser')
  })

  test('takeout selection routes back to the takeout method id', () => {
    const onSelectMethod = vi.fn()
    render(
      <PaperImportPanel
        activeMethod="browser"
        onSelectMethod={onSelectMethod}
        stepIndex={0}
        importT={importT}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-import-method-takeout'))
    expect(onSelectMethod).toHaveBeenCalledWith('takeout')
  })

  test('a negative stepIndex clamps to step 0', () => {
    render(
      <PaperImportPanel
        activeMethod="takeout"
        onSelectMethod={() => {}}
        stepIndex={-3}
        importT={importT}
      />,
    )
    // The first step is "active", the rest "idle"
    expect(
      screen.getByTestId('paper-import-step-0').getAttribute('data-step'),
    ).toBe('active')
    expect(
      screen.getByTestId('paper-import-step-4').getAttribute('data-step'),
    ).toBe('idle')
  })
})
