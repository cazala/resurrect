//! Native RBP reference node components.
//!
//! The bootstrap controller depends only on replaceable discovery, dial, and
//! announcement traits. [`Libp2pHost`] is the production Tokio/rust-libp2p
//! implementation used by the CLI.

mod bootstrap;
mod identity;
mod p2p;

pub use bootstrap::{
    AnnouncementPublisher, BootstrapController, BootstrapError, BootstrapOutcome, BootstrapPolicy,
    BootstrapState, DiscoverySource, NativeDiscovery, PeerConnector,
};
pub use identity::{IdentityError, load_or_create_identity};
pub use p2p::{HostConfig, HostError, HostStatus, Libp2pHost, Libp2pHostHandle, NativePeerSource};
