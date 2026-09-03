import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import type { BrowserPeerCandidate } from '@resurrect-protocol/client'
import { afterEach, describe, expect, it } from 'vitest'
import { probePeer } from '../src/peer-probe.js'

const nodeBinary = process.env.RESURRECT_NODE_BIN
const websocketPort = Number(process.env.RESURRECT_TEST_WS_PORT ?? '42008')
let child: ChildProcessWithoutNullStreams | undefined
let workDirectory: string | undefined

describe.skipIf(nodeBinary == null)('Rust node WebSocket interoperability', () => {
  afterEach(async () => {
    child?.kill('SIGTERM')
    child = undefined
    if (workDirectory != null) await rm(workDirectory, { recursive: true, force: true })
    workDirectory = undefined
  })

  it('authenticates, identifies, and pings the Rust peer from JavaScript', async () => {
    workDirectory = await mkdtemp(join(tmpdir(), 'resurrect-browser-interop-'))
    const statusFile = join(workDirectory, 'status.json')
    child = spawn(nodeBinary!, [
      '--application', 'browser-interop-test',
      '--major-version', '1',
      '--rpc-url', 'http://127.0.0.1:1',
      '--identity', join(workDirectory, 'identity.key'),
      '--cache', join(workDirectory, 'peers.sqlite3'),
      '--listen', `/ip4/127.0.0.1/tcp/${websocketPort}/ws`,
      '--mdns', 'false',
      '--minimum-peers', '1',
      '--native-observation-millis', '50',
      '--initial-backoff-millis', '50',
      '--maximum-backoff-seconds', '1',
      '--status-file', statusFile,
      '--log-format', 'json'
    ], { env: { ...process.env, RUST_LOG: 'warn' } })

    let logs = ''
    child.stdout.on('data', (chunk: Buffer) => { logs += chunk.toString() })
    child.stderr.on('data', (chunk: Buffer) => { logs += chunk.toString() })
    const status = await waitForStatus(statusFile, () => logs)
    const candidate: BrowserPeerCandidate = {
      recordType: 2,
      peerId: status.peerId,
      sequence: 1n,
      endpoints: [`/ip4/127.0.0.1/tcp/${websocketPort}/ws`],
      rawSignedRecord: '0x00',
      validUntil: BigInt(Math.floor(Date.now() / 1000) + 60),
      blockNumber: 1n,
      logIndex: 0n
    }

    const session = await probePeer(candidate, 10_000)
    try {
      expect(session.result.peerId).toBe(status.peerId)
      expect(session.result.endpoint.endsWith(`/p2p/${status.peerId}`)).toBe(true)
      expect(session.result.pingMs).toBeGreaterThanOrEqual(0)
      expect(session.result.protocolVersion).toBe('/resurrect/1.0.0')
      expect(session.result.protocols).toContain('/ipfs/ping/1.0.0')
    } finally {
      await session.close()
    }
  }, 20_000)
})

interface NodeStatus {
  peerId: string
}

async function waitForStatus(path: string, currentLogs: () => string): Promise<NodeStatus> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const value: unknown = JSON.parse(await readFile(path, 'utf8'))
      if (isNodeStatus(value)) return value
    } catch {
      // The node writes this file atomically after its first bootstrap cycle.
    }
    await delay(50)
  }
  throw new Error(`Rust node did not become ready. Logs: ${currentLogs()}`)
}

function isNodeStatus(value: unknown): value is NodeStatus {
  return typeof value === 'object' && value != null && 'peerId' in value && typeof value.peerId === 'string'
}
