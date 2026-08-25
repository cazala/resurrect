import type { Eip1193Provider, RegistryProvider } from './types.js'

export interface JsonRpcHttpOptions {
  fetch?: typeof globalThis.fetch
  headers?: Readonly<Record<string, string>>
}

export function jsonRpcProvider(url: string, options: JsonRpcHttpOptions = {}): RegistryProvider {
  const parsed = new URL(url)
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error('JSON-RPC URL must use HTTP or HTTPS')
  }
  const fetcher = options.fetch ?? globalThis.fetch
  if (fetcher == null) throw new Error('Fetch API is unavailable')
  let id = 0
  return {
    async request(method, params = []) {
      const response = await fetcher(parsed.href, {
        method: 'POST',
        headers: { 'content-type': 'application/json', ...options.headers },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params })
      })
      if (!response.ok) throw new Error(`JSON-RPC HTTP error ${response.status}`)
      const payload = (await response.json()) as { result?: unknown; error?: { code: number; message: string } }
      if (payload.error != null) throw new Error(`JSON-RPC ${payload.error.code}: ${payload.error.message}`)
      if (!('result' in payload)) throw new Error('JSON-RPC response has no result')
      return payload.result
    }
  }
}

export function injectedProvider(provider: Eip1193Provider): RegistryProvider {
  return {
    request(method, params = []) {
      return provider.request({ method, params })
    }
  }
}

export function persistJsonRpcUrl(
  url: string,
  storage: Pick<Storage, 'setItem'>,
  key = 'rbp.rpcUrl'
): void {
  const parsed = new URL(url)
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error('JSON-RPC URL must use HTTP or HTTPS')
  }
  storage.setItem(key, parsed.href)
}
