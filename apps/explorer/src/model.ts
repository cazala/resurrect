import {
  ethereumMainnetDescriptor,
  type NetworkDescriptor,
  type ScanReport
} from '@resurrect-protocol/client'

export const DEFAULT_RPC_URL = 'https://eth.drpc.org'
export const DEFAULT_NAMESPACE = '0x0c07fdd466a110bea1916247b73191c331123bbc77b010462676a10d1c3928e2'

export interface ScanSummary {
  announcements: string
  browserPeers: string
  filteredRecords: string
  confirmedHead: string
  scannedBlocks: string
}

export function networkDescriptor(): NetworkDescriptor {
  return ethereumMainnetDescriptor(DEFAULT_NAMESPACE)
}

export function normalizeRpcUrl(value: string): string {
  const url = new URL(value.trim())
  if (url.protocol !== 'https:' && url.protocol !== 'http:') {
    throw new Error('RPC URL must use HTTPS or HTTP')
  }
  return url.href
}

export function summarizeScan(report: ScanReport): ScanSummary {
  return {
    announcements: report.logsProcessed.toLocaleString(),
    browserPeers: report.candidates.length.toLocaleString(),
    filteredRecords: report.recordsRejected.toLocaleString(),
    confirmedHead: report.headBlock.toLocaleString(),
    scannedBlocks: (report.headBlock - report.startBlock + 1n).toLocaleString()
  }
}

export function formatChainTime(timestamp: bigint): string {
  const milliseconds = Number(timestamp) * 1000
  if (!Number.isSafeInteger(milliseconds)) return 'Timestamp out of range'
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return 'Invalid timestamp'
  return date.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })
}

export function shortValue(value: string, leading = 10, trailing = 8): string {
  if (value.length <= leading + trailing + 1) return value
  return `${value.slice(0, leading)}…${value.slice(-trailing)}`
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
