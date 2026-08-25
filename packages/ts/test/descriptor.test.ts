import { describe, expect, it } from 'vitest'
import { deriveNamespace, parseDescriptor } from '../src/index.js'

describe('network descriptor', () => {
  it('derives the shared namespace algorithm', () => {
    expect(deriveNamespace('example-network', 1)).toBe(
      '0xf90b28e5c2deb8854a5a0cda7584edcca25b73bc5a45f456aaa33c1de303646e'
    )
  })

  it('normalizes and rejects ambiguous descriptors', () => {
    const descriptor = parseDescriptor({
      rbpVersion: 1,
      registry: {
        chainId: '31337',
        address: '0x1111111111111111111111111111111111111111',
        deploymentBlock: 42,
        maxTtlSeconds: 7_776_000
      },
      namespace: deriveNamespace('browser-test', 1),
      acceptedRecordTypes: [2, 1]
    })
    expect(descriptor.registry.chainId).toBe(31337n)
    expect(descriptor.acceptedRecordTypes).toEqual([1, 2])
    expect(() => parseDescriptor({ ...descriptor, rpcUrl: 'https://central.invalid' })).toThrow(
      /unexpected descriptor field/
    )
  })

  it('enforces the registry integer widths', () => {
    const base = {
      rbpVersion: 1,
      registry: {
        chainId: 1,
        address: '0x1111111111111111111111111111111111111111',
        deploymentBlock: 1,
        maxTtlSeconds: 7_776_000
      },
      namespace: deriveNamespace('browser-test', 1),
      acceptedRecordTypes: [1]
    }

    expect(() => parseDescriptor({
      ...base,
      registry: { ...base.registry, chainId: (1n << 256n).toString() }
    })).toThrow(/chainId exceeds/)
    expect(() => parseDescriptor({
      ...base,
      registry: { ...base.registry, deploymentBlock: (1n << 64n).toString() }
    })).toThrow(/deploymentBlock exceeds/)
  })
})
