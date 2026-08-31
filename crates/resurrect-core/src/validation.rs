use crate::{DialContext, Namespace, PeerCandidate, PeerRecordCodec, PeerRecordError};
use alloy_primitives::{B256, keccak256};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};
use thiserror::Error;

/// Registry v1 maximum record envelope size.
pub const DEFAULT_MAX_RECORD_BYTES: usize = 4096;
/// Suggested maximum decoded endpoints per record.
pub const DEFAULT_MAX_ENDPOINTS_PER_RECORD: usize = 16;
/// Suggested maximum active candidate set.
pub const DEFAULT_MAX_ACTIVE_CANDIDATES: usize = 256;

/// Provider-neutral representation of a registry announcement log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// Indexed application namespace.
    pub namespace: Namespace,
    /// Indexed signed-record codec identifier.
    pub record_type: u32,
    /// Contract-derived expiration timestamp.
    pub valid_until: u64,
    /// Signed peer-record bytes.
    pub peer_record: Vec<u8>,
    /// Inclusion block.
    pub block_number: u64,
    /// Log position within the block/transaction receipt.
    pub log_index: u64,
    /// Inclusion block hash, if supplied by the provider.
    pub block_hash: Option<B256>,
}

/// Bounds and application policy applied to one registry scan.
#[derive(Clone, Copy, Debug)]
pub struct AnnouncementPolicy<'a> {
    /// Application namespace to accept.
    pub expected_namespace: Namespace,
    /// Descriptor-configured codecs to accept.
    pub accepted_record_types: &'a [u32],
    /// Timestamp of the latest sufficiently confirmed registry block.
    pub chain_time: u64,
    /// Maximum registry envelope bytes to decode.
    pub max_record_bytes: usize,
    /// Maximum signed endpoints retained from one record.
    pub max_endpoints: usize,
    /// Environment-specific endpoint selection rules.
    pub dial_context: DialContext,
}

/// Runtime registry of explicitly accepted codecs.
#[derive(Default, Debug)]
pub struct CodecRegistry {
    codecs: HashMap<u32, Arc<dyn PeerRecordCodec>>,
}

impl CodecRegistry {
    /// Creates an empty, deny-by-default codec registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces one codec implementation.
    pub fn register(&mut self, codec: Arc<dyn PeerRecordCodec>) {
        self.codecs.insert(codec.record_type(), codec);
    }

    /// Returns whether a codec has been explicitly registered.
    #[must_use]
    pub fn contains(&self, record_type: u32) -> bool {
        self.codecs.contains_key(&record_type)
    }

    /// Validates the announcement envelope and its signed peer record.
    ///
    /// # Errors
    ///
    /// Returns an envelope error before invoking a codec, or a peer-record error
    /// if cryptographic, identity, or endpoint validation fails.
    pub async fn validate_announcement(
        &self,
        announcement: &Announcement,
        policy: AnnouncementPolicy<'_>,
    ) -> Result<PeerCandidate, AnnouncementError> {
        if announcement.namespace != policy.expected_namespace {
            return Err(AnnouncementError::WrongNamespace);
        }
        if !policy
            .accepted_record_types
            .contains(&announcement.record_type)
        {
            return Err(AnnouncementError::UnsupportedRecordType(
                announcement.record_type,
            ));
        }
        if announcement.valid_until <= policy.chain_time {
            return Err(AnnouncementError::Expired {
                valid_until: announcement.valid_until,
                chain_time: policy.chain_time,
            });
        }
        if announcement.peer_record.is_empty()
            || announcement.peer_record.len() > policy.max_record_bytes
        {
            return Err(AnnouncementError::InvalidRecordSize(
                announcement.peer_record.len(),
            ));
        }

        let codec = self.codecs.get(&announcement.record_type).ok_or(
            AnnouncementError::UnsupportedRecordType(announcement.record_type),
        )?;
        let mut candidate = codec
            .decode_and_verify(&announcement.peer_record, policy.dial_context)
            .await?;
        if candidate.record_type != announcement.record_type {
            return Err(AnnouncementError::CodecTypeMismatch {
                announced: announcement.record_type,
                decoded: candidate.record_type,
            });
        }
        if candidate.endpoints.len() > policy.max_endpoints {
            return Err(AnnouncementError::PeerRecord(
                PeerRecordError::TooManyEndpoints(policy.max_endpoints),
            ));
        }

        candidate.expires_at = announcement.valid_until;
        candidate.source = crate::DiscoverySourceKind::ResurrectRegistry;
        candidate.announcement_block = Some(announcement.block_number);
        candidate.announcement_log_index = Some(announcement.log_index);
        Ok(candidate)
    }
}

