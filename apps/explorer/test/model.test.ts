import { describe, expect, it } from 'vitest'
import type { ScanReport } from '@resurrect-protocol/client'
import {
  DEFAULT_NAMESPACE,
  DEFAULT_RPC_URL,
  errorMessage,
  formatChainTime,
  networkDescriptor,
  normalizeRpcUrl,
  shortValue,
  summarizeScan
} from '../src/model.js'

function report(): ScanReport {
  return { startBlock: 100n, headBlock: 149n, headTimestamp: 2_000n, logsProcessed: 3, recordsRejected: 2, chunkReductions: 1, candidates: [] }
}

describe('explorer model', () => {
  it('pins the live namespace and canonical Ethereum deployment', () => {
    const descriptor = networkDescriptor()
    expect(descriptor.namespace).toBe(DEFAULT_NAMESPACE)
    expect(descriptor.registry.chainId).toBe(1n)
    expect(descriptor.registry.address).toBe('0x6F33c332e8251dcd307D85A27fCcAbd85d578910')
    expect(DEFAULT_RPC_URL).toBe('https://eth.drpc.org')
  })

  it('normalizes HTTP providers and rejects other transports', () => {
    expect(normalizeRpcUrl(' https://eth.drpc.org ')).toBe('https://eth.drpc.org/')
    expect(normalizeRpcUrl('http://127.0.0.1:8545')).toBe('http://127.0.0.1:8545/')
    expect(() => normalizeRpcUrl('wss://rpc.example')).toThrow(/HTTPS or HTTP/)
  })

  it('keeps announcements and browser peers as separate metrics', () => {
    expect(summarizeScan(report())).toEqual({ announcements: '3', browserPeers: '0', filteredRecords: '2', confirmedHead: '149', scannedBlocks: '50' })
  })

  it('formats chain time and long identifiers', () => {
    expect(formatChainTime(2_000n)).not.toMatch(/Invalid|range/)
    expect(formatChainTime(BigInt(Number.MAX_SAFE_INTEGER))).toBe('Timestamp out of range')
    expect(shortValue('0x1234567890abcdef', 6, 4)).toBe('0x1234…cdef')
    expect(shortValue('short')).toBe('short')
  })

  it('preserves useful error messages', () => {
    expect(errorMessage(new Error('provider failed'))).toBe('provider failed')
    expect(errorMessage('unknown failure')).toBe('unknown failure')
  })
})
