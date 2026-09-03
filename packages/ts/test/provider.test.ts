import { describe, expect, it, vi } from 'vitest'
import { injectedProvider, jsonRpcProvider, persistJsonRpcUrl } from '../src/index.js'

describe('registry provider privacy', () => {
  it('delegates injected reads without requesting accounts', async () => {
    const request = vi.fn(async ({ method }: { method: string }) => method === 'eth_chainId' ? '0x1' : null)
    const provider = injectedProvider({ request })
    await provider.request('eth_chainId')
    expect(request).toHaveBeenCalledWith({ method: 'eth_chainId', params: [] })
    expect(request.mock.calls.some(([argument]) => argument.method === 'eth_requestAccounts')).toBe(false)
  })

  it('keeps a custom URL in memory unless persistence is explicit', async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x1' })))
    const storage = { setItem: vi.fn() }
    const provider = jsonRpcProvider('https://rpc.example/private-token', { fetch: fetcher })
    expect(storage.setItem).not.toHaveBeenCalled()
    await expect(provider.request('eth_chainId')).resolves.toBe('0x1')
    expect(fetcher).toHaveBeenCalledWith(
      'https://rpc.example/private-token',
      expect.objectContaining({ method: 'POST' })
    )
    expect(storage.setItem).not.toHaveBeenCalled()
    persistJsonRpcUrl('https://rpc.example/private-token', storage)
    expect(storage.setItem).toHaveBeenCalledOnce()
  })

  it('preserves JSON-RPC range errors returned with an HTTP error status', async () => {
    const fetcher = vi.fn(async () => new Response(
      JSON.stringify({ jsonrpc: '2.0', id: 1, error: { code: -32602, message: 'block range is too wide' } }),
      { status: 400 }
    ))
    const provider = jsonRpcProvider('https://rpc.example', { fetch: fetcher })
    await expect(provider.request('eth_getLogs')).rejects.toThrow('JSON-RPC -32602: block range is too wide')
  })
})
