# RBP documentation

The normative protocol is [spec.md](spec.md). Other documents explain this repository's reference implementation and do not override the specification.

| Document | Audience | Contents |
|---|---|---|
| [Architecture](architecture.md) | reviewers and maintainers | trust boundaries, components, data flow, state |
| [Application integration](application-integration.md) | protocol developers | descriptor/profile decisions and Rust integration |
| [Node operations](node-operations.md) | seed operators | keys, endpoints, configuration, lifecycle, recovery |
| [Browser client](browser-client.md) | web developers | providers, privacy, scanning, dial-context rules |
| [Security](security.md) | security reviewers and operators | threats, mitigations, residual risk |
| [Testing](testing.md) | contributors and auditors | unit, fuzz, invariant, fork, interop, reboot suites |
| [Conformance](conformance.md) | implementers | spec checklist-to-evidence map |
| [Releasing](releasing.md) | maintainers | next/latest automation and required credentials |
| [Deployments](deployments.md) | application maintainers | registry deployment and descriptor pinning |

Start with the [project README](../README.md) for installation and quick-start commands.
