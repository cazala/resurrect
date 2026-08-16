use crate::{BlockInfo, BlockReference, ProviderError, RegistryConstants, RegistryProvider};
use alloy_primitives::{B256, keccak256};
use rbp_core::{
    AnnouncementPolicy, CandidateStore, CandidateStoreConfig, CodecRegistry, DialContext,
    MAX_TTL_SECONDS, NetworkDescriptor, PeerCandidate, RBP_VERSION,
};
use thiserror::Error;

/// Default initial `eth_getLogs` block range.
pub const DEFAULT_INITIAL_CHUNK_SIZE: u64 = 20_000;
/// Hard cap on raw logs decoded during one scan.
pub const DEFAULT_MAX_LOGS_PER_SCAN: usize = 50_000;

/// Resource, finality, and endpoint policy for one scanner.
#[derive(Clone, Debug)]
pub struct ScannerConfig {
    /// Initial bounded log-query width; providers can force automatic reduction.
    pub initial_chunk_size: u64,
    /// Smallest retry width after range errors.
    pub minimum_chunk_size: u64,
    /// Maximum raw events processed per scan.
    pub max_logs_per_scan: usize,
    /// Maximum distinct validated candidates retained.
    pub max_candidates: usize,
    /// Maximum signed endpoints decoded per record.
    pub max_endpoints_per_record: usize,
    /// Confirmations used if safe/finalized tags are unavailable.
    pub fallback_confirmations: u64,
    /// Prefer safe/finalized tags when the provider exposes them.
    /// Disable only for development chains without progressing finality.
    pub use_finality_tags: bool,
    /// Client-environment endpoint profile.
    pub dial_context: DialContext,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            initial_chunk_size: DEFAULT_INITIAL_CHUNK_SIZE,
            minimum_chunk_size: 64,
            max_logs_per_scan: DEFAULT_MAX_LOGS_PER_SCAN,
            max_candidates: rbp_core::DEFAULT_MAX_ACTIVE_CANDIDATES,
            max_endpoints_per_record: rbp_core::DEFAULT_MAX_ENDPOINTS_PER_RECORD,
            fallback_confirmations: 12,
            use_finality_tags: true,
            dial_context: DialContext::NativeServer,
        }
    }
}

/// Persistable head marker used only to detect reorgs between scans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanCheckpoint {
    /// Previously scanned head number.
    pub number: u64,
    /// Previously scanned head hash.
    pub hash: B256,
}

/// Complete bounded scan result and observability counters.
#[derive(Clone, Debug)]
pub struct ScanReport {
    /// First block queried after timestamp binary search.
    pub start_block: u64,
    /// Sufficiently confirmed scan head.
    pub head: BlockInfo,
    /// Marker for the next scan.
    pub checkpoint: ScanCheckpoint,
    /// Whether a supplied prior checkpoint disappeared or changed hash.
    pub reorg_detected: bool,
    /// Raw registry events processed under the hard cap.
    pub logs_processed: usize,
    /// Events rejected by envelope, codec, signature, identity, or endpoint policy.
    pub records_rejected: usize,
    /// Number of range-limit reductions performed.
    pub chunk_reductions: usize,
    /// Sequence-deduplicated, deterministically sampled candidates.
    pub candidates: Vec<PeerCandidate>,
}

/// Recent-window RBP registry scanner.
#[derive(Debug)]
pub struct RegistryScanner<'a, P: RegistryProvider + ?Sized> {
    provider: &'a P,
    codecs: &'a CodecRegistry,
    config: ScannerConfig,
}

impl<'a, P: RegistryProvider + ?Sized> RegistryScanner<'a, P> {
    /// Creates a scanner over caller-owned provider and codec abstractions.
    pub const fn new(provider: &'a P, codecs: &'a CodecRegistry, config: ScannerConfig) -> Self {
        Self {
            provider,
            codecs,
            config,
        }
    }

