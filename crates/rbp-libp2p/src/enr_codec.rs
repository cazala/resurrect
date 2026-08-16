use crate::EndpointPolicy;
use alloy_rlp::Decodable;
use async_trait::async_trait;
use enr::{Enr, k256::ecdsa::SigningKey};
use rbp_core::{
    DialContext, DiscoverySourceKind, Endpoint, PeerCandidate, PeerRecordCodec, PeerRecordError,
    RECORD_TYPE_ENR,
};
use std::net::IpAddr;

const MAX_ENR_BYTES: usize = 300;

/// EIP-778 ENR codec for raw RLP records.
#[derive(Clone, Debug, Default)]
pub struct EnrCodec {
    policy: EndpointPolicy,
}

impl EnrCodec {
    /// Creates an ENR codec with explicit endpoint policy.
    #[must_use]
    pub const fn new(policy: EndpointPolicy) -> Self {
        Self { policy }
    }
}

#[async_trait]
impl PeerRecordCodec for EnrCodec {
    fn record_type(&self) -> u32 {
        RECORD_TYPE_ENR
    }

    async fn decode_and_verify(
        &self,
        raw: &[u8],
        dial_context: DialContext,
    ) -> Result<PeerCandidate, PeerRecordError> {
        if raw.is_empty() || raw.len() > MAX_ENR_BYTES {
            return Err(PeerRecordError::InvalidSize(raw.len()));
        }
        if dial_context == DialContext::Browser {
            return Err(PeerRecordError::NoUsableEndpoint(dial_context));
        }

        let mut remaining = raw;
        let record = Enr::<SigningKey>::decode(&mut remaining).map_err(|error| {
            if error.to_string().contains("Signature") {
                PeerRecordError::InvalidSignature
            } else {
                PeerRecordError::Malformed(error.to_string())
            }
        })?;
        if !remaining.is_empty() {
            return Err(PeerRecordError::Malformed(
                "trailing bytes after ENR".to_owned(),
            ));
        }

        let endpoints = enr_endpoints(&record)
            .into_iter()
            .filter(|address| self.policy.accepts(address, None, dial_context))
            .map(|address| Endpoint {
                address: address.to_string(),
            })
            .collect::<Vec<_>>();
        if endpoints.is_empty() {
            return Err(PeerRecordError::NoUsableEndpoint(dial_context));
        }

        Ok(PeerCandidate {
            record_type: RECORD_TYPE_ENR,
            peer_id: record.node_id().raw().to_vec(),
            sequence: record.seq(),
            endpoints,
            raw_signed_record: raw.to_vec(),
            expires_at: 0,
            source: DiscoverySourceKind::RbpRegistry,
            announcement_block: None,
            announcement_log_index: None,
        })
    }
}

fn enr_endpoints(record: &Enr<SigningKey>) -> Vec<multiaddr::Multiaddr> {
    let mut endpoints = Vec::with_capacity(4);
    if let (Some(ip), Some(port)) = (record.ip4(), record.tcp4()) {
        push_endpoint(&mut endpoints, IpAddr::V4(ip), "tcp", port);
    }
    if let (Some(ip), Some(port)) = (record.ip6(), record.tcp6()) {
        push_endpoint(&mut endpoints, IpAddr::V6(ip), "tcp", port);
    }
    if let (Some(ip), Some(port)) = (record.ip4(), record.udp4()) {
        push_endpoint(&mut endpoints, IpAddr::V4(ip), "udp", port);
    }
    if let (Some(ip), Some(port)) = (record.ip6(), record.udp6()) {
        push_endpoint(&mut endpoints, IpAddr::V6(ip), "udp", port);
    }
    endpoints
}

fn push_endpoint(
    endpoints: &mut Vec<multiaddr::Multiaddr>,
    ip: IpAddr,
    transport: &str,
    port: u16,
) {
    let protocol = match ip {
        IpAddr::V4(_) => "ip4",
        IpAddr::V6(_) => "ip6",
    };
    if let Ok(address) = format!("/{protocol}/{ip}/{transport}/{port}").parse() {
        endpoints.push(address);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rlp::Encodable;
    use enr::Enr;
    use std::net::Ipv4Addr;

    fn signed_enr() -> Vec<u8> {
        let key = SigningKey::random(&mut enr::k256::elliptic_curve::rand_core::OsRng);
        let record = Enr::builder()
            .seq(42)
            .ip4(Ipv4Addr::new(8, 8, 8, 8))
            .tcp4(4001)
            .build(&key)
            .unwrap();
        let mut encoded = Vec::new();
        record.encode(&mut encoded);
        encoded
    }

    #[tokio::test]
    async fn accepts_valid_raw_enr() {
        let decoded = EnrCodec::default()
            .decode_and_verify(&signed_enr(), DialContext::NativeServer)
            .await
            .unwrap();
        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.endpoints[0].address, "/ip4/8.8.8.8/tcp/4001");
    }

    #[tokio::test]
    async fn rejects_invalid_signature_and_oversize() {
        let mut invalid = signed_enr();
        let last = invalid.len() - 1;
        invalid[last] ^= 1;
        assert!(
            EnrCodec::default()
                .decode_and_verify(&invalid, DialContext::NativeServer)
                .await
                .is_err()
        );
        assert_eq!(
            EnrCodec::default()
                .decode_and_verify(&vec![0; 301], DialContext::NativeServer)
                .await,
            Err(PeerRecordError::InvalidSize(301))
        );
    }

    #[tokio::test]
    async fn rejects_enr_for_browser_context() {
        assert_eq!(
            EnrCodec::default()
                .decode_and_verify(&signed_enr(), DialContext::Browser)
                .await,
            Err(PeerRecordError::NoUsableEndpoint(DialContext::Browser))
        );
    }
}
