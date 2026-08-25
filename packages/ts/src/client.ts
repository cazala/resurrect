import { scanRegistry, verifyProvider } from './scanner.js'
import type { NetworkDescriptor, RegistryProvider, ScanOptions, ScanReport } from './types.js'

export class RbpBrowserClient {
  readonly descriptor: NetworkDescriptor
  #provider: RegistryProvider

  constructor(descriptor: NetworkDescriptor, provider: RegistryProvider) {
    this.descriptor = descriptor
    this.#provider = provider
  }

  setProvider(provider: RegistryProvider): void {
    this.#provider = provider
  }

  async verifyProvider(): Promise<void> {
    await verifyProvider(this.#provider, this.descriptor)
  }

  async scan(options: ScanOptions = {}): Promise<ScanReport> {
    return scanRegistry(this.#provider, this.descriptor, options)
  }
}
