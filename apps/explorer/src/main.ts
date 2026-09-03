import {
  ResurrectBrowserClient,
  injectedProvider,
  jsonRpcProvider,
  type BrowserPeerCandidate,
  type Eip1193Provider,
  type ScanReport
} from '@resurrect-protocol/client'
import {
  DEFAULT_NAMESPACE,
  DEFAULT_RPC_URL,
  errorMessage,
  formatChainTime,
  networkDescriptor,
  normalizeRpcUrl,
  shortValue,
  summarizeScan
} from './model.js'
import { probePeer, type PeerProbeSession } from './peer-probe.js'
import './styles.css'

declare global {
  interface Window {
    ethereum?: Eip1193Provider
  }
}

type ProviderMode = 'rpc' | 'wallet'
const descriptor = networkDescriptor()
const app = document.querySelector<HTMLDivElement>('#app')
if (app == null) throw new Error('Explorer mount point is missing')

app.innerHTML = `
  <header class="site-header">
    <a class="wordmark" href="./" aria-label="Resurrect Explorer home"><span class="wordmark-mark">R</span><span>RESURRECT</span></a>
    <div class="network-chip"><span></span>Ethereum mainnet</div>
  </header>
  <main>
    <section class="hero">
      <p class="eyebrow">Network recovery console</p>
      <h1>Find the network.<br />Prove it is alive.</h1>
      <p class="hero-copy">Discover signed peer records through Ethereum, then open a Noise-authenticated libp2p connection and measure a real ping from this browser.</p>
    </section>

    <section class="workspace" aria-label="Discovery controls">
      <form class="connect-card" id="provider-form">
        <div class="section-heading">
          <div><p class="step">01 / Provider</p><h2>Choose how to read Ethereum</h2></div>
          <div class="status-badge idle" id="connection-status" aria-live="polite"><span></span><span id="connection-label">Not scanned</span></div>
        </div>
        <div class="mode-switch" role="radiogroup" aria-label="Ethereum provider">
          <button class="mode active" type="button" role="radio" aria-checked="true" data-mode="rpc"><span class="mode-icon">↗</span><span><strong>Public RPC</strong><small>No wallet needed</small></span></button>
          <button class="mode" type="button" role="radio" aria-checked="false" data-mode="wallet"><span class="mode-icon">◇</span><span><strong>Browser wallet</strong><small>Read-only provider</small></span></button>
        </div>
        <label class="field" id="rpc-field"><span>Ethereum JSON-RPC URL</span><div class="input-wrap"><input id="rpc-url" name="rpc-url" type="url" value="${DEFAULT_RPC_URL}" spellcheck="false" autocomplete="off" /><button id="reset-rpc" type="button">Reset</button></div></label>
        <div class="wallet-note" id="wallet-note" hidden><span>◇</span><p><strong>No account exposure.</strong> The explorer uses only read methods and never calls <code>eth_requestAccounts</code>.</p></div>
        <button class="scan-button" id="scan-button" type="submit"><span id="scan-button-label">Connect &amp; discover</span><span>→</span></button>
        <p class="form-error" id="form-error" role="alert" hidden></p>
      </form>

      <aside class="network-card">
        <p class="step">02 / Target</p><h2>Canonical registry</h2>
        <dl class="network-details">
          <div><dt>Chain</dt><dd>Ethereum · 1</dd></div>
          <div><dt>Contract</dt><dd><a href="https://etherscan.io/address/${descriptor.registry.address}#code" target="_blank" rel="noreferrer">${shortValue(descriptor.registry.address)}</a></dd></div>
          <div><dt>From block</dt><dd>${descriptor.registry.deploymentBlock.toLocaleString()}</dd></div>
          <div><dt>Namespace</dt><dd title="${DEFAULT_NAMESPACE}">${shortValue(DEFAULT_NAMESPACE)}</dd></div>
        </dl>
        <div class="verified-note"><span>✓</span><p><strong>Verified before use</strong><br />Chain ID, registry constants, signed envelope and peer identity must all agree.</p></div>
      </aside>
    </section>

    <section class="results" aria-live="polite">
      <div class="results-heading"><div><p class="step">03 / Discovery</p><h2>Network view</h2></div><p id="scan-context">Run a scan to inspect recent announcements.</p></div>
      <div class="metrics">
        <article><span>Announcements processed</span><strong id="metric-announcements">—</strong><small>Matching registry events</small></article>
        <article><span>Browser-ready peers</span><strong id="metric-peers">—</strong><small>Verified unique candidates</small></article>
        <article><span>Filtered records</span><strong id="metric-filtered">—</strong><small>Invalid, expired or native-only</small></article>
        <article><span>Confirmed head</span><strong id="metric-head">—</strong><small id="metric-window">No blocks scanned</small></article>
      </div>
      <div class="peer-list" id="peer-list"><div class="empty-state"><span class="radar"><i></i></span><h3>Ready when you are</h3><p>Signed browser endpoints will appear here with an authenticated ping control.</p></div></div>
    </section>

    <section class="limits">
      <div><p class="step">Read the result correctly</p><h2>Discovery is not liveness.</h2></div>
      <div class="limit-grid">
        <article><span>01</span><h3>Ethereum tells us</h3><p>Which signed records were announced, their sequence and expiry, and where a peer says it can be reached.</p></article>
        <article><span>02</span><h3>The probe proves</h3><p>The endpoint is reachable now, controls the announced libp2p identity, speaks the expected protocols, and answers ping.</p></article>
        <article><span>03</span><h3>No global peer count</h3><p>Resurrect has no authoritative membership list. The UI reports announced candidates and this browser's live connections separately.</p></article>
      </div>
    </section>
  </main>
  <footer><span>Resurrect v1 · discovery, not trust</span><a href="https://github.com/cazala/resurrect" target="_blank" rel="noreferrer">Source ↗</a></footer>
`

