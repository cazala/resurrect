//! Cryptographic peer-record codecs used by RBP v1.
//!
//! The libp2p implementation is interoperable with the current
//! `libp2p-peer-record` signed-envelope domain and multicodec. The ENR codec
//! consumes raw RLP bytes (never the textual `enr:` representation) and relies
//! on the EIP-778 decoder's signature verification.

mod endpoint_policy;
mod enr_codec;
mod signed_peer_record;

pub use endpoint_policy::EndpointPolicy;
pub use enr_codec::EnrCodec;
pub use libp2p_identity::{Keypair, PeerId, PublicKey};
pub use multiaddr::Multiaddr;
pub use signed_peer_record::{
    LIBP2P_PEER_RECORD_DOMAIN, LIBP2P_PEER_RECORD_PAYLOAD_TYPE, Libp2pPeerRecordCodec,
    SignedPeerRecordError, sign_peer_record,
};
