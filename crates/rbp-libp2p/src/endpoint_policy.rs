use libp2p_identity::PeerId;
use multiaddr::{Multiaddr, Protocol};
use rbp_core::DialContext;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Local address acceptance rules applied after signature verification.
#[derive(Clone, Debug, Default)]
pub struct EndpointPolicy {
    /// Permits private, loopback, link-local, and documentation addresses.
    /// This should only be enabled for tests or explicitly private overlays.
    pub allow_non_global: bool,
}

impl EndpointPolicy {
    /// Test/local-network policy that permits non-global IP addresses.
    pub const fn local_testing() -> Self {
        Self {
            allow_non_global: true,
        }
    }

    /// Returns whether a multiaddr is useful and safe in a dial context.
    pub fn accepts(
        &self,
        address: &Multiaddr,
        expected_peer: Option<PeerId>,
        context: DialContext,
    ) -> bool {
        if address.is_empty() {
            return false;
        }

        let mut has_transport = false;
        let mut browser_secure = false;
        for protocol in address {
            match protocol {
                Protocol::Ip4(ip) => {
                    if !self.accept_ip(IpAddr::V4(ip)) {
                        return false;
                    }
                }
                Protocol::Ip6(ip) => {
                    if !self.accept_ip(IpAddr::V6(ip)) {
                        return false;
                    }
                }
                Protocol::P2p(peer) => {
                    if expected_peer.is_some_and(|expected| expected != peer) {
                        return false;
                    }
                }
                Protocol::Tcp(_) | Protocol::Udp(_) | Protocol::QuicV1 | Protocol::WebTransport => {
                    has_transport = true;
                }
                Protocol::Wss(_) | Protocol::Https => {
                    has_transport = true;
                    browser_secure = true;
                }
                Protocol::Tls | Protocol::Certhash(_) => {
                    browser_secure = true;
                }
                _ => {}
            }
            if matches!(protocol, Protocol::WebTransport) {
                browser_secure = true;
            }
        }

        match context {
            DialContext::NativeServer | DialContext::Mobile => has_transport,
            DialContext::Browser | DialContext::RestrictedEgress => has_transport && browser_secure,
        }
    }

    fn accept_ip(&self, ip: IpAddr) -> bool {
        if self.allow_non_global {
            return !ip.is_unspecified() && !ip.is_multicast();
        }
        match ip {
            IpAddr::V4(ip) => is_global_ipv4(ip),
            IpAddr::V6(ip) => is_global_ipv6(ip),
        }
    }
}

fn is_global_ipv4(ip: Ipv4Addr) -> bool {
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.octets()[0] == 0
        || ip.octets()[0] >= 240
        || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        || (ip.octets()[0] == 198 && (ip.octets()[1] == 18 || ip.octets()[1] == 19)))
}

fn is_global_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn rejects_non_global_addresses_by_default() {
        let policy = EndpointPolicy::default();
        let local = Multiaddr::from_str("/ip4/127.0.0.1/tcp/4001").unwrap();
        let private = Multiaddr::from_str("/ip4/10.0.0.1/tcp/4001").unwrap();
        let unspecified = Multiaddr::from_str("/ip4/0.0.0.0/tcp/4001").unwrap();
        assert!(!policy.accepts(&local, None, DialContext::NativeServer));
        assert!(!policy.accepts(&private, None, DialContext::NativeServer));
        assert!(!policy.accepts(&unspecified, None, DialContext::NativeServer));
    }

    #[test]
    fn local_test_policy_still_rejects_unspecified() {
        let policy = EndpointPolicy::local_testing();
        let local = Multiaddr::from_str("/ip4/127.0.0.1/tcp/4001").unwrap();
        let unspecified = Multiaddr::from_str("/ip4/0.0.0.0/tcp/4001").unwrap();
        assert!(policy.accepts(&local, None, DialContext::NativeServer));
        assert!(!policy.accepts(&unspecified, None, DialContext::NativeServer));
    }

    #[test]
    fn browser_context_requires_secure_browser_transport() {
        let policy = EndpointPolicy::default();
        let tcp = Multiaddr::from_str("/ip4/8.8.8.8/tcp/4001").unwrap();
        let wss = Multiaddr::from_str("/dns4/example.com/tcp/443/wss").unwrap();
        assert!(!policy.accepts(&tcp, None, DialContext::Browser));
        assert!(policy.accepts(&wss, None, DialContext::Browser));
    }
}
