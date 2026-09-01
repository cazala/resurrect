export { ResurrectBrowserClient } from './client.js'
export {
  ETHEREUM_MAINNET_CHAIN_ID,
  ETHEREUM_MAINNET_REGISTRY,
  ETHEREUM_MAINNET_REGISTRY_ADDRESS,
  ETHEREUM_MAINNET_REGISTRY_DEPLOYMENT_BLOCK,
  MAX_TTL_SECONDS,
  RESURRECT_VERSION,
  deriveNamespace,
  ethereumMainnetDescriptor,
  parseDescriptor,
  parseDescriptorJson
} from './descriptor.js'
export { decodeBrowserPeerRecord } from './peer-record.js'
export { injectedProvider, jsonRpcProvider, persistJsonRpcUrl } from './provider.js'
export { scanRegistry, verifyProvider } from './scanner.js'
export type {
  BrowserPeerCandidate,
  Eip1193Provider,
  Eip1193RequestArguments,
  JsonNetworkDescriptor,
  JsonRegistryDescriptor,
  NetworkDescriptor,
  RegistryDescriptor,
  RegistryProvider,
  ScanOptions,
  ScanReport
} from './types.js'
