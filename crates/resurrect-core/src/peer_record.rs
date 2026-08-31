use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// EIP-778 ENR record type.
pub const RECORD_TYPE_ENR: u32 = 1;
/// libp2p signed peer/address record envelope type.
pub const RECORD_TYPE_LIBP2P: u32 = 2;

/// Environment-specific endpoint selection profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DialContext {
    /// Native daemon/server transports.
    #[default]
    NativeServer,
    /// Browser-compatible secure transports.
    Browser,
    /// Mobile runtime transports.
    Mobile,
    /// Environments with explicit egress limitations.
    RestrictedEgress,
}

/// A signed endpoint that passed codec syntax checks.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    /// Canonical endpoint representation (for example, a multiaddr).
    pub address: String,
}

/// Origin of a candidate, used for diversity and observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiscoverySourceKind {
    /// Validated persisted peer cache.
    LocalCache,
    /// Application-native discovery.
    Native,
    /// Resurrect registry log.
    ResurrectRegistry,
}

/// A cryptographically verified and policy-filtered peer candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerCandidate {
    /// Resurrect codec identifier.
    pub record_type: u32,
    /// Codec-defined canonical peer identity bytes.
    #[serde(with = "hex_bytes")]
    pub peer_id: Vec<u8>,
    /// Monotonically increasing signed-record sequence.
    pub sequence: u64,
    /// Usable, signed endpoints.
    pub endpoints: Vec<Endpoint>,
    /// Original signed bytes, retained for peer exchange.
    #[serde(with = "hex_bytes")]
    pub raw_signed_record: Vec<u8>,
    /// Registry-derived expiration timestamp.
    pub expires_at: u64,
    /// Discovery source.
    pub source: DiscoverySourceKind,
    /// Block number of the registry observation, when applicable.
    pub announcement_block: Option<u64>,
    /// Log index of the registry observation, when applicable.
    pub announcement_log_index: Option<u64>,
}

impl PeerCandidate {
    /// A stable composite identity key that avoids cross-codec collisions.
    #[must_use]
    pub fn identity_key(&self) -> Vec<u8> {
        let mut key = Vec::with_capacity(4 + self.peer_id.len());
        key.extend_from_slice(&self.record_type.to_be_bytes());
        key.extend_from_slice(&self.peer_id);
        key
    }
}

/// Pluggable cryptographic peer-record codec.
#[async_trait]
pub trait PeerRecordCodec: Send + Sync + fmt::Debug {
    /// Resurrect codec identifier handled by this implementation.
    fn record_type(&self) -> u32;

    /// Verifies and decodes raw signed bytes.
    ///
    /// Implementations must verify the signature and identity/key consistency
    /// before returning endpoints.
    async fn decode_and_verify(
        &self,
        raw: &[u8],
        dial_context: DialContext,
    ) -> Result<PeerCandidate, PeerRecordError>;
}

/// Signed peer-record validation errors.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PeerRecordError {
    /// The record is empty or too large for its codec.
    #[error("invalid record size: {0} bytes")]
    InvalidSize(usize),
    /// The record encoding is malformed.
    #[error("malformed peer record: {0}")]
    Malformed(String),
    /// The cryptographic signature is invalid.
    #[error("peer-record signature verification failed")]
    InvalidSignature,
    /// The payload peer identity does not match the signing key.
    #[error("peer identity does not match the signing key")]
    IdentityMismatch,
    /// No signed endpoint survives the selected dial policy.
    #[error("record has no endpoint accepted by the {0:?} dial context")]
    NoUsableEndpoint(DialContext),
    /// Resource caps were exceeded while decoding.
    #[error("peer record exceeds the endpoint cap of {0}")]
    TooManyEndpoints(usize),
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(value)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let stripped = value
            .strip_prefix("0x")
            .ok_or_else(|| de::Error::custom("byte strings must be 0x-prefixed"))?;
        hex::decode(stripped).map_err(de::Error::custom)
    }
}
