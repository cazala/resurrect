import { peerIdFromPublicKey } from '@libp2p/peer-id'
import { PeerRecord, RecordEnvelope } from '@libp2p/peer-record'
import { bytesToHex, hexToBytes } from 'viem'
import type { BrowserPeerCandidate } from './types.js'

const LIBP2P_RECORD_TYPE = 2 as const

export interface DecodePeerRecordOptions {
  maxEndpoints?: number | undefined
  allowPrivateEndpoints?: boolean | undefined
}

export async function decodeBrowserPeerRecord(
  rawRecord: `0x${string}`,
  validUntil: bigint,
  blockNumber: bigint,
  logIndex: bigint,
  options: DecodePeerRecordOptions = {}
): Promise<BrowserPeerCandidate> {
  const bytes = hexToBytes(rawRecord)
  if (bytes.length === 0 || bytes.length > 4096) throw new Error('invalid signed record size')
  const envelope = await RecordEnvelope.openAndCertify(bytes, PeerRecord.DOMAIN)
  if (!equalBytes(envelope.payloadType, PeerRecord.CODEC)) throw new Error('unexpected envelope payload type')
  const record = PeerRecord.createFromProtobuf(envelope.payload)
  const signer = peerIdFromPublicKey(envelope.publicKey)
  if (!record.peerId.equals(signer)) throw new Error('signed peer ID does not match signing key')
  const maximum = positiveInteger(options.maxEndpoints ?? 16, 'maxEndpoints')
  if (record.multiaddrs.length > maximum) throw new Error('signed record exceeds endpoint cap')
  const endpoints = record.multiaddrs
    .filter((address) => browserEndpointAccepted(address, record.peerId.toString(), options.allowPrivateEndpoints ?? false))
    .map((address) => address.toString())
  if (endpoints.length === 0) throw new Error('signed record has no browser-dialable endpoint')
  return {
    recordType: LIBP2P_RECORD_TYPE,
    peerId: record.peerId.toString(),
    sequence: record.seqNumber,
    endpoints,
    rawSignedRecord: bytesToHex(bytes),
    validUntil,
    blockNumber,
    logIndex
  }
}

function positiveInteger(value: number, field: string): number {
  if (!Number.isSafeInteger(value) || value < 1) throw new Error(`${field} must be a positive integer`)
  return value
}

function browserEndpointAccepted(
  address: { getComponents(): Array<{ name: string; value?: string }> },
  expectedPeerId: string,
  allowPrivate: boolean
): boolean {
  const components = address.getComponents()
  const names = new Set(components.map((component) => component.name))
  const secure = names.has('webtransport') || names.has('wss') || names.has('https') || (names.has('tls') && names.has('ws'))
  if (!secure) return false
  const peer = components.find((component) => component.name === 'p2p')?.value
  if (peer != null && peer !== expectedPeerId) return false
  if (allowPrivate) return true
  for (const component of components) {
    if (component.name === 'ip4' && component.value != null && !isGlobalIpv4(component.value)) return false
    if (component.name === 'ip6' && component.value != null && !isGlobalIpv6(component.value)) return false
  }
  return true
}

function isGlobalIpv4(value: string): boolean {
  const octets = value.split('.').map(Number)
  if (octets.length !== 4 || octets.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return false
  const [first = 0, second = 0] = octets
  return !(
    first === 0 || first === 10 || first === 127 || first >= 224 ||
    (first === 100 && second >= 64 && second <= 127) ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 168) ||
    (first === 192 && second === 0) ||
    (first === 198 && (second === 18 || second === 19)) ||
    (first === 198 && second === 51) ||
    (first === 203 && second === 0)
  )
}

function isGlobalIpv6(value: string): boolean {
  const lower = value.toLowerCase()
  return !(
    lower.startsWith('::') || lower.startsWith('fc') || lower.startsWith('fd') ||
    lower.startsWith('fe8') || lower.startsWith('fe9') || lower.startsWith('fea') ||
    lower.startsWith('feb') || lower.startsWith('ff') || lower.startsWith('2001:db8')
  )
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}
