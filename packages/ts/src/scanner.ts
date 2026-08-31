import {
  bytesToHex,
  concat,
  decodeAbiParameters,
  encodeFunctionData,
  keccak256,
  parseAbi,
  parseAbiParameters,
  toEventSelector,
  type Hex
} from 'viem'
import { decodeBrowserPeerRecord } from './peer-record.js'
import type {
  BrowserPeerCandidate,
  NetworkDescriptor,
  RegistryProvider,
  ScanOptions,
  ScanReport
} from './types.js'

const registryAbi = parseAbi([
  'function VERSION() view returns (uint32)',
  'function MAX_TTL() view returns (uint32)',
  'function MAX_RECORD_BYTES() view returns (uint32)'
])
const eventTopic = toEventSelector('PeerAnnounced(bytes32,uint32,uint64,bytes)')
const eventData = parseAbiParameters('uint64 validUntil, bytes peerRecord')

interface RpcBlock {
  number: Hex
  timestamp: Hex
  hash: Hex
}

interface RpcLog {
  address: Hex
  topics: Hex[]
  data: Hex
  blockNumber: Hex
  logIndex: Hex
  removed?: boolean
}

export async function scanRegistry(
  provider: RegistryProvider,
  descriptor: NetworkDescriptor,
  options: ScanOptions = {}
): Promise<ScanReport> {
  validateOptions(options)
  await verifyProvider(provider, descriptor)
  const latest = await getBlock(provider, 'latest')
  const confirmations = options.confirmations ?? 12n
  const headNumber = BigInt(latest.number) > confirmations ? BigInt(latest.number) - confirmations : 0n
  if (headNumber < descriptor.registry.deploymentBlock) throw new Error('registry deployment is newer than confirmed head')
  const head = headNumber === BigInt(latest.number) ? latest : await getBlock(provider, quantity(headNumber))
  const headTimestamp = BigInt(head.timestamp)
  const cutoff = headTimestamp > BigInt(descriptor.registry.maxTtlSeconds)
    ? headTimestamp - BigInt(descriptor.registry.maxTtlSeconds)
    : 0n
  const startBlock = await findStartBlock(provider, descriptor.registry.deploymentBlock, headNumber, cutoff)
  const accepted = new Set(descriptor.acceptedRecordTypes)
  const maxLogs = options.maxLogs ?? 50_000
  const maxCandidates = options.maxCandidates ?? 256
  let chunk = options.initialChunkSize ?? 20_000n
  const minimumChunk = options.minimumChunkSize ?? 64n
  let from = startBlock
  let logsProcessed = 0
  let recordsRejected = 0
  let chunkReductions = 0
  const candidates = new Map<string, BrowserPeerCandidate>()
  while (from <= headNumber && logsProcessed < maxLogs) {
    const to = min(from + maxBigInt(1n, chunk) - 1n, headNumber)
    let logs: RpcLog[]
    try {
      logs = asLogs(await provider.request('eth_getLogs', [{
        address: descriptor.registry.address,
        topics: [eventTopic, descriptor.namespace],
        fromBlock: quantity(from),
        toBlock: quantity(to)
      }]))
    } catch (error) {
      if (chunk > minimumChunk && isRangeError(error)) {
        chunk = maxBigInt(chunk / 2n, minimumChunk)
        chunkReductions += 1
        continue
      }
      throw error
    }
    for (const log of logs) {
      if (logsProcessed >= maxLogs) break
      logsProcessed += 1
      try {
        if (log.removed === true || log.address.toLowerCase() !== descriptor.registry.address.toLowerCase()) throw new Error('wrong log source')
        if (log.topics[0]?.toLowerCase() !== eventTopic.toLowerCase() || log.topics[1]?.toLowerCase() !== descriptor.namespace.toLowerCase()) throw new Error('wrong event')
        const recordType = Number(BigInt(required(log.topics[2], 'record type topic')))
        if (!accepted.has(recordType) || recordType !== 2) throw new Error('unsupported record type')
        const [validUntil, peerRecord] = decodeAbiParameters(eventData, log.data)
        if (validUntil <= headTimestamp) throw new Error('expired record')
        const candidate = await decodeBrowserPeerRecord(
          peerRecord,
          validUntil,
          BigInt(log.blockNumber),
          BigInt(log.logIndex),
          {
            maxEndpoints: options.maxEndpointsPerRecord,
            allowPrivateEndpoints: options.allowPrivateEndpoints
          }
        )
        insertBounded(candidates, candidate, maxCandidates, descriptor.namespace, head.hash)
      } catch {
        recordsRejected += 1
      }
    }
    if (to === headNumber) break
    from = to + 1n
  }
  return {
    startBlock,
    headBlock: headNumber,
    headTimestamp,
    logsProcessed,
    recordsRejected,
    chunkReductions,
    candidates: [...candidates.values()].sort(compareCandidates)
  }
}

function validateOptions(options: ScanOptions): void {
  if ((options.confirmations ?? 0n) < 0n) throw new Error('confirmations must not be negative')
  if ((options.initialChunkSize ?? 1n) < 1n) throw new Error('initialChunkSize must be positive')
  if ((options.minimumChunkSize ?? 1n) < 1n) throw new Error('minimumChunkSize must be positive')
  for (const [field, value] of [
    ['maxLogs', options.maxLogs],
    ['maxCandidates', options.maxCandidates],
    ['maxEndpointsPerRecord', options.maxEndpointsPerRecord]
  ] as const) {
    if (value != null && (!Number.isSafeInteger(value) || value < 1)) {
      throw new Error(`${field} must be a positive integer`)
    }
  }
}

