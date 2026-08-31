use alloy_primitives::{Address, B256, U256};
use async_trait::async_trait;
use resurrect_core::{Announcement, Namespace};
use std::fmt::Debug;
use thiserror::Error;

/// Canonical immutable registry constants read from the deployed bytecode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryConstants {
    /// Contract protocol version.
    pub version: u32,
    /// Maximum announcement TTL.
    pub max_ttl: u32,
    /// Maximum record envelope bytes.
    pub max_record_bytes: u32,
}

/// Minimal block metadata required for recent-window search and reorg checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockInfo {
    /// Block number.
    pub number: u64,
    /// Consensus timestamp in seconds.
    pub timestamp: u64,
    /// Block hash.
    pub hash: B256,
}

/// Block selector supported by the scanner abstraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReference {
    /// Explicit block number.
    Number(u64),
    /// Current canonical chain head.
    Latest,
    /// EIP-1898 safe head when supported.
    Safe,
    /// Finalized head when supported.
    Finalized,
}

/// Caller-supplied registry-chain transport abstraction.
#[async_trait]
pub trait RegistryProvider: Send + Sync + Debug {
    /// Returns `eth_chainId` without requesting account exposure.
    async fn chain_id(&self) -> Result<U256, ProviderError>;

    /// Reads canonical registry constants.
    async fn registry_constants(
        &self,
        address: Address,
    ) -> Result<RegistryConstants, ProviderError>;

    /// Loads a block header by number or finality tag.
    async fn block(&self, reference: BlockReference) -> Result<Option<BlockInfo>, ProviderError>;

    /// Queries one bounded range of namespace-filtered announcement logs.
    async fn announcements(
        &self,
        address: Address,
        namespace: Namespace,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Announcement>, ProviderError>;

    /// Publishes an announcement and waits for inclusion.
    ///
    /// Read-only implementations return [`ProviderError::ReadOnly`].
    async fn announce(
        &self,
        address: Address,
        namespace: Namespace,
        record_type: u32,
        ttl: u32,
        peer_record: &[u8],
    ) -> Result<B256, ProviderError>;
}

/// Provider transport, RPC, decoding, and policy errors.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    /// Invalid provider URL or configuration.
    #[error("invalid provider configuration: {0}")]
    Configuration(String),
    /// Ordinary transport/RPC failure.
    #[error("registry provider request failed: {0}")]
    Request(String),
    /// Provider rejected a log query range/response size.
    #[error("registry provider log range is too large: {0}")]
    RangeLimit(String),
    /// Optional finality tag/method is unsupported.
    #[error("registry provider feature is unsupported: {0}")]
    Unsupported(String),
    /// Expected chain object was missing.
    #[error("registry provider returned no {0}")]
    Missing(&'static str),
    /// Publishing was attempted through a read-only provider.
    #[error("registry provider is read-only")]
    ReadOnly,
}
