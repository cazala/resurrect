export { RbpBrowserClient } from './client.js'
export {
  MAX_TTL_SECONDS,
  RBP_VERSION,
  deriveNamespace,
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
