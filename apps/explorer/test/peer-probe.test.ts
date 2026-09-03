import { describe, expect, it } from 'vitest'
import { peerQualifiedAddress } from '../src/peer-probe.js'

const peer = '12D3KooWRFAprLu4b2RQzq9PWJ2sTYSYuCYA9yDJNEF5kFPYh7B6'

describe('browser peer probe', () => {
  it('binds a signed WSS endpoint to the expected peer identity', () => {
    expect(peerQualifiedAddress('/dns4/resurrect-ws.caza.la/tcp/443/wss', peer).toString()).toBe(
      `/dns4/resurrect-ws.caza.la/tcp/443/wss/p2p/${peer}`
    )
  })

  it('keeps a matching embedded peer identity', () => {
    const address = `/dns4/resurrect-ws.caza.la/tcp/443/wss/p2p/${peer}`
    expect(peerQualifiedAddress(address, peer).toString()).toBe(address)
  })

  it('rejects an endpoint bound to another peer', () => {
    const other = '12D3KooWMiWjo9cCWJG3KqYcTkKWWT7mKoBNGL2pZgbhiaVdtZtP'
    expect(() => peerQualifiedAddress(`/dns4/resurrect-ws.caza.la/tcp/443/wss/p2p/${other}`, peer)).toThrow(/does not match/)
  })
})