    /// Scans only blocks that can contain non-expired announcements.
    ///
    /// # Errors
    ///
    /// Fails closed on descriptor/chain/contract mismatches, missing blocks,
    /// irreducible provider range errors, or ordinary provider failures.
    pub async fn scan(
        &self,
        descriptor: &NetworkDescriptor,
        local_node_id: &[u8],
        previous: Option<ScanCheckpoint>,
    ) -> Result<ScanReport, ScanError> {
        descriptor
            .validate()
            .map_err(|error| ScanError::Descriptor(error.to_string()))?;
        let actual_chain_id = self.provider.chain_id().await?;
        if actual_chain_id != descriptor.registry.chain_id {
            return Err(ScanError::WrongChain {
                expected: descriptor.registry.chain_id,
                actual: actual_chain_id,
            });
        }
        let constants = self
            .provider
            .registry_constants(descriptor.registry.address)
            .await?;
        verify_constants(constants, descriptor)?;

        let head = self.confirmed_head().await?;
        if descriptor.registry.deployment_block > head.number {
            return Err(ScanError::DeploymentAfterHead {
                deployment: descriptor.registry.deployment_block,
                head: head.number,
            });
        }
        let reorg_detected = self.detect_reorg(previous).await?;
        let cutoff = head
            .timestamp
            .saturating_sub(u64::from(descriptor.registry.max_ttl_seconds));
        let start_block = self
            .find_start_block(descriptor.registry.deployment_block, head.number, cutoff)
            .await?;

        let scanned = self
            .scan_logs(descriptor, local_node_id, constants, start_block, head)
            .await?;

        Ok(ScanReport {
            start_block,
            head,
            checkpoint: ScanCheckpoint {
                number: head.number,
                hash: head.hash,
            },
            reorg_detected,
            logs_processed: scanned.logs_processed,
            records_rejected: scanned.records_rejected,
            chunk_reductions: scanned.chunk_reductions,
            candidates: scanned.candidates,
        })
    }

