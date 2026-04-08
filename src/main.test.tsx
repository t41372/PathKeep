import { beforeEach, describe, expect, test, vi } from 'vitest'

const { createRootMock, renderMock } = vi.hoisted(() => ({
  createRootMock: vi.fn(),
  renderMock: vi.fn(),
}))

vi.mock('react-dom/client', () => ({
  createRoot: createRootMock,
}))

vi.mock('./app', () => ({
  default: () => <div>App shell</div>,
}))

describe('main entrypoint', () => {
  beforeEach(() => {
    vi.resetModules()
    document.body.innerHTML = '<div id="root"></div>'
    document.documentElement.removeAttribute('data-theme')
    window.localStorage.clear()
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

  test('restores a persisted theme preference before rendering', async () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')

    await import('./main.tsx')

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('restores a persisted light theme preference before rendering', async () => {
    window.localStorage.setItem('pathkeep.theme', 'light')

    await import('./main.tsx')

    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('ignores unsupported persisted theme values', async () => {
    window.localStorage.setItem('pathkeep.theme', 'sepia')

    await import('./main.tsx')

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false)
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('keeps rendering if reading theme preference throws', async () => {
    const getItemSpy = vi
      .spyOn(Storage.prototype, 'getItem')
      .mockImplementation(() => {
        throw new Error('storage unavailable')
      })

    await import('./main.tsx')

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false)
    expect(renderMock).toHaveBeenCalledTimes(1)

    getItemSpy.mockRestore()
  })
})
