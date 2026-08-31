use crate::{
    BlockInfo, BlockReference, ProviderError, RegistryConstants, RegistryProvider,
    ResurrectRegistryV1,
};
use alloy::{
    eips::BlockNumberOrTag,
    network::EthereumWallet,
    primitives::{Address, B256, Bytes, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Filter,
    signers::local::PrivateKeySigner,
    sol_types::SolEvent,
};
use async_trait::async_trait;
use resurrect_core::{Announcement, Namespace};
use std::fmt;
use url::Url;

/// Alloy-backed HTTP provider with optional local transaction signer.
#[derive(Clone)]
pub struct AlloyRegistryProvider {
    provider: DynProvider,
    writable: bool,
}

impl fmt::Debug for AlloyRegistryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AlloyRegistryProvider")
            .field("writable", &self.writable)
            .finish_non_exhaustive()
    }
}

impl AlloyRegistryProvider {
    /// Creates a read-only provider from a caller-supplied JSON-RPC URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid.
    pub fn read_only(rpc_url: &str) -> Result<Self, ProviderError> {
        let url = parse_url(rpc_url)?;
        let provider = ProviderBuilder::new().connect_http(url).erased();
        Ok(Self {
            provider,
            writable: false,
        })
    }

    /// Creates a provider that signs announcement transactions locally.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid. The caller retains responsibility
    /// for securely obtaining and storing the signer.
    pub fn with_signer(rpc_url: &str, signer: PrivateKeySigner) -> Result<Self, ProviderError> {
        let url = parse_url(rpc_url)?;
        let wallet = EthereumWallet::from(signer);
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(url)
            .erased();
        Ok(Self {
            provider,
            writable: true,
        })
    }
}

#[async_trait]
impl RegistryProvider for AlloyRegistryProvider {
    async fn chain_id(&self) -> Result<U256, ProviderError> {
        self.provider
            .client()
            .request_noparams::<U256>("eth_chainId")
            .await
            .map_err(map_request_error)
    }

    async fn registry_constants(
        &self,
        address: Address,
    ) -> Result<RegistryConstants, ProviderError> {
        let contract = ResurrectRegistryV1::new(address, &self.provider);
        let version = contract.VERSION().call().await.map_err(map_request_error)?;
        let max_ttl = contract.MAX_TTL().call().await.map_err(map_request_error)?;
        let max_record_bytes = contract
            .MAX_RECORD_BYTES()
            .call()
            .await
            .map_err(map_request_error)?;
        Ok(RegistryConstants {
            version,
            max_ttl,
            max_record_bytes,
        })
    }

    async fn block(&self, reference: BlockReference) -> Result<Option<BlockInfo>, ProviderError> {
        let tag = match reference {
            BlockReference::Number(number) => BlockNumberOrTag::Number(number),
            BlockReference::Latest => BlockNumberOrTag::Latest,
            BlockReference::Safe => BlockNumberOrTag::Safe,
            BlockReference::Finalized => BlockNumberOrTag::Finalized,
        };
        let block = self
            .provider
            .get_block_by_number(tag)
            .await
            .map_err(|error| map_block_error(reference, error))?;
        Ok(block.map(|block| BlockInfo {
            number: block.header.number,
            timestamp: block.header.timestamp,
            hash: block.header.hash,
        }))
    }

    async fn announcements(
        &self,
        address: Address,
        namespace: Namespace,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Announcement>, ProviderError> {
        let filter = Filter::new()
            .address(address)
            .event_signature(ResurrectRegistryV1::PeerAnnounced::SIGNATURE_HASH)
            .topic1(namespace.as_b256())
            .from_block(from_block)
            .to_block(to_block);
        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .map_err(map_log_error)?;

        logs.into_iter()
            .filter(|log| !log.removed)
            .map(|log| {
                let block_number = log
                    .block_number
                    .ok_or(ProviderError::Missing("log block"))?;
                let log_index = log.log_index.ok_or(ProviderError::Missing("log index"))?;
                let block_hash = log.block_hash;
                let decoded = log
                    .log_decode_validate::<ResurrectRegistryV1::PeerAnnounced>()
                    .map_err(|error| ProviderError::Request(error.to_string()))?;
                let event = decoded.data();
                Ok(Announcement {
                    namespace: Namespace::from_bytes(event.namespace.0),
                    record_type: event.recordType,
                    valid_until: event.validUntil,
                    peer_record: event.peerRecord.to_vec(),
                    block_number,
                    log_index,
                    block_hash,
                })
            })
            .collect()
    }

    async fn announce(
        &self,
        address: Address,
        namespace: Namespace,
        record_type: u32,
        ttl: u32,
        peer_record: &[u8],
    ) -> Result<B256, ProviderError> {
        if !self.writable {
            return Err(ProviderError::ReadOnly);
        }
        let contract = ResurrectRegistryV1::new(address, &self.provider);
        let pending = contract
            .announce(
                namespace.as_b256(),
                record_type,
                ttl,
                Bytes::copy_from_slice(peer_record),
            )
            .send()
            .await
            .map_err(map_request_error)?;
        let receipt = pending.get_receipt().await.map_err(map_request_error)?;
        Ok(receipt.transaction_hash)
    }
}

fn parse_url(rpc_url: &str) -> Result<Url, ProviderError> {
    let url =
        Url::parse(rpc_url).map_err(|error| ProviderError::Configuration(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ProviderError::Configuration(
            "only http and https JSON-RPC URLs are supported".to_owned(),
        ));
    }
    Ok(url)
}

fn map_request_error(error: impl fmt::Display) -> ProviderError {
    ProviderError::Request(error.to_string())
}

fn map_block_error(reference: BlockReference, error: impl fmt::Display) -> ProviderError {
    let message = error.to_string();
    if !matches!(
        reference,
        BlockReference::Number(_) | BlockReference::Latest
    ) && looks_unsupported(&message)
    {
        ProviderError::Unsupported(message)
    } else {
        ProviderError::Request(message)
    }
}

fn map_log_error(error: impl fmt::Display) -> ProviderError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("range")
        || lower.contains("too many")
        || lower.contains("response size")
        || lower.contains("limit exceeded")
        || lower.contains("query returned more than")
    {
        ProviderError::RangeLimit(message)
    } else {
        ProviderError::Request(message)
    }
}

fn looks_unsupported(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("unsupported")
        || lower.contains("not found")
        || lower.contains("invalid argument")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_urls_without_leaking_credentials() {
        let error = AlloyRegistryProvider::read_only("file:///tmp/node.ipc").unwrap_err();
        assert!(matches!(error, ProviderError::Configuration(_)));
    }

    #[tokio::test]
    async fn read_only_provider_refuses_publication_before_network_io() {
        let provider = AlloyRegistryProvider::read_only("http://127.0.0.1:1").unwrap();
        assert_eq!(
            provider
                .announce(Address::ZERO, Namespace::default(), 2, 1, &[1],)
                .await,
            Err(ProviderError::ReadOnly)
        );
    }
}
