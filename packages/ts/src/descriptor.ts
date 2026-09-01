import { getAddress, isAddress, isHex, keccak256, stringToHex } from 'viem'
import type { JsonNetworkDescriptor, NetworkDescriptor, RegistryDescriptor } from './types.js'

export const RESURRECT_VERSION = 1 as const
export const MAX_TTL_SECONDS = 7_776_000
export const ETHEREUM_MAINNET_CHAIN_ID = 1n
export const ETHEREUM_MAINNET_REGISTRY_ADDRESS = getAddress(
  '0x6F33c332e8251dcd307D85A27fCcAbd85d578910'
)
export const ETHEREUM_MAINNET_REGISTRY_DEPLOYMENT_BLOCK = 25_882_327n
export const ETHEREUM_MAINNET_REGISTRY: Readonly<RegistryDescriptor> = Object.freeze({
  chainId: ETHEREUM_MAINNET_CHAIN_ID,
  address: ETHEREUM_MAINNET_REGISTRY_ADDRESS,
  deploymentBlock: ETHEREUM_MAINNET_REGISTRY_DEPLOYMENT_BLOCK,
  maxTtlSeconds: MAX_TTL_SECONDS
})
const UINT64_MAX = (1n << 64n) - 1n
const UINT256_MAX = (1n << 256n) - 1n

export function deriveNamespace(application: string, majorVersion: bigint | number): `0x${string}` {
  if (application.length === 0) throw new Error('application identifier must not be empty')
  const major = BigInt(majorVersion)
  if (major < 0n) throw new Error('major protocol version must not be negative')
  return keccak256(stringToHex(`resurrect:${application}:${major}`))
}

export function ethereumMainnetDescriptor(
  namespace: `0x${string}`,
  acceptedRecordTypes: readonly number[] = [2]
): NetworkDescriptor {
  return parseDescriptor({
    resurrectVersion: RESURRECT_VERSION,
    registry: {
      chainId: Number(ETHEREUM_MAINNET_CHAIN_ID),
      address: ETHEREUM_MAINNET_REGISTRY_ADDRESS,
      deploymentBlock: Number(ETHEREUM_MAINNET_REGISTRY_DEPLOYMENT_BLOCK),
      maxTtlSeconds: MAX_TTL_SECONDS
    },
    namespace,
    acceptedRecordTypes: [...acceptedRecordTypes]
  })
}

export function parseDescriptor(input: unknown): NetworkDescriptor {
  if (!isRecord(input)) throw new Error('descriptor must be an object')
  assertExactKeys(input, ['acceptedRecordTypes', 'namespace', 'registry', 'resurrectVersion'])
  if (input.resurrectVersion !== RESURRECT_VERSION) throw new Error('unsupported Resurrect version')
  if (!isRecord(input.registry)) throw new Error('descriptor registry must be an object')
  assertExactKeys(input.registry, ['address', 'chainId', 'deploymentBlock', 'maxTtlSeconds'])
  if (typeof input.registry.address !== 'string' || !isAddress(input.registry.address, { strict: true })) {
    throw new Error('invalid registry address')
  }
  if (input.registry.maxTtlSeconds !== MAX_TTL_SECONDS) {
    throw new Error('registry max TTL does not match Resurrect v1')
  }
  if (typeof input.namespace !== 'string' || !isHex(input.namespace) || input.namespace.length !== 66) {
    throw new Error('invalid namespace')
  }
  if (!Array.isArray(input.acceptedRecordTypes) || input.acceptedRecordTypes.length === 0) {
    throw new Error('acceptedRecordTypes must not be empty')
  }
  const accepted = input.acceptedRecordTypes.map((value) => {
    if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new Error('record type must be uint32')
    }
    return value
  })
  if (new Set(accepted).size !== accepted.length) throw new Error('duplicate record type')
  const chainId = parseUnsigned(input.registry.chainId, 'chainId', UINT256_MAX)
  const deploymentBlock = parseUnsigned(input.registry.deploymentBlock, 'deploymentBlock', UINT64_MAX)
  return {
    resurrectVersion: RESURRECT_VERSION,
    registry: {
      chainId,
      address: getAddress(input.registry.address),
      deploymentBlock,
      maxTtlSeconds: MAX_TTL_SECONDS
    },
    namespace: input.namespace.toLowerCase() as `0x${string}`,
    acceptedRecordTypes: Object.freeze([...accepted].sort((left, right) => left - right))
  }
}

export function parseDescriptorJson(json: string): NetworkDescriptor {
  return parseDescriptor(JSON.parse(json) as JsonNetworkDescriptor)
}

function parseUnsigned(value: unknown, field: string, maximum: bigint): bigint {
  if (typeof value !== 'number' && typeof value !== 'string') throw new Error(`${field} must be an integer`)
  if (typeof value === 'number' && !Number.isSafeInteger(value)) {
    throw new Error(`${field} exceeds JSON safe integer range; encode it as a decimal string`)
  }
  if (typeof value === 'string' && !/^(0|[1-9][0-9]*)$/.test(value)) {
    throw new Error(`${field} must be an unsigned decimal integer`)
  }
  const parsed = BigInt(value)
  if (parsed < 0n) throw new Error(`${field} must not be negative`)
  if (parsed > maximum) throw new Error(`${field} exceeds its on-chain integer range`)
  return parsed
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function assertExactKeys(value: Record<string, unknown>, expected: readonly string[]): void {
  const keys = Object.keys(value).sort()
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    throw new Error(`unexpected descriptor field: ${keys.find((key) => !expected.includes(key)) ?? 'missing field'}`)
  }
}