const form = requiredElement<HTMLFormElement>('provider-form')
const rpcInput = requiredElement<HTMLInputElement>('rpc-url')
const rpcField = requiredElement<HTMLElement>('rpc-field')
const walletNote = requiredElement<HTMLElement>('wallet-note')
const scanButton = requiredElement<HTMLButtonElement>('scan-button')
const scanButtonLabel = requiredElement<HTMLElement>('scan-button-label')
const formError = requiredElement<HTMLElement>('form-error')
let providerMode: ProviderMode = 'rpc'
let scanNumber = 0
let activeProbe: PeerProbeSession | undefined

for (const button of document.querySelectorAll<HTMLButtonElement>('[data-mode]')) {
  button.addEventListener('click', () => selectMode(button.dataset.mode === 'wallet' ? 'wallet' : 'rpc'))
}
requiredElement<HTMLButtonElement>('reset-rpc').addEventListener('click', () => { rpcInput.value = DEFAULT_RPC_URL; rpcInput.focus() })
form.addEventListener('submit', (event) => { event.preventDefault(); void scan() })
window.addEventListener('pagehide', () => { void activeProbe?.close() })

function selectMode(mode: ProviderMode): void {
  providerMode = mode
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-mode]')) {
    const active = button.dataset.mode === mode
    button.classList.toggle('active', active)
    button.setAttribute('aria-checked', String(active))
  }
  rpcField.hidden = mode !== 'rpc'
  walletNote.hidden = mode !== 'wallet'
  scanButtonLabel.textContent = mode === 'rpc' ? 'Connect & discover' : 'Use wallet provider & discover'
  clearError()
}

async function scan(): Promise<void> {
  const currentScan = ++scanNumber
  clearError(); setBusy(true); setStatus('loading', 'Reading Ethereum')
  requiredElement<HTMLElement>('scan-context').textContent = 'Verifying the provider and scanning the bounded announcement window…'
  try {
    await activeProbe?.close(); activeProbe = undefined
    const provider = providerMode === 'rpc' ? jsonRpcProvider(normalizeRpcUrl(rpcInput.value)) : injectedProvider(requireInjectedProvider())
    const report = await new ResurrectBrowserClient(descriptor, provider).scan()
    if (currentScan !== scanNumber) return
    renderReport(report); setStatus('connected', 'Discovery complete')
  } catch (error) {
    if (currentScan !== scanNumber) return
    formError.textContent = friendlyError(errorMessage(error)); formError.hidden = false
    setStatus('error', 'Discovery failed')
    requiredElement<HTMLElement>('scan-context').textContent = 'Change the provider and try again.'
  } finally {
    if (currentScan === scanNumber) setBusy(false)
  }
}

function renderReport(report: ScanReport): void {
  const summary = summarizeScan(report)
  requiredElement<HTMLElement>('metric-announcements').textContent = summary.announcements
  requiredElement<HTMLElement>('metric-peers').textContent = summary.browserPeers
  requiredElement<HTMLElement>('metric-filtered').textContent = summary.filteredRecords
  requiredElement<HTMLElement>('metric-head').textContent = summary.confirmedHead
  requiredElement<HTMLElement>('metric-window').textContent = `${summary.scannedBlocks} blocks scanned`
  requiredElement<HTMLElement>('scan-context').textContent = `Chain time ${formatChainTime(report.headTimestamp)}`
  const list = requiredElement<HTMLElement>('peer-list'); list.replaceChildren()
  if (report.candidates.length === 0) {
    const empty = document.createElement('div'); empty.className = 'empty-state result-empty'
    empty.innerHTML = '<span class="radar muted"><i></i></span><h3>No browser-ready peers</h3>'
    const text = document.createElement('p')
    text.textContent = report.recordsRejected > 0
      ? `${report.recordsRejected.toLocaleString()} record${report.recordsRejected === 1 ? ' was' : 's were'} filtered because ${report.recordsRejected === 1 ? 'it was' : 'they were'} invalid, expired, or had no secure browser transport.`
      : 'No unexpired browser-compatible records were found.'
    empty.append(text); list.append(empty); return
  }
  report.candidates.forEach((candidate, index) => list.append(peerCard(candidate, index)))
}

