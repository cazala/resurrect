import { describe, expect, it } from 'vitest'
import {
  ETHEREUM_MAINNET_REGISTRY,
  deriveNamespace,
  ethereumMainnetDescriptor,
  parseDescriptor
} from '../src/index.js'

describe('network descriptor', () => {
  it('derives the shared namespace algorithm', () => {
    expect(deriveNamespace('example-network', 1)).toBe(
      '0x80b9e2baaf4a666ec32337c351ad59485b3a43eca09a5f372e2f84b981123c88'
    )
  })

  it('pins the published Ethereum mainnet registry without choosing a namespace or provider', () => {
    const namespace = deriveNamespace('canonical-deployment-test', 1)
    const descriptor = ethereumMainnetDescriptor(namespace)

    expect(ETHEREUM_MAINNET_REGISTRY).toEqual({
      chainId: 1n,
      address: '0x6F33c332e8251dcd307D85A27fCcAbd85d578910',
      deploymentBlock: 25_882_327n,
      maxTtlSeconds: 7_776_000
    })
    expect(descriptor.registry).toEqual(ETHEREUM_MAINNET_REGISTRY)
    expect(descriptor.namespace).toBe(namespace)
    expect(descriptor.acceptedRecordTypes).toEqual([2])
  })

  it('normalizes and rejects ambiguous descriptors', () => {
    const descriptor = parseDescriptor({
      resurrectVersion: 1,
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
      resurrectVersion: 1,
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
