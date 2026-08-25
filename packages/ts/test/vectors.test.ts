import { decodeEventLog, parseAbi, type Hex } from 'viem'
import { describe, expect, it } from 'vitest'
import descriptorVector from '../../../test-vectors/descriptors/rbp-v1.json' with { type: 'json' }
import eventVector from '../../../test-vectors/registry-events/peer-announced-v1.json' with { type: 'json' }
import { deriveNamespace, parseDescriptor } from '../src/index.js'

const registryEvent = parseAbi([
  'event PeerAnnounced(bytes32 indexed namespace, uint32 indexed recordType, uint64 validUntil, bytes peerRecord)'
])

describe('cross-language protocol vectors', () => {
  it('normalizes the same descriptor and namespace as Rust', () => {
    const descriptor = parseDescriptor(descriptorVector.descriptor)
    expect(deriveNamespace(descriptorVector.application, descriptorVector.majorVersion)).toBe(
      descriptorVector.derivedNamespace
    )
    expect(descriptor.namespace).toBe(descriptorVector.derivedNamespace)
    expect(descriptor.acceptedRecordTypes).toEqual([1, 2])
  })

  it('decodes the same PeerAnnounced ABI event as Alloy', () => {
    const decoded = decodeEventLog({
      abi: registryEvent,
      data: eventVector.data as Hex,
      topics: [
        eventVector.topic0 as Hex,
        eventVector.namespace as Hex,
        eventVector.recordTypeTopic as Hex
      ]
    })
    expect(decoded.eventName).toBe('PeerAnnounced')
    expect(decoded.args).toEqual({
      namespace: eventVector.namespace,
      recordType: eventVector.recordType,
      validUntil: BigInt(eventVector.validUntil),
      peerRecord: eventVector.peerRecord
    })
  })
})