function peerCard(candidate: BrowserPeerCandidate, index: number): HTMLElement {
  const article = document.createElement('article'); article.className = 'peer-card'
  const top = document.createElement('div'); top.className = 'peer-heading'
  const identity = document.createElement('div'); identity.innerHTML = `<span>Peer ${String(index + 1).padStart(2, '0')}</span>`
  const peerId = document.createElement('h3'); peerId.textContent = candidate.peerId; peerId.title = candidate.peerId; identity.append(peerId)
  const live = document.createElement('div'); live.className = 'reachable-badge'; live.textContent = 'Announced'; top.append(identity, live)
  const details = document.createElement('dl')
  const facts: Array<[string, string]> = [
    ['Sequence', candidate.sequence.toLocaleString()],
    ['Valid until', formatChainTime(candidate.validUntil)],
    ['Block', candidate.blockNumber.toLocaleString()]
  ]
  for (const [term, value] of facts) {
    const row = document.createElement('div'); const dt = document.createElement('dt'); const dd = document.createElement('dd')
    dt.textContent = term; dd.textContent = value; row.append(dt, dd); details.append(row)
  }
  const endpoints = document.createElement('div'); endpoints.className = 'endpoints'; endpoints.innerHTML = '<span>Signed browser endpoints</span>'
  candidate.endpoints.forEach((endpoint) => { const code = document.createElement('code'); code.textContent = endpoint; endpoints.append(code) })
  const probeArea = document.createElement('div'); probeArea.className = 'probe-area'
  const button = document.createElement('button'); button.className = 'probe-button'; button.type = 'button'; button.textContent = 'Authenticate & ping →'
  const result = document.createElement('div'); result.className = 'probe-result'; result.hidden = true
  button.addEventListener('click', () => { void runProbe(candidate, button, result, live) })
  probeArea.append(button, result); article.append(top, details, endpoints, probeArea); return article
}

async function runProbe(candidate: BrowserPeerCandidate, button: HTMLButtonElement, result: HTMLElement, badge: HTMLElement): Promise<void> {
  button.disabled = true; button.textContent = 'Opening authenticated WSS…'; result.hidden = true; badge.textContent = 'Connecting'; badge.className = 'reachable-badge probing'
  try {
    await activeProbe?.close(); activeProbe = await probePeer(candidate)
    const probe = activeProbe.result; badge.textContent = 'Live'; badge.className = 'reachable-badge live'
    result.replaceChildren()
    const values: Array<[string, string]> = [['Noise peer', probe.peerId], ['Ping RTT', `${probe.pingMs.toFixed(1)} ms`], ['Connect', `${probe.connectionMs.toFixed(1)} ms`], ['Agent', probe.agentVersion ?? 'Not reported'], ['Protocol', probe.protocolVersion ?? 'Not reported']]
    for (const [label, value] of values) { const item = document.createElement('div'); const span = document.createElement('span'); const strong = document.createElement('strong'); span.textContent = label; strong.textContent = value; item.append(span, strong); result.append(item) }
    const protocols = document.createElement('p'); protocols.textContent = `Protocols: ${probe.protocols.join(', ') || 'none reported'}`; result.append(protocols); result.hidden = false
    button.textContent = 'Ping again →'; button.disabled = false
  } catch (error) {
    badge.textContent = 'Unreachable'; badge.className = 'reachable-badge failed'; result.textContent = errorMessage(error); result.hidden = false; button.textContent = 'Retry authenticated ping →'; button.disabled = false
  }
}

function requireInjectedProvider(): Eip1193Provider {
  if (window.ethereum == null) throw new Error('No injected Ethereum provider was found. Enable a browser wallet or use Public RPC.')
  return window.ethereum
}
function setBusy(busy: boolean): void { scanButton.disabled = busy; scanButton.classList.toggle('busy', busy); scanButtonLabel.textContent = busy ? 'Scanning recent blocks…' : providerMode === 'rpc' ? 'Connect & discover' : 'Use wallet provider & discover' }
function setStatus(state: 'loading' | 'connected' | 'error', label: string): void { const status = requiredElement<HTMLElement>('connection-status'); status.className = `status-badge ${state}`; requiredElement<HTMLElement>('connection-label').textContent = label }
function clearError(): void { formError.hidden = true; formError.textContent = '' }
function friendlyError(message: string): string { if (/block range|eth_getlogs|range limit/i.test(message)) return `${message} Try another RPC or a plan that permits wider eth_getLogs ranges.`; if (/chain id|does not match/i.test(message)) return `${message} Select Ethereum mainnet or use the default RPC.`; return message }
function requiredElement<T extends HTMLElement>(id: string): T { const element = document.getElementById(id); if (element == null) throw new Error(`Missing #${id}`); return element as T }
