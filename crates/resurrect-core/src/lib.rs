//! Protocol-independent primitives for Resurrect v1.
//!
//! This crate intentionally contains no concrete Ethereum transport or P2P
//! implementation. Applications can validate descriptors and announcements,
//! register signed-record codecs, and keep a deterministic bounded candidate
//! set without coupling their bootstrap state machine to a provider or host.

mod descriptor;
mod namespace;
mod peer_record;
mod validation;

pub use descriptor::{
    DescriptorError, ETHEREUM_MAINNET_CHAIN_ID, ETHEREUM_MAINNET_REGISTRY_ADDRESS,
    ETHEREUM_MAINNET_REGISTRY_DEPLOYMENT_BLOCK, NetworkDescriptor, RESURRECT_VERSION,
    RegistryDescriptor, ethereum_mainnet_registry,
};
pub use namespace::{Namespace, NamespaceError};
pub use peer_record::{
    DialContext, DiscoverySourceKind, Endpoint, PeerCandidate, PeerRecordCodec, PeerRecordError,
    RECORD_TYPE_ENR, RECORD_TYPE_LIBP2P,
};
pub use validation::{
    Announcement, AnnouncementError, AnnouncementPolicy, CandidateInsert, CandidateStore,
    CandidateStoreConfig, CodecRegistry, DEFAULT_MAX_ACTIVE_CANDIDATES,
    DEFAULT_MAX_ENDPOINTS_PER_RECORD, DEFAULT_MAX_RECORD_BYTES,
};

/// The registry's protocol-wide maximum TTL: 90 days.
pub const MAX_TTL_SECONDS: u32 = 90 * 24 * 60 * 60;
