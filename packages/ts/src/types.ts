import type { Address, Hex } from 'viem'

export interface JsonRegistryDescriptor {
  chainId: number | string
  address: Address
  deploymentBlock: number | string
  maxTtlSeconds: number
}

export interface JsonNetworkDescriptor {
  resurrectVersion: number
  registry: JsonRegistryDescriptor
  namespace: Hex
  acceptedRecordTypes: number[]
}

export interface RegistryDescriptor {
  chainId: bigint
  address: Address
  deploymentBlock: bigint
  maxTtlSeconds: number
}

export interface NetworkDescriptor {
  resurrectVersion: 1
  registry: RegistryDescriptor
  namespace: Hex
  acceptedRecordTypes: readonly number[]
}

export interface Eip1193RequestArguments {
  method: string
  params?: readonly unknown[] | object
}

export interface Eip1193Provider {
  request(args: Eip1193RequestArguments): Promise<unknown>
}

export interface RegistryProvider {
  request(method: string, params?: readonly unknown[]): Promise<unknown>
}

export interface BrowserPeerCandidate {
  recordType: 2
  peerId: string
  sequence: bigint
  endpoints: readonly string[]
  rawSignedRecord: Hex
  validUntil: bigint
  blockNumber: bigint
  logIndex: bigint
}

export interface ScanOptions {
  confirmations?: bigint
  initialChunkSize?: bigint
  minimumChunkSize?: bigint
  maxLogs?: number
  maxCandidates?: number
  maxEndpointsPerRecord?: number
  allowPrivateEndpoints?: boolean
}

export interface ScanReport {
  startBlock: bigint
  headBlock: bigint
  headTimestamp: bigint
  logsProcessed: number
  recordsRejected: number
  chunkReductions: number
  candidates: readonly BrowserPeerCandidate[]
}
