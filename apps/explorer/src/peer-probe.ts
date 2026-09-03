import { noise } from '@chainsafe/libp2p-noise'
import { yamux } from '@chainsafe/libp2p-yamux'
import { identify } from '@libp2p/identify'
import { ping } from '@libp2p/ping'
import { webSockets } from '@libp2p/websockets'
import { multiaddr, type Multiaddr } from '@multiformats/multiaddr'
import { createLibp2p } from 'libp2p'
import type { BrowserPeerCandidate } from '@resurrect-protocol/client'

export interface PeerProbeResult {
  peerId: string
  endpoint: string
  connectionMs: number
  pingMs: number
  agentVersion?: string
  protocolVersion?: string
  protocols: readonly string[]
}

export interface PeerProbeSession {
  result: PeerProbeResult
  close(): Promise<void>
}

export function peerQualifiedAddress(endpoint: string, expectedPeerId: string): Multiaddr {
  const address = multiaddr(endpoint)
  const embeddedPeer = address.getComponents().find((component) => component.name === 'p2p')?.value
  if (embeddedPeer != null && embeddedPeer !== expectedPeerId) {
    throw new Error('signed endpoint peer ID does not match the signed record identity')
  }
  return embeddedPeer == null ? address.encapsulate(`/p2p/${expectedPeerId}`) : address
}

export async function probePeer(
  candidate: BrowserPeerCandidate,
  timeoutMilliseconds = 15_000
): Promise<PeerProbeSession> {
  const endpoint = candidate.endpoints[0]
  if (endpoint == null) throw new Error('candidate has no browser endpoint')
  const target = peerQualifiedAddress(endpoint, candidate.peerId)
  const node = await createLibp2p({
    start: false,
    nodeInfo: { name: 'resurrect-explorer', version: '1' },
    transports: [webSockets()],
    connectionEncrypters: [noise()],
    streamMuxers: [yamux()],
    services: {
      identify: identify({ runOnConnectionOpen: false, timeout: timeoutMilliseconds }),
      ping: ping({ timeout: timeoutMilliseconds })
    }
  })

  try {
    await node.start()
    const startedAt = performance.now()
    const connection = await node.dial(target, { signal: AbortSignal.timeout(timeoutMilliseconds) })
    const connectionMs = performance.now() - startedAt
    if (connection.remotePeer.toString() !== candidate.peerId) {
      throw new Error('connected peer identity does not match the signed registry record')
    }
    const [pingMs, identified] = await Promise.all([
      node.services.ping.ping(connection.remotePeer, { signal: AbortSignal.timeout(timeoutMilliseconds) }),
      node.services.identify.identify(connection, { signal: AbortSignal.timeout(timeoutMilliseconds) })
    ])
    if (identified.peerId.toString() !== candidate.peerId) {
      throw new Error('identify response does not match the signed registry record')
    }
    return {
      result: {
        peerId: candidate.peerId,
        endpoint: target.toString(),
        connectionMs,
        pingMs,
        ...(identified.agentVersion == null ? {} : { agentVersion: identified.agentVersion }),
        ...(identified.protocolVersion == null ? {} : { protocolVersion: identified.protocolVersion }),
        protocols: [...identified.protocols].sort()
      },
      close: async () => node.stop()
    }
  } catch (error) {
    await node.stop()
    throw error
  }
}
