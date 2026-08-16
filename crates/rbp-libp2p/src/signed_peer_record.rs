use crate::EndpointPolicy;
use async_trait::async_trait;
use libp2p_identity::{Keypair, PeerId, PublicKey};
use multiaddr::Multiaddr;
use prost::Message;
use rbp_core::{
    DEFAULT_MAX_ENDPOINTS_PER_RECORD, DialContext, DiscoverySourceKind, Endpoint, PeerCandidate,
    PeerRecordCodec, PeerRecordError, RECORD_TYPE_LIBP2P,
};
use thiserror::Error;

/// Domain used by current libp2p `PeerRecord` signed envelopes.
pub const LIBP2P_PEER_RECORD_DOMAIN: &str = "libp2p-peer-record";
/// Multicodec `libp2p-peer-record` (`0x0103`) encoded as unsigned varint.
pub const LIBP2P_PEER_RECORD_PAYLOAD_TYPE: &[u8] = &[0x03, 0x01];
const MAX_ENVELOPE_BYTES: usize = 4096;

#[derive(Clone, PartialEq, Message)]
struct Envelope {
    #[prost(bytes = "vec", tag = "1")]
    public_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    payload_type: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    payload: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    signature: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct PeerRecordMessage {
    #[prost(bytes = "vec", tag = "1")]
    peer_id: Vec<u8>,
    #[prost(uint64, tag = "2")]
    sequence: u64,
    #[prost(message, repeated, tag = "3")]
    addresses: Vec<AddressInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct AddressInfo {
    #[prost(bytes = "vec", tag = "1")]
    multiaddr: Vec<u8>,
}

/// Codec for standard libp2p signed peer/address records.
#[derive(Clone, Debug)]
pub struct Libp2pPeerRecordCodec {
    policy: EndpointPolicy,
    max_endpoints: usize,
}

impl Default for Libp2pPeerRecordCodec {
    fn default() -> Self {
        Self {
            policy: EndpointPolicy::default(),
            max_endpoints: DEFAULT_MAX_ENDPOINTS_PER_RECORD,
        }
    }
}

impl Libp2pPeerRecordCodec {
    /// Creates a codec with explicit endpoint and count limits.
    #[must_use]
    pub const fn new(policy: EndpointPolicy, max_endpoints: usize) -> Self {
        Self {
            policy,
            max_endpoints,
        }
    }
}

#[async_trait]
impl PeerRecordCodec for Libp2pPeerRecordCodec {
    fn record_type(&self) -> u32 {
        RECORD_TYPE_LIBP2P
    }

    async fn decode_and_verify(
        &self,
        raw: &[u8],
        dial_context: DialContext,
    ) -> Result<PeerCandidate, PeerRecordError> {
        if raw.is_empty() || raw.len() > MAX_ENVELOPE_BYTES {
            return Err(PeerRecordError::InvalidSize(raw.len()));
        }
        let envelope =
            Envelope::decode(raw).map_err(|error| PeerRecordError::Malformed(error.to_string()))?;
        if envelope.payload_type != LIBP2P_PEER_RECORD_PAYLOAD_TYPE {
            return Err(PeerRecordError::Malformed(
                "unexpected signed-envelope payload type".to_owned(),
            ));
        }

        let public_key = PublicKey::try_decode_protobuf(&envelope.public_key)
            .map_err(|error| PeerRecordError::Malformed(error.to_string()))?;
        let signing_input = signing_input(&envelope.payload_type, &envelope.payload);
        if !public_key.verify(&signing_input, &envelope.signature) {
            return Err(PeerRecordError::InvalidSignature);
        }

        let payload = PeerRecordMessage::decode(envelope.payload.as_slice())
            .map_err(|error| PeerRecordError::Malformed(error.to_string()))?;
        let derived_peer = PeerId::from_public_key(&public_key);
        let payload_peer = PeerId::from_bytes(&payload.peer_id)
            .map_err(|error| PeerRecordError::Malformed(error.to_string()))?;
        if derived_peer != payload_peer {
            return Err(PeerRecordError::IdentityMismatch);
        }
        if payload.addresses.len() > self.max_endpoints {
            return Err(PeerRecordError::TooManyEndpoints(self.max_endpoints));
        }

        let endpoints = payload
            .addresses
            .into_iter()
            .filter_map(|info| Multiaddr::try_from(info.multiaddr).ok())
            .filter(|address| {
                self.policy
                    .accepts(address, Some(derived_peer), dial_context)
            })
            .map(|address| Endpoint {
                address: address.to_string(),
            })
            .collect::<Vec<_>>();
        if endpoints.is_empty() {
            return Err(PeerRecordError::NoUsableEndpoint(dial_context));
        }

        Ok(PeerCandidate {
            record_type: RECORD_TYPE_LIBP2P,
            peer_id: derived_peer.to_bytes(),
            sequence: payload.sequence,
            endpoints,
            raw_signed_record: raw.to_vec(),
            expires_at: 0,
            source: DiscoverySourceKind::RbpRegistry,
            announcement_block: None,
            announcement_log_index: None,
        })
    }
}

/// Creates an interoperable libp2p signed `PeerRecord` envelope.
///
/// # Errors
///
/// Returns an error if no address is supplied, an address cannot be encoded,
/// the endpoint cap is exceeded, or the identity key cannot sign.
pub fn sign_peer_record(
    keypair: &Keypair,
    sequence: u64,
    addresses: &[Multiaddr],
) -> Result<Vec<u8>, SignedPeerRecordError> {
    if addresses.is_empty() {
        return Err(SignedPeerRecordError::NoEndpoints);
    }
    if addresses.len() > DEFAULT_MAX_ENDPOINTS_PER_RECORD {
        return Err(SignedPeerRecordError::TooManyEndpoints(
            DEFAULT_MAX_ENDPOINTS_PER_RECORD,
        ));
    }

    let peer_id = PeerId::from_public_key(&keypair.public());
    let payload = PeerRecordMessage {
        peer_id: peer_id.to_bytes(),
        sequence,
        addresses: addresses
            .iter()
            .map(|address| AddressInfo {
                multiaddr: address.to_vec(),
            })
            .collect(),
    }
    .encode_to_vec();
    seal_payload(keypair, payload)
}

fn seal_payload(keypair: &Keypair, payload: Vec<u8>) -> Result<Vec<u8>, SignedPeerRecordError> {
    let signing_input = signing_input(LIBP2P_PEER_RECORD_PAYLOAD_TYPE, &payload);
    let signature = keypair
        .sign(&signing_input)
        .map_err(|error| SignedPeerRecordError::Signing(error.to_string()))?;
    let envelope = Envelope {
        public_key: keypair.public().encode_protobuf(),
        payload_type: LIBP2P_PEER_RECORD_PAYLOAD_TYPE.to_vec(),
        payload,
        signature,
    };
    let encoded = envelope.encode_to_vec();
    if encoded.len() > MAX_ENVELOPE_BYTES {
        return Err(SignedPeerRecordError::EnvelopeTooLarge(encoded.len()));
    }
    Ok(encoded)
}

fn signing_input(payload_type: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(
        LIBP2P_PEER_RECORD_DOMAIN.len() + payload_type.len() + payload.len() + 30,
    );
    append_uvarint(&mut input, LIBP2P_PEER_RECORD_DOMAIN.len());
    input.extend_from_slice(LIBP2P_PEER_RECORD_DOMAIN.as_bytes());
    append_uvarint(&mut input, payload_type.len());
    input.extend_from_slice(payload_type);
    append_uvarint(&mut input, payload.len());
    input.extend_from_slice(payload);
    input
}

fn append_uvarint(buffer: &mut Vec<u8>, mut value: usize) {
    while value >= 0x80 {
        buffer.push((value.to_le_bytes()[0] & 0x7f) | 0x80);
        value >>= 7;
    }
    buffer.push(value.to_le_bytes()[0]);
}

/// Errors while producing a signed libp2p peer record.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SignedPeerRecordError {
    /// At least one endpoint is required.
    #[error("a signed peer record requires at least one endpoint")]
    NoEndpoints,
    /// Resource limit violation.
    #[error("signed peer record exceeds the endpoint cap of {0}")]
    TooManyEndpoints(usize),
    /// Keypair signing failure.
    #[error("failed to sign peer record: {0}")]
    Signing(String),
    /// Result cannot fit in an RBP v1 registry announcement.
    #[error("signed envelope is too large: {0} bytes")]
    EnvelopeTooLarge(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn key_and_address() -> (Keypair, Multiaddr) {
        (
            Keypair::generate_ed25519(),
            Multiaddr::from_str("/ip4/127.0.0.1/tcp/4001").unwrap(),
        )
    }

    #[tokio::test]
    async fn signs_and_verifies_peer_record() {
        let (key, address) = key_and_address();
        let signed = sign_peer_record(&key, 42, std::slice::from_ref(&address)).unwrap();
        let decoded = Libp2pPeerRecordCodec::new(EndpointPolicy::local_testing(), 16)
            .decode_and_verify(&signed, DialContext::NativeServer)
            .await
            .unwrap();
        assert_eq!(
            decoded.peer_id,
            PeerId::from_public_key(&key.public()).to_bytes()
        );
        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.endpoints[0].address, address.to_string());
    }

    #[tokio::test]
    async fn rejects_tampered_signature_and_malformed_envelope() {
        let (key, address) = key_and_address();
        let signed = sign_peer_record(&key, 42, &[address]).unwrap();
        let mut envelope = Envelope::decode(signed.as_slice()).unwrap();
        envelope.signature[0] ^= 1;
        assert_eq!(
            Libp2pPeerRecordCodec::new(EndpointPolicy::local_testing(), 16)
                .decode_and_verify(&envelope.encode_to_vec(), DialContext::NativeServer)
                .await,
            Err(PeerRecordError::InvalidSignature)
        );
        assert!(
            Libp2pPeerRecordCodec::default()
                .decode_and_verify(&[0xff, 0xff], DialContext::NativeServer)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_peer_id_key_mismatch() {
        let (key, address) = key_and_address();
        let other = Keypair::generate_ed25519();
        let payload = PeerRecordMessage {
            peer_id: PeerId::from_public_key(&other.public()).to_bytes(),
            sequence: 1,
            addresses: vec![AddressInfo {
                multiaddr: address.to_vec(),
            }],
        }
        .encode_to_vec();
        let signed = seal_payload(&key, payload).unwrap();
        assert_eq!(
            Libp2pPeerRecordCodec::new(EndpointPolicy::local_testing(), 16)
                .decode_and_verify(&signed, DialContext::NativeServer)
                .await,
            Err(PeerRecordError::IdentityMismatch)
        );
    }

    #[tokio::test]
    async fn browser_ignores_non_browser_endpoint() {
        let key = Keypair::generate_ed25519();
        let tcp = Multiaddr::from_str("/ip4/8.8.8.8/tcp/4001").unwrap();
        let signed = sign_peer_record(&key, 1, &[tcp]).unwrap();
        assert_eq!(
            Libp2pPeerRecordCodec::default()
                .decode_and_verify(&signed, DialContext::Browser)
                .await,
            Err(PeerRecordError::NoUsableEndpoint(DialContext::Browser))
        );

        let wss = Multiaddr::from_str("/dns4/example.com/tcp/443/wss").unwrap();
        let signed = sign_peer_record(&key, 2, std::slice::from_ref(&wss)).unwrap();
        let decoded = Libp2pPeerRecordCodec::default()
            .decode_and_verify(&signed, DialContext::Browser)
            .await
            .unwrap();
        assert_eq!(decoded.endpoints[0].address, wss.to_string());
    }

    #[test]
    fn signing_rejects_resource_limit_violations() {
        let key = Keypair::generate_ed25519();
        assert_eq!(
            sign_peer_record(&key, 1, &[]),
            Err(SignedPeerRecordError::NoEndpoints)
        );
        let address = Multiaddr::from_str("/ip4/127.0.0.1/tcp/4001").unwrap();
        assert_eq!(
            sign_peer_record(&key, 1, &vec![address; 17]),
            Err(SignedPeerRecordError::TooManyEndpoints(16))
        );
    }
}