/// Registry envelope or signed-record error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AnnouncementError {
    /// The event belongs to another application.
    #[error("announcement namespace does not match the configured network")]
    WrongNamespace,
    /// The application does not accept or implement this codec.
    #[error("unsupported record type {0}")]
    UnsupportedRecordType(u32),
    /// Registry expiration is not newer than chain time.
    #[error("announcement expired at {valid_until}; chain time is {chain_time}")]
    Expired {
        /// Contract-derived expiration.
        valid_until: u64,
        /// Latest sufficiently confirmed block timestamp.
        chain_time: u64,
    },
    /// Empty or oversized registry record.
    #[error("invalid registry peer-record size: {0} bytes")]
    InvalidRecordSize(usize),
    /// A buggy/malicious codec returned the wrong identifier.
    #[error("codec type mismatch: event {announced}, decoder {decoded}")]
    CodecTypeMismatch {
        /// Event codec.
        announced: u32,
        /// Decoder output codec.
        decoded: u32,
    },
    /// Cryptographic or codec-level failure.
    #[error(transparent)]
    PeerRecord(#[from] PeerRecordError),
}

/// Deterministic bounded-candidate configuration.
#[derive(Clone, Debug)]
pub struct CandidateStoreConfig {
    /// Maximum distinct peer identities retained.
    pub max_candidates: usize,
    /// Per-scan unpredictable seed derived from node ID, block hash, namespace.
    pub sampling_seed: B256,
}

impl Default for CandidateStoreConfig {
    fn default() -> Self {
        Self {
            max_candidates: DEFAULT_MAX_ACTIVE_CANDIDATES,
            sampling_seed: B256::ZERO,
        }
    }
}

/// Result of inserting a validated candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateInsert {
    /// New identity retained.
    Inserted,
    /// Existing identity replaced by a higher sequence/newer announcement.
    Replaced,
    /// Stale or same-position record ignored.
    Stale,
    /// Valid candidate fell outside the deterministic bounded sample.
    SampledOut,
}

/// Memory-bounded, sequence-aware candidate set.
#[derive(Debug)]
pub struct CandidateStore {
    config: CandidateStoreConfig,
    candidates: HashMap<Vec<u8>, PeerCandidate>,
}

impl CandidateStore {
    /// Creates a store. A zero candidate cap is promoted to one.
    #[must_use]
    pub fn new(mut config: CandidateStoreConfig) -> Self {
        config.max_candidates = config.max_candidates.max(1);
        Self {
            config,
            candidates: HashMap::new(),
        }
    }

    /// Number of distinct retained identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether no candidates are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Returns the retained record for a codec-defined identity.
    #[must_use]
    pub fn get(&self, record_type: u32, peer_id: &[u8]) -> Option<&PeerCandidate> {
        let mut key = Vec::with_capacity(4 + peer_id.len());
        key.extend_from_slice(&record_type.to_be_bytes());
        key.extend_from_slice(peer_id);
        self.candidates.get(&key)
    }

    /// Inserts a candidate with monotonic sequence and bounded sampling.
    pub fn insert(&mut self, candidate: PeerCandidate) -> CandidateInsert {
        let identity = candidate.identity_key();
        if let Some(existing) = self.candidates.get(&identity) {
            if compare_freshness(&candidate, existing) == Ordering::Greater {
                self.candidates.insert(identity, candidate);
                return CandidateInsert::Replaced;
            }
            return CandidateInsert::Stale;
        }

        if self.candidates.len() < self.config.max_candidates {
            self.candidates.insert(identity, candidate);
            return CandidateInsert::Inserted;
        }

        let incoming_score = self.score(&identity);
        let Some(worst) = self
            .candidates
            .keys()
            .max_by_key(|identity| self.score(identity))
            .cloned()
        else {
            self.candidates.insert(identity, candidate);
            return CandidateInsert::Inserted;
        };
        if incoming_score < self.score(&worst) {
            self.candidates.remove(&worst);
            self.candidates.insert(identity, candidate);
            CandidateInsert::Inserted
        } else {
            CandidateInsert::SampledOut
        }
    }

    /// Returns candidates in deterministic pseudo-random dial order.
    #[must_use]
    pub fn ranked(&self) -> Vec<&PeerCandidate> {
        let mut entries: Vec<_> = self.candidates.iter().collect();
        entries.sort_by_key(|(identity, _)| self.score(identity));
        entries
            .into_iter()
            .map(|(_, candidate)| candidate)
            .collect()
    }

    fn score(&self, identity: &[u8]) -> B256 {
        let mut input = Vec::with_capacity(32 + identity.len());
        input.extend_from_slice(self.config.sampling_seed.as_slice());
        input.extend_from_slice(identity);
        keccak256(input)
    }
}

