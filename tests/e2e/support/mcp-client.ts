/**
 * A minimal MCP client for end-to-end checks: starts the command Settings
 * shows (as Claude or another tool would), with this run's environment so
 * it finds this run's data folder, and calls tools over stdio
 * (newline-delimited JSON-RPC).
 */
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'

const SUFFIX = ' --worker mcp-server'

interface Pending {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
}

export async function startMcpClient(command: string) {
  if (!command.endsWith(SUFFIX))
    throw new Error(`unexpected MCP command: ${command}`)
  const child = spawn(
    command.slice(0, -SUFFIX.length),
    ['--worker', 'mcp-server'],
    { stdio: ['pipe', 'pipe', 'inherit'], env: process.env },
  )
  const pending = new Map<number, Pending>()
  let nextId = 1
  createInterface({ input: child.stdout }).on('line', (line) => {
    if (!line.trim()) return
    const message = JSON.parse(line) as {
      id?: number
      result?: unknown
      error?: { message: string }
    }
    if (message.id === undefined) return
    const waiting = pending.get(message.id)
    pending.delete(message.id)
    if (message.error) waiting?.reject(new Error(message.error.message))
    else waiting?.resolve(message.result)
  })
  child.on('exit', (code) => {
    for (const waiting of pending.values())
      waiting.reject(new Error(`the MCP server exited (${code})`))
    pending.clear()
  })

  const send = (message: object) =>
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', ...message })}\n`)
  const request = (method: string, params: object) =>
    new Promise<unknown>((resolve, reject) => {
      const id = nextId++
      pending.set(id, { resolve, reject })
      send({ id, method, params })
    })

  await request('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'pathkeep-e2e', version: '1' },
  })
  send({ method: 'notifications/initialized' })

  return {
    /** Calls a tool and returns its structured result. */
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- each tool answers its own shape
    async call(name: string, args: object): Promise<any> {
      const result = (await request('tools/call', {
        name,
        arguments: args,
      })) as {
        isError?: boolean
        structuredContent?: unknown
        content?: { text?: string }[]
      }
      if (result.isError)
        throw new Error(result.content?.[0]?.text ?? `${name} failed`)
      return (
        result.structuredContent ?? JSON.parse(result.content?.[0]?.text ?? '')
      )
    },
    close: () =>
      new Promise<void>((resolve) => {
        child.once('exit', () => resolve())
        child.stdin.end()
        setTimeout(() => child.kill(), 5_000).unref()
      }),
  }
}
