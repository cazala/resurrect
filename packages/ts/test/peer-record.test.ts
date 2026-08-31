import { describe, expect, it } from 'vitest'
import { multiaddr } from '@multiformats/multiaddr'
import { PeerRecord, RecordEnvelope } from '@libp2p/peer-record'
import { peerIdFromPrivateKey } from '@libp2p/peer-id'
import { generateKeyPair } from '@libp2p/crypto/keys'
import { bytesToHex } from 'viem'
import vectors from '../../../test-vectors/peer-records/libp2p-ed25519.json' with { type: 'json' }
import { decodeBrowserPeerRecord } from '../src/index.js'

describe('browser signed peer records', () => {
  it('parses the same deterministic record as Rust', async () => {
    const candidate = await decodeBrowserPeerRecord(
      vectors.browser.recordHex as `0x${string}`,
      2000n,
      12n,
      3n
    )
    expect(candidate.peerId).toBe(vectors.peerId)
    expect(candidate.sequence).toBe(BigInt(vectors.browser.sequence))
    expect(candidate.endpoints).toEqual([vectors.browser.endpoint])
  })

  it('rejects valid signatures without browser transport endpoints', async () => {
    await expect(
      decodeBrowserPeerRecord(
        vectors.nativeOnly.recordHex as `0x${string}`,
        2000n,
        12n,
        3n
      )
    ).rejects.toThrow(/browser-dialable/)
  })

  it('rejects signature tampering', async () => {
    const tampered = `${vectors.browser.recordHex.slice(0, -2)}01` as `0x${string}`
    await expect(decodeBrowserPeerRecord(tampered, 2000n, 12n, 3n)).rejects.toThrow()
  })

  it('rejects IPv4-mapped loopback endpoints', async () => {
    const privateKey = await generateKeyPair('Ed25519')
    const peerId = peerIdFromPrivateKey(privateKey)
    const record = new PeerRecord({
      peerId,
      multiaddrs: [multiaddr(`/ip6/::ffff:127.0.0.1/tcp/443/wss/p2p/${peerId}`)],
      seqNumber: 1n
    })
    const envelope = await RecordEnvelope.seal(record, privateKey)
    await expect(
      decodeBrowserPeerRecord(bytesToHex(envelope.marshal()), 2000n, 12n, 3n)
    ).rejects.toThrow(/browser-dialable/)
  })
})
