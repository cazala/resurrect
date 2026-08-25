import {
  encodeAbiParameters,
  parseAbiParameters,
  toEventSelector,
  toFunctionSelector,
  toHex,
  type Hex
} from 'viem'
import { describe, expect, it } from 'vitest'
import vectors from '../../../test-vectors/peer-records/libp2p-ed25519.json' with { type: 'json' }
import {
  RbpBrowserClient,
  deriveNamespace,
  injectedProvider,
  jsonRpcProvider,
  parseDescriptor,
  type NetworkDescriptor,
  type RegistryProvider
} from '../src/index.js'

const eventTopic = toEventSelector('PeerAnnounced(bytes32,uint32,uint64,bytes)')
const dataParameters = parseAbiParameters('uint64 validUntil, bytes peerRecord')

class MockProvider implements RegistryProvider {
  readonly calls: string[] = []
  readonly chainId: bigint
  readonly records: Array<{ validUntil: bigint; recordHex: Hex }>
  failLogs = false
  rangeFailures = 0

  constructor(
    chainId = 31337n,
    records: Array<{ validUntil: bigint; recordHex: Hex }> = [{
      validUntil: 2000n,
      recordHex: vectors.browser.recordHex as Hex
    }]
  ) {
    this.chainId = chainId
    this.records = records
  }

  async request(method: string, params: readonly unknown[] = []): Promise<unknown> {
    this.calls.push(method)
    if (method === 'eth_chainId') return toHex(this.chainId)
    if (method === 'eth_call') {
      const call = params[0] as { data: Hex }
      const constants = new Map([
        [toFunctionSelector('VERSION()'), 1n],
        [toFunctionSelector('MAX_TTL()'), 7_776_000n],
        [toFunctionSelector('MAX_RECORD_BYTES()'), 4096n]
      ])
      const value = constants.get(call.data)
      if (value == null) throw new Error('unexpected eth_call')
      return toHex(value, { size: 32 })
    }
    if (method === 'eth_getBlockByNumber') {
      const tag = params[0]
      const number = tag === 'latest' ? 10n : BigInt(String(tag))
      return { number: toHex(number), timestamp: toHex(1000n), hash: toHex(number, { size: 32 }) }
    }
    if (method === 'eth_getLogs') {
      if (this.failLogs) throw new Error('provider unavailable')
      if (this.rangeFailures > 0) {
        this.rangeFailures -= 1
        throw new Error('block range exceeds provider limit')
      }
      const range = params[0] as { fromBlock: Hex; toBlock: Hex }
      if (BigInt(range.fromBlock) > 8n || BigInt(range.toBlock) < 8n) return []
      return this.records.map((record, index) => ({
        address: '0x1111111111111111111111111111111111111111',
        topics: [eventTopic, descriptor().namespace, toHex(2, { size: 32 })],
        data: encodeAbiParameters(dataParameters, [record.validUntil, record.recordHex]),
        blockNumber: toHex(8),
        logIndex: toHex(index),
        removed: false
      }))
    }
    throw new Error(`unexpected method ${method}`)
  }
}

function descriptor(): NetworkDescriptor {
  return parseDescriptor({
    rbpVersion: 1,
    registry: {
      chainId: 31337,
      address: '0x1111111111111111111111111111111111111111',
      deploymentBlock: 1,
      maxTtlSeconds: 7_776_000
    },
    namespace: deriveNamespace('browser-scanner', 1),
    acceptedRecordTypes: [2]
  })
}

describe('browser registry scanner', () => {
  it('verifies chain/constants and recovers a browser seed', async () => {
    const provider = new MockProvider()
    const report = await new RbpBrowserClient(descriptor(), provider).scan({ confirmations: 0n })
    expect(report.startBlock).toBe(1n)
    expect(report.logsProcessed).toBe(1)
    expect(report.candidates).toHaveLength(1)
    expect(report.candidates[0]?.peerId).toBe(vectors.peerId)
    expect(provider.calls).not.toContain('eth_requestAccounts')
  })

  it('rejects the wrong chain before scanning', async () => {
    const provider = new MockProvider(1n)
    await expect(new RbpBrowserClient(descriptor(), provider).scan({ confirmations: 0n })).rejects.toThrow(
      /does not match/
    )
    expect(provider.calls).not.toContain('eth_getLogs')
  })

  it('switches away from a failing provider', async () => {
    const failing = new MockProvider()
    failing.failLogs = true
    const client = new RbpBrowserClient(descriptor(), failing)
    await expect(client.scan({ confirmations: 0n })).rejects.toThrow(/unavailable/)
    const replacement = new MockProvider()
    client.setProvider(replacement)
    await expect(client.scan({ confirmations: 0n })).resolves.toMatchObject({ logsProcessed: 1 })
  })

  it('scans through custom JSON-RPC and injected EIP-1193 providers without accounts', async () => {
    const backend = new MockProvider()
    const requested: string[] = []
    const fetcher = async (_input: string | URL | Request, init?: RequestInit): Promise<Response> => {
      const request = JSON.parse(String(init?.body)) as { id: number; method: string; params?: unknown[] }
      requested.push(request.method)
      const result = await backend.request(request.method, request.params ?? [])
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }))
    }
    const custom = jsonRpcProvider('https://rpc.example', { fetch: fetcher })
    await expect(new RbpBrowserClient(descriptor(), custom).scan({ confirmations: 0n })).resolves.toMatchObject({
      logsProcessed: 1
    })

    const injectedMethods: string[] = []
    const injected = injectedProvider({
      async request({ method, params = [] }) {
        injectedMethods.push(method)
        return backend.request(method, Array.isArray(params) ? params : [])
      }
    })
    await expect(new RbpBrowserClient(descriptor(), injected).scan({ confirmations: 0n })).resolves.toMatchObject({
      logsProcessed: 1
    })
    expect(requested).not.toContain('eth_requestAccounts')
    expect(injectedMethods).not.toContain('eth_requestAccounts')
  })

  it('reduces rejected provider ranges and deduplicates repeated peer records', async () => {
    const provider = new MockProvider(31337n, [
      { validUntil: 2000n, recordHex: vectors.browser.recordHex as Hex },
      { validUntil: 2000n, recordHex: vectors.browser.recordHex as Hex }
    ])
    provider.rangeFailures = 1
    const report = await new RbpBrowserClient(descriptor(), provider).scan({
      confirmations: 0n,
      initialChunkSize: 8n,
      minimumChunkSize: 1n
    })
    expect(report.chunkReductions).toBe(1)
    expect(report.logsProcessed).toBe(2)
    expect(report.candidates).toHaveLength(1)
  })

  it('filters expired and non-browser records while remaining bounded', async () => {
    const provider = new MockProvider(31337n, [
      { validUntil: 999n, recordHex: vectors.browser.recordHex as Hex },
      { validUntil: 2000n, recordHex: vectors.nativeOnly.recordHex as Hex },
      { validUntil: 2000n, recordHex: vectors.browser.recordHex as Hex }
    ])
    const report = await new RbpBrowserClient(descriptor(), provider).scan({
      confirmations: 0n,
      maxCandidates: 1
    })
    expect(report.recordsRejected).toBe(2)
    expect(report.candidates).toHaveLength(1)
  })
})
