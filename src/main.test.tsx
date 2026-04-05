import { beforeEach, describe, expect, test, vi } from 'vitest'

const { createRootMock, renderMock } = vi.hoisted(() => ({
  createRootMock: vi.fn(),
  renderMock: vi.fn(),
}))

vi.mock('react-dom/client', () => ({
  createRoot: createRootMock,
}))

vi.mock('./App.tsx', () => ({
  default: () => <div>App shell</div>,
}))

describe('main entrypoint', () => {
  beforeEach(() => {
    vi.resetModules()
    document.body.innerHTML = '<div id="root"></div>'
    renderMock.mockReset()
    createRootMock.mockReset().mockReturnValue({
      render: renderMock,
    })
  })

  test('mounts the React application into the root element', async () => {
    await import('./main.tsx')

    expect(createRootMock).toHaveBeenCalledWith(document.getElementById('root'))
    expect(renderMock).toHaveBeenCalledTimes(1)
  })
})