fn compare_freshness(left: &PeerCandidate, right: &PeerCandidate) -> Ordering {
    left.sequence
        .cmp(&right.sequence)
        .then_with(|| {
            left.announcement_block
                .unwrap_or_default()
                .cmp(&right.announcement_block.unwrap_or_default())
        })
        .then_with(|| {
            left.announcement_log_index
                .unwrap_or_default()
                .cmp(&right.announcement_log_index.unwrap_or_default())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiscoverySourceKind, Endpoint, RECORD_TYPE_LIBP2P};
    use async_trait::async_trait;

    fn candidate(id: u8, sequence: u64) -> PeerCandidate {
        PeerCandidate {
            record_type: 2,
            peer_id: vec![id],
            sequence,
            endpoints: vec![Endpoint {
                address: format!("/ip4/192.0.2.{id}/tcp/4001"),
            }],
            raw_signed_record: vec![id],
            expires_at: 100,
            source: DiscoverySourceKind::ResurrectRegistry,
            announcement_block: Some(sequence),
            announcement_log_index: Some(0),
        }
    }

    #[derive(Debug)]
    struct TestCodec {
        decoded_type: u32,
    }

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
            let mut decoded = candidate(raw[0], 1);
            decoded.record_type = self.decoded_type;
            Ok(decoded)
        }
    }

    fn announcement(namespace: Namespace) -> Announcement {
        Announcement {
            namespace,
            record_type: RECORD_TYPE_LIBP2P,
            valid_until: 101,
            peer_record: vec![1],
            block_number: 10,
            log_index: 2,
            block_hash: Some(B256::repeat_byte(3)),
        }
    }

    fn policy(namespace: Namespace) -> AnnouncementPolicy<'static> {
        AnnouncementPolicy {
            expected_namespace: namespace,
            accepted_record_types: &[RECORD_TYPE_LIBP2P],
            chain_time: 100,
            max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
            max_endpoints: DEFAULT_MAX_ENDPOINTS_PER_RECORD,
            dial_context: DialContext::NativeServer,
        }
    }

    #[tokio::test]
    async fn announcement_validation_filters_before_and_after_codec() {
        let namespace = Namespace::derive("validation-test", 1);
        let mut codecs = CodecRegistry::new();
        codecs.register(Arc::new(TestCodec {
            decoded_type: RECORD_TYPE_LIBP2P,
        }));

        let valid = codecs
            .validate_announcement(&announcement(namespace), policy(namespace))
            .await
            .unwrap();
        assert_eq!(valid.expires_at, 101);
        assert_eq!(valid.announcement_block, Some(10));

        let mut invalid = announcement(Namespace::derive("other", 1));
        assert_eq!(
            codecs
                .validate_announcement(&invalid, policy(namespace))
                .await,
            Err(AnnouncementError::WrongNamespace)
        );

        invalid = announcement(namespace);
        invalid.valid_until = 100;
        assert!(matches!(
            codecs
                .validate_announcement(&invalid, policy(namespace))
                .await,
            Err(AnnouncementError::Expired { .. })
        ));

        invalid = announcement(namespace);
        invalid.peer_record.clear();
        assert_eq!(
            codecs
                .validate_announcement(&invalid, policy(namespace))
                .await,
            Err(AnnouncementError::InvalidRecordSize(0))
        );
    }

    #[tokio::test]
    async fn rejects_unregistered_and_mismatched_codecs() {
        let namespace = Namespace::derive("validation-test", 1);
        let codecs = CodecRegistry::new();
        assert_eq!(
            codecs
                .validate_announcement(&announcement(namespace), policy(namespace))
                .await,
            Err(AnnouncementError::UnsupportedRecordType(2))
        );

        let mut codecs = CodecRegistry::new();
        codecs.register(Arc::new(TestCodec { decoded_type: 1 }));
        assert_eq!(
            codecs
                .validate_announcement(&announcement(namespace), policy(namespace))
                .await,
            Err(AnnouncementError::CodecTypeMismatch {
                announced: 2,
                decoded: 1,
            })
        );
    }

    #[test]
    fn newer_sequence_wins_and_older_late_record_is_stale() {
        let mut store = CandidateStore::new(CandidateStoreConfig::default());
        assert_eq!(store.insert(candidate(1, 2)), CandidateInsert::Inserted);
        assert_eq!(store.insert(candidate(1, 3)), CandidateInsert::Replaced);
        assert_eq!(store.insert(candidate(1, 1)), CandidateInsert::Stale);
        assert_eq!(store.get(2, &[1]).unwrap().sequence, 3);
    }

    #[test]
    fn same_sequence_uses_newest_chain_position() {
        let mut store = CandidateStore::new(CandidateStoreConfig::default());
        let mut first = candidate(1, 1);
        first.announcement_block = Some(10);
        let mut second = first.clone();
        second.announcement_block = Some(11);
        assert_eq!(store.insert(first), CandidateInsert::Inserted);
        assert_eq!(store.insert(second), CandidateInsert::Replaced);
    }

    #[test]
    fn spam_is_deterministically_bounded_and_order_independent() {
        let config = CandidateStoreConfig {
            max_candidates: 16,
            sampling_seed: B256::repeat_byte(7),
        };
        let mut forward = CandidateStore::new(config.clone());
        let mut reverse = CandidateStore::new(config);
        for id in 0..=255 {
            forward.insert(candidate(id, 1));
        }
        for id in (0..=255).rev() {
            reverse.insert(candidate(id, 1));
        }
        assert_eq!(forward.len(), 16);
        assert_eq!(
            forward
                .ranked()
                .iter()
                .map(|p| p.peer_id.clone())
                .collect::<Vec<_>>(),
            reverse
                .ranked()
                .iter()
                .map(|p| p.peer_id.clone())
                .collect::<Vec<_>>()
        );
    }
}