export async function verifyProvider(provider: RegistryProvider, descriptor: NetworkDescriptor): Promise<void> {
  const chainId = BigInt(asHex(await provider.request('eth_chainId'), 'chain ID'))
  if (chainId !== descriptor.registry.chainId) {
    throw new Error(`provider chain ID ${chainId} does not match descriptor chain ID ${descriptor.registry.chainId}`)
  }
  const [version, maxTtl, maxRecord] = await Promise.all([
    readConstant(provider, descriptor.registry.address, 'VERSION'),
    readConstant(provider, descriptor.registry.address, 'MAX_TTL'),
    readConstant(provider, descriptor.registry.address, 'MAX_RECORD_BYTES')
  ])
  if (version !== 1n || maxTtl !== 7_776_000n || maxRecord !== 4096n || maxTtl !== BigInt(descriptor.registry.maxTtlSeconds)) {
    throw new Error('deployed registry constants do not match Resurrect v1')
  }
}

async function readConstant(
  provider: RegistryProvider,
  address: Hex,
  name: 'VERSION' | 'MAX_TTL' | 'MAX_RECORD_BYTES'
): Promise<bigint> {
  const data = encodeFunctionData({ abi: registryAbi, functionName: name })
  return BigInt(asHex(await provider.request('eth_call', [{ to: address, data }, 'latest']), name))
}

async function findStartBlock(provider: RegistryProvider, deployment: bigint, head: bigint, cutoff: bigint): Promise<bigint> {
  let low = deployment
  let high = head
  while (low < high) {
    const middle = low + (high - low) / 2n
    const block = await getBlock(provider, quantity(middle))
    if (BigInt(block.timestamp) < cutoff) low = middle + 1n
    else high = middle
  }
  return low
}

async function getBlock(provider: RegistryProvider, tag: string): Promise<RpcBlock> {
  const value = await provider.request('eth_getBlockByNumber', [tag, false])
  if (!isRecord(value) || !isHexValue(value.number) || !isHexValue(value.timestamp) || !isHexValue(value.hash)) {
    throw new Error(`provider returned malformed block ${tag}`)
  }
  return value as unknown as RpcBlock
}

function asLogs(value: unknown): RpcLog[] {
  if (!Array.isArray(value)) throw new Error('provider returned malformed logs')
  return value.map((log) => {
    if (!isRecord(log) || !isHexValue(log.address) || !Array.isArray(log.topics) || !log.topics.every(isHexValue) || !isHexValue(log.data) || !isHexValue(log.blockNumber) || !isHexValue(log.logIndex)) {
      throw new Error('provider returned malformed log')
    }
    return log as unknown as RpcLog
  })
}

function insertBounded(
  store: Map<string, BrowserPeerCandidate>,
  candidate: BrowserPeerCandidate,
  cap: number,
  namespace: Hex,
  headHash: Hex
): void {
  const existing = store.get(candidate.peerId)
  if (existing != null) {
    if (compareFreshness(candidate, existing) > 0) store.set(candidate.peerId, candidate)
    return
  }
  if (store.size < cap) {
    store.set(candidate.peerId, candidate)
    return
  }
  const incomingScore = score(namespace, headHash, candidate.peerId)
  let worst: { peerId: string; score: bigint } | undefined
  for (const peerId of store.keys()) {
    const value = score(namespace, headHash, peerId)
    if (worst == null || value > worst.score) worst = { peerId, score: value }
  }
  if (worst != null && incomingScore < worst.score) {
    store.delete(worst.peerId)
    store.set(candidate.peerId, candidate)
  }
}

function score(namespace: Hex, headHash: Hex, peerId: string): bigint {
  return BigInt(keccak256(concat([headHash, namespace, bytesToHex(new TextEncoder().encode(peerId))])))
}

function compareFreshness(left: BrowserPeerCandidate, right: BrowserPeerCandidate): number {
  if (left.sequence !== right.sequence) return left.sequence > right.sequence ? 1 : -1
  if (left.blockNumber !== right.blockNumber) return left.blockNumber > right.blockNumber ? 1 : -1
  if (left.logIndex !== right.logIndex) return left.logIndex > right.logIndex ? 1 : -1
  return 0
}

function compareCandidates(left: BrowserPeerCandidate, right: BrowserPeerCandidate): number {
  return compareFreshness(right, left) || left.peerId.localeCompare(right.peerId)
}

function quantity(value: bigint): Hex {
  return `0x${value.toString(16)}`
}

function asHex(value: unknown, field: string): Hex {
  if (!isHexValue(value)) throw new Error(`provider returned invalid ${field}`)
  return value
}

function isHexValue(value: unknown): value is Hex {
  return typeof value === 'string' && /^0x[0-9a-fA-F]+$/.test(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function required<T>(value: T | undefined, field: string): T {
  if (value == null) throw new Error(`missing ${field}`)
  return value
}

function isRangeError(error: unknown): boolean {
  const message = error instanceof Error ? error.message.toLowerCase() : String(error).toLowerCase()
  return ['range', 'too many', 'response size', 'limit exceeded'].some((needle) => message.includes(needle))
}

function min(left: bigint, right: bigint): bigint {
  return left < right ? left : right
}

function maxBigInt(left: bigint, right: bigint): bigint {
  return left > right ? left : right
}