    async fn scan_logs(
        &self,
        descriptor: &NetworkDescriptor,
        local_node_id: &[u8],
        constants: RegistryConstants,
        start_block: u64,
        head: BlockInfo,
    ) -> Result<LogScanOutcome, ScanError> {
        let sampling_seed =
            sampling_seed(local_node_id, head.hash, descriptor.namespace.as_bytes());
        let mut candidates = CandidateStore::new(CandidateStoreConfig {
            max_candidates: self.config.max_candidates,
            sampling_seed,
        });
        let mut logs_processed = 0_usize;
        let mut records_rejected = 0_usize;
        let mut chunk_reductions = 0_usize;
        let mut chunk_size = self.config.initial_chunk_size.max(1);
        let minimum_chunk_size = self.config.minimum_chunk_size.max(1);
        let mut from = start_block;

        while from <= head.number && logs_processed < self.config.max_logs_per_scan {
            let to = from
                .saturating_add(chunk_size.saturating_sub(1))
                .min(head.number);
            let announcements = match self
                .provider
                .announcements(descriptor.registry.address, descriptor.namespace, from, to)
                .await
            {
                Ok(announcements) => announcements,
                Err(ProviderError::RangeLimit(_)) if chunk_size > minimum_chunk_size => {
                    chunk_size = (chunk_size / 2).max(minimum_chunk_size);
                    chunk_reductions += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            for announcement in announcements
                .into_iter()
                .take(self.config.max_logs_per_scan - logs_processed)
            {
                logs_processed += 1;
                let policy = AnnouncementPolicy {
                    expected_namespace: descriptor.namespace,
                    accepted_record_types: &descriptor.accepted_record_types,
                    chain_time: head.timestamp,
                    max_record_bytes: usize::try_from(constants.max_record_bytes)
                        .unwrap_or(rbp_core::DEFAULT_MAX_RECORD_BYTES),
                    max_endpoints: self.config.max_endpoints_per_record,
                    dial_context: self.config.dial_context,
                };
                match self
                    .codecs
                    .validate_announcement(&announcement, policy)
                    .await
                {
                    Ok(candidate) => {
                        candidates.insert(candidate);
                    }
                    Err(_) => records_rejected += 1,
                }
            }
            if to == head.number {
                break;
            }
            from = to + 1;
        }

        Ok(LogScanOutcome {
            logs_processed,
            records_rejected,
            chunk_reductions,
            candidates: candidates.ranked().into_iter().cloned().collect(),
        })
    }

    async fn confirmed_head(&self) -> Result<BlockInfo, ScanError> {
        if self.config.use_finality_tags {
            match self.provider.block(BlockReference::Finalized).await {
                Ok(Some(block)) => return Ok(block),
                Ok(None) | Err(ProviderError::Unsupported(_)) => {}
                Err(error) => return Err(error.into()),
            }
            match self.provider.block(BlockReference::Safe).await {
                Ok(Some(block)) => return Ok(block),
                Ok(None) | Err(ProviderError::Unsupported(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let latest = self
            .provider
            .block(BlockReference::Latest)
            .await?
            .ok_or(ScanError::MissingBlock("latest"))?;
        let confirmed_number = latest
            .number
            .saturating_sub(self.config.fallback_confirmations);
        self.provider
            .block(BlockReference::Number(confirmed_number))
            .await?
            .ok_or(ScanError::MissingBlock("confirmed"))
    }

    async fn detect_reorg(&self, previous: Option<ScanCheckpoint>) -> Result<bool, ScanError> {
        let Some(previous) = previous else {
            return Ok(false);
        };
        let current = self
            .provider
            .block(BlockReference::Number(previous.number))
            .await?;
        Ok(current.is_none_or(|block| block.hash != previous.hash))
    }

    async fn find_start_block(
        &self,
        deployment_block: u64,
        head: u64,
        cutoff: u64,
    ) -> Result<u64, ScanError> {
        if deployment_block >= head {
            return Ok(deployment_block.min(head));
        }
        let mut low = deployment_block;
        let mut high = head;
        while low < high {
            let middle = low + (high - low) / 2;
            let block = self
                .provider
                .block(BlockReference::Number(middle))
                .await?
                .ok_or(ScanError::MissingBlock("timestamp-search"))?;
            if block.timestamp < cutoff {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }
}

struct LogScanOutcome {
    logs_processed: usize,
    records_rejected: usize,
    chunk_reductions: usize,
    candidates: Vec<PeerCandidate>,
}

fn verify_constants(
    constants: RegistryConstants,
    descriptor: &NetworkDescriptor,
) -> Result<(), ScanError> {
    if constants.version != RBP_VERSION
        || constants.max_ttl != MAX_TTL_SECONDS
        || constants.max_ttl != descriptor.registry.max_ttl_seconds
        || usize::try_from(constants.max_record_bytes).ok()
            != Some(rbp_core::DEFAULT_MAX_RECORD_BYTES)
    {
        return Err(ScanError::RegistryConstants(constants));
    }
    Ok(())
}

fn sampling_seed(local_node_id: &[u8], block_hash: B256, namespace: &[u8; 32]) -> B256 {
    let mut input = Vec::with_capacity(local_node_id.len() + 64);
    input.extend_from_slice(local_node_id);
    input.extend_from_slice(block_hash.as_slice());
    input.extend_from_slice(namespace);
    keccak256(input)
}

/// Scanner configuration, conformance, and provider failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ScanError {
    /// Invalid network descriptor.
    #[error("invalid network descriptor: {0}")]
    Descriptor(String),
    /// Provider chain mismatch.
    #[error("provider chain ID {actual} does not match descriptor chain ID {expected}")]
    WrongChain {
        /// Descriptor chain.
        expected: u64,
        /// Provider chain.
        actual: u64,
    },
    /// Deployed contract constants differ from canonical v1.
    #[error("deployed registry constants are not RBP v1: {0:?}")]
    RegistryConstants(RegistryConstants),
    /// Required canonical block is absent.
    #[error("registry provider returned no {0} block")]
    MissingBlock(&'static str),
    /// Descriptor cannot be active at the selected chain head.
    #[error("registry deployment block {deployment} is newer than scan head {head}")]
    DeploymentAfterHead {
        /// Configured deployment block.
        deployment: u64,
        /// Selected sufficiently confirmed block.
        head: u64,
    },
    /// Provider failure.
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use async_trait::async_trait;
    use rbp_core::{
        Announcement, DiscoverySourceKind, Endpoint, Namespace, PeerRecordCodec, PeerRecordError,
        RECORD_TYPE_LIBP2P, RegistryDescriptor,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct TestCodec;

    #[async_trait]
    impl PeerRecordCodec for TestCodec {
        fn record_type(&self) -> u32 {
            RECORD_TYPE_LIBP2P
        }

        async fn decode_and_verify(
            &self,
            raw: &[u8],
            _dial_context: DialContext,
        ) -> Result<PeerCandidate, PeerRecordError> {
            if raw == b"invalid" {
                return Err(PeerRecordError::InvalidSignature);
            }
            Ok(PeerCandidate {
                record_type: RECORD_TYPE_LIBP2P,
                peer_id: vec![raw[0]],
                sequence: u64::from(raw[1]),
                endpoints: vec![Endpoint {
                    address: "/ip4/8.8.8.8/tcp/4001".to_owned(),
                }],
                raw_signed_record: raw.to_vec(),
                expires_at: 0,
                source: DiscoverySourceKind::RbpRegistry,
                announcement_block: None,
                announcement_log_index: None,
            })
        }
    }

    #[derive(Debug)]
    struct MockProvider {
        chain_id: u64,
        blocks: Mutex<Vec<BlockInfo>>,
        announcements: Mutex<Vec<Announcement>>,
        max_range: u64,
        ranges: Mutex<Vec<(u64, u64)>>,
        supports_finality: bool,
    }

    impl MockProvider {
        fn block_at(number: u64) -> BlockInfo {
            BlockInfo {
                number,
                timestamp: number * 86_400,
                hash: B256::from([number.to_le_bytes()[0]; 32]),
            }
        }
    }

    #[async_trait]
    impl RegistryProvider for MockProvider {
        async fn chain_id(&self) -> Result<u64, ProviderError> {
            Ok(self.chain_id)
        }

        async fn registry_constants(
            &self,
            _address: Address,
        ) -> Result<RegistryConstants, ProviderError> {
            Ok(RegistryConstants {
                version: 1,
                max_ttl: MAX_TTL_SECONDS,
                max_record_bytes: 4096,
            })
        }

        async fn block(
            &self,
            reference: BlockReference,
        ) -> Result<Option<BlockInfo>, ProviderError> {
            let blocks = self.blocks.lock().unwrap();
            Ok(match reference {
                BlockReference::Finalized | BlockReference::Safe if !self.supports_finality => {
                    return Err(ProviderError::Unsupported("test finality".to_owned()));
                }
                BlockReference::Finalized | BlockReference::Safe | BlockReference::Latest => {
                    blocks.last().copied()
                }
                BlockReference::Number(number) => {
                    blocks.iter().find(|block| block.number == number).copied()
                }
            })
        }

        async fn announcements(
            &self,
            _address: Address,
            _namespace: Namespace,
            from_block: u64,
            to_block: u64,
        ) -> Result<Vec<Announcement>, ProviderError> {
            self.ranges.lock().unwrap().push((from_block, to_block));
            if to_block - from_block + 1 > self.max_range {
                return Err(ProviderError::RangeLimit("test limit".to_owned()));
            }
            Ok(self
                .announcements
                .lock()
                .unwrap()
                .iter()
                .filter(|event| (from_block..=to_block).contains(&event.block_number))
                .cloned()
                .collect())
        }

        async fn announce(
            &self,
            _address: Address,
            _namespace: Namespace,
            _record_type: u32,
            _ttl: u32,
            _peer_record: &[u8],
        ) -> Result<B256, ProviderError> {
            Err(ProviderError::ReadOnly)
        }
    }

    fn descriptor(namespace: Namespace) -> NetworkDescriptor {
        NetworkDescriptor {
            rbp_version: 1,
            registry: RegistryDescriptor {
                chain_id: 31337,
                address: Address::repeat_byte(1),
                deployment_block: 0,
                max_ttl_seconds: MAX_TTL_SECONDS,
            },
            namespace,
            accepted_record_types: vec![RECORD_TYPE_LIBP2P],
        }
    }

    fn provider(events: Vec<Announcement>) -> MockProvider {
        MockProvider {
            chain_id: 31337,
            blocks: Mutex::new((0..=100).map(MockProvider::block_at).collect()),
            announcements: Mutex::new(events),
            max_range: 3,
            ranges: Mutex::new(Vec::new()),
            supports_finality: true,
        }
    }

    fn announcement(namespace: Namespace, block: u64, raw: &[u8]) -> Announcement {
        Announcement {
            namespace,
            record_type: RECORD_TYPE_LIBP2P,
            valid_until: 100 * 86_400 + 1,
            peer_record: raw.to_vec(),
            block_number: block,
            log_index: 0,
            block_hash: Some(B256::from([block.to_le_bytes()[0]; 32])),
        }
    }

    fn codecs() -> CodecRegistry {
        let mut codecs = CodecRegistry::new();
        codecs.register(Arc::new(TestCodec));
        codecs
    }

    #[tokio::test]
    async fn scans_only_ttl_window_and_reduces_chunks() {
        let namespace = Namespace::derive("scanner", 1);
        let provider = provider(vec![announcement(namespace, 50, &[1, 1])]);
        let config = ScannerConfig {
            initial_chunk_size: 20,
            minimum_chunk_size: 1,
            ..ScannerConfig::default()
        };
        let codecs = codecs();
        let report = RegistryScanner::new(&provider, &codecs, config)
            .scan(&descriptor(namespace), b"local", None)
            .await
            .unwrap();
        assert_eq!(report.start_block, 10);
        assert!(report.chunk_reductions > 0);
        assert_eq!(report.candidates.len(), 1);
        assert!(
            provider
                .ranges
                .lock()
                .unwrap()
                .iter()
                .all(|(from, _)| *from >= 10)
        );
    }

    #[tokio::test]
    async fn filters_invalid_expired_and_deduplicates_sequences() {
        let namespace = Namespace::derive("scanner", 1);
        let mut expired = announcement(namespace, 60, &[3, 1]);
        expired.valid_until = 100 * 86_400;
        let provider = provider(vec![
            announcement(namespace, 50, &[1, 1]),
            announcement(namespace, 51, &[1, 2]),
            announcement(namespace, 52, b"invalid"),
            expired,
        ]);
        let codecs = codecs();
        let report = RegistryScanner::new(
            &provider,
            &codecs,
            ScannerConfig {
                initial_chunk_size: 3,
                minimum_chunk_size: 1,
                ..ScannerConfig::default()
            },
        )
        .scan(&descriptor(namespace), b"local", None)
        .await
        .unwrap();
        assert_eq!(report.logs_processed, 4);
        assert_eq!(report.records_rejected, 2);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].sequence, 2);
    }

    #[tokio::test]
    async fn rejects_wrong_chain_before_scanning_logs() {
        let namespace = Namespace::derive("scanner", 1);
        let mut provider = provider(Vec::new());
        provider.chain_id = 1;
        let codecs = codecs();
        let error = RegistryScanner::new(&provider, &codecs, ScannerConfig::default())
            .scan(&descriptor(namespace), b"local", None)
            .await
            .unwrap_err();
        assert!(matches!(error, ScanError::WrongChain { .. }));
        assert!(provider.ranges.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn falls_back_to_confirmations_without_finality_tags() {
        let namespace = Namespace::derive("scanner", 1);
        let mut provider = provider(Vec::new());
        provider.supports_finality = false;
        let codecs = codecs();
        let report = RegistryScanner::new(
            &provider,
            &codecs,
            ScannerConfig {
                initial_chunk_size: 3,
                minimum_chunk_size: 1,
                fallback_confirmations: 12,
                ..ScannerConfig::default()
            },
        )
        .scan(&descriptor(namespace), b"local", None)
        .await
        .unwrap();
        assert_eq!(report.head.number, 88);
    }

    #[tokio::test]
    async fn can_explicitly_use_confirmations_on_development_chains() {
        let namespace = Namespace::derive("scanner", 1);
        let provider = provider(Vec::new());
        let codecs = codecs();
        let report = RegistryScanner::new(
            &provider,
            &codecs,
            ScannerConfig {
                initial_chunk_size: 3,
                minimum_chunk_size: 1,
                fallback_confirmations: 2,
                use_finality_tags: false,
                ..ScannerConfig::default()
            },
        )
        .scan(&descriptor(namespace), b"local", None)
        .await
        .unwrap();
        assert_eq!(report.head.number, 98);
    }

    #[tokio::test]
    async fn enforces_raw_log_processing_cap() {
        let namespace = Namespace::derive("scanner", 1);
        let events = (10..30)
            .map(|block| announcement(namespace, block, &[block.to_le_bytes()[0], 1]))
            .collect();
        let mut provider = provider(events);
        provider.max_range = 100;
        let codecs = codecs();
        let report = RegistryScanner::new(
            &provider,
            &codecs,
            ScannerConfig {
                initial_chunk_size: 100,
                minimum_chunk_size: 1,
                max_logs_per_scan: 5,
                ..ScannerConfig::default()
            },
        )
        .scan(&descriptor(namespace), b"local", None)
        .await
        .unwrap();
        assert_eq!(report.logs_processed, 5);
        assert_eq!(report.candidates.len(), 5);
    }

    #[tokio::test]
    async fn rejects_descriptor_deployed_after_head() {
        let namespace = Namespace::derive("scanner", 1);
        let provider = provider(Vec::new());
        let mut descriptor = descriptor(namespace);
        descriptor.registry.deployment_block = 101;
        let codecs = codecs();
        let error = RegistryScanner::new(&provider, &codecs, ScannerConfig::default())
            .scan(&descriptor, b"local", None)
            .await
            .unwrap_err();
        assert!(matches!(error, ScanError::DeploymentAfterHead { .. }));
    }

    #[tokio::test]
    async fn detects_reorg_and_rebuilds_candidate_view() {
        let namespace = Namespace::derive("scanner", 1);
        let provider = provider(vec![announcement(namespace, 50, &[1, 1])]);
        let codecs = codecs();
        let scanner = RegistryScanner::new(
            &provider,
            &codecs,
            ScannerConfig {
                initial_chunk_size: 3,
                minimum_chunk_size: 1,
                ..ScannerConfig::default()
            },
        );
        let first = scanner
            .scan(&descriptor(namespace), b"local", None)
            .await
            .unwrap();
        provider.blocks.lock().unwrap()[100].hash = B256::repeat_byte(0xaa);
        let second = scanner
            .scan(&descriptor(namespace), b"local", Some(first.checkpoint))
            .await
            .unwrap();
        assert!(second.reorg_detected);
        assert_eq!(second.candidates.len(), 1);
    }
}
