use crate::{AnnouncementPublisher, BootstrapError, DiscoverySource, SqlitePeerCache};
use async_trait::async_trait;
use rbp_core::{CodecRegistry, Namespace, NetworkDescriptor, PeerCandidate, RECORD_TYPE_LIBP2P};
use rbp_ethereum::{RegistryProvider, RegistryScanner, ScanCheckpoint, ScannerConfig};
use rbp_libp2p::{Keypair, Multiaddr, sign_peer_record};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

/// Shared lock-free counters used by logs, status files, and integration tests.
#[derive(Debug, Default)]
pub struct RegistryTelemetry {
    scans: AtomicU64,
    scan_failures: AtomicU64,
    logs_processed: AtomicU64,
    rejected_records: AtomicU64,
    reorgs: AtomicU64,
    announcements: AtomicU64,
}

impl RegistryTelemetry {
    /// Returns a consistent-enough observability snapshot. These values never
    /// participate in protocol decisions.
    #[must_use]
    pub fn snapshot(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            scans: self.scans.load(Ordering::Relaxed),
            scan_failures: self.scan_failures.load(Ordering::Relaxed),
            logs_processed: self.logs_processed.load(Ordering::Relaxed),
            rejected_records: self.rejected_records.load(Ordering::Relaxed),
            reorgs: self.reorgs.load(Ordering::Relaxed),
            announcements: self.announcements.load(Ordering::Relaxed),
        }
    }
}

/// Serializable registry activity counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    /// Successful registry scans.
    pub scans: u64,
    /// Failed registry scans.
    pub scan_failures: u64,
    /// Raw logs processed under scanner caps.
    pub logs_processed: u64,
    /// Invalid or unusable peer records rejected.
    pub rejected_records: u64,
    /// Prior scan checkpoints invalidated by reorgs.
    pub reorgs: u64,
    /// Included self-announcement transactions.
    pub announcements: u64,
}

/// Production discovery adapter over the provider-neutral registry scanner.
#[derive(Debug)]
pub struct RegistryDiscovery {
    provider: Arc<dyn RegistryProvider>,
    descriptor: NetworkDescriptor,
    codecs: Arc<CodecRegistry>,
    scanner_config: ScannerConfig,
    local_node_id: Vec<u8>,
    checkpoint: Mutex<Option<ScanCheckpoint>>,
    cache: Option<SqlitePeerCache>,
    telemetry: Arc<RegistryTelemetry>,
}

impl RegistryDiscovery {
    /// Creates a registry source. All chain access remains caller-supplied.
    #[must_use]
    pub fn new(
        provider: Arc<dyn RegistryProvider>,
        descriptor: NetworkDescriptor,
        codecs: Arc<CodecRegistry>,
        scanner_config: ScannerConfig,
        local_node_id: Vec<u8>,
        cache: Option<SqlitePeerCache>,
        telemetry: Arc<RegistryTelemetry>,
    ) -> Self {
        Self {
            provider,
            descriptor,
            codecs,
            scanner_config,
            local_node_id,
            checkpoint: Mutex::new(None),
            cache,
            telemetry,
        }
    }
}

#[async_trait]
impl DiscoverySource for RegistryDiscovery {
    async fn discover(&self, namespace: Namespace) -> Result<Vec<PeerCandidate>, BootstrapError> {
        if namespace != self.descriptor.namespace {
            return Err(BootstrapError::Discovery(
                "bootstrap namespace differs from descriptor".to_owned(),
            ));
        }
        let previous = *self.checkpoint.lock().await;
        let scanner = RegistryScanner::new(
            self.provider.as_ref(),
            self.codecs.as_ref(),
            self.scanner_config.clone(),
        );
        let report = match scanner
            .scan(&self.descriptor, &self.local_node_id, previous)
            .await
        {
            Ok(report) => report,
            Err(error) => {
                self.telemetry.scan_failures.fetch_add(1, Ordering::Relaxed);
                return Err(BootstrapError::Discovery(error.to_string()));
            }
        };
        *self.checkpoint.lock().await = Some(report.checkpoint);
        self.telemetry.scans.fetch_add(1, Ordering::Relaxed);
        self.telemetry
            .logs_processed
            .fetch_add(report.logs_processed as u64, Ordering::Relaxed);
        self.telemetry
            .rejected_records
            .fetch_add(report.records_rejected as u64, Ordering::Relaxed);
        if report.reorg_detected {
            self.telemetry.reorgs.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(cache) = &self.cache
            && let Err(error) = cache.store_verified(namespace, &report.candidates).await
        {
            tracing::warn!(%error, "could not update disposable peer cache");
        }
        Ok(report.candidates)
    }
}

/// Seed self-announcement and renewal adapter.
#[derive(Debug)]
pub struct RegistryAnnouncer {
    provider: Arc<dyn RegistryProvider>,
    descriptor: NetworkDescriptor,
    keypair: Keypair,
    addresses: Vec<Multiaddr>,
    eligible: bool,
    sequence: AtomicU64,
    next_renewal: AtomicU64,
    renewal_interval: Duration,
    telemetry: Arc<RegistryTelemetry>,
}

impl RegistryAnnouncer {
    /// Creates an announcer. Eligibility should only be set for nodes whose
    /// signed endpoints are intentionally publicly reachable.
    #[must_use]
    pub fn new(
        provider: Arc<dyn RegistryProvider>,
        descriptor: NetworkDescriptor,
        keypair: Keypair,
        addresses: Vec<Multiaddr>,
        eligible: bool,
        renewal_interval: Duration,
        telemetry: Arc<RegistryTelemetry>,
    ) -> Self {
        Self {
            provider,
            descriptor,
            keypair,
            addresses,
            eligible,
            sequence: AtomicU64::new(sequence_now()),
            next_renewal: AtomicU64::new(0),
            renewal_interval,
            telemetry,
        }
    }
}

#[async_trait]
impl AnnouncementPublisher for RegistryAnnouncer {
    fn eligible(&self) -> bool {
        self.eligible && self.renewal_due()
    }

    fn renewal_due(&self) -> bool {
        self.eligible
            && unix_time().is_ok_and(|now| now >= self.next_renewal.load(Ordering::Relaxed))
    }

    async fn announce(&self, ttl: Duration) -> Result<(), BootstrapError> {
        if !self.eligible {
            return Ok(());
        }
        if !self
            .descriptor
            .accepted_record_types
            .contains(&RECORD_TYPE_LIBP2P)
        {
            return Err(BootstrapError::Announcement(
                "descriptor does not accept libp2p peer records".to_owned(),
            ));
        }
        let ttl = u32::try_from(ttl.as_secs()).map_err(|_| {
            BootstrapError::Announcement("announcement TTL exceeds uint32".to_owned())
        })?;
        if ttl == 0 || ttl > self.descriptor.registry.max_ttl_seconds {
            return Err(BootstrapError::Announcement(
                "announcement TTL is outside registry bounds".to_owned(),
            ));
        }
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        let record = sign_peer_record(&self.keypair, sequence, &self.addresses)
            .map_err(|error| BootstrapError::Announcement(error.to_string()))?;
        let transaction = self
            .provider
            .announce(
                self.descriptor.registry.address,
                self.descriptor.namespace,
                RECORD_TYPE_LIBP2P,
                ttl,
                &record,
            )
            .await
            .map_err(|error| BootstrapError::Announcement(error.to_string()))?;
        let now = unix_time().map_err(BootstrapError::Announcement)?;
        let renewal_after = self
            .renewal_interval
            .as_secs()
            .min(u64::from(ttl).saturating_div(2).max(1));
        self.next_renewal
            .store(now.saturating_add(renewal_after), Ordering::Relaxed);
        self.telemetry.announcements.fetch_add(1, Ordering::Relaxed);
        tracing::info!(%transaction, sequence, ttl, "published signed RBP peer record");
        Ok(())
    }
}

fn sequence_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX - 1)
        })
}

fn unix_time() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock predates the Unix epoch".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, B256};
    use rbp_core::{MAX_TTL_SECONDS, RegistryDescriptor};
    use rbp_ethereum::{BlockInfo, BlockReference, ProviderError, RegistryConstants};
    use std::sync::Mutex as StdMutex;

    #[derive(Debug)]
    struct Provider {
        published: StdMutex<Vec<Vec<u8>>>,
    }

    #[async_trait]
    impl RegistryProvider for Provider {
        async fn chain_id(&self) -> Result<u64, ProviderError> {
            Ok(31337)
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
            _reference: BlockReference,
        ) -> Result<Option<BlockInfo>, ProviderError> {
            Ok(Some(BlockInfo {
                number: 1,
                timestamp: unix_time().unwrap(),
                hash: B256::ZERO,
            }))
        }

        async fn announcements(
            &self,
            _address: Address,
            _namespace: Namespace,
            _from_block: u64,
            _to_block: u64,
        ) -> Result<Vec<rbp_core::Announcement>, ProviderError> {
            Ok(Vec::new())
        }

        async fn announce(
            &self,
            _address: Address,
            _namespace: Namespace,
            _record_type: u32,
            _ttl: u32,
            peer_record: &[u8],
        ) -> Result<B256, ProviderError> {
            self.published.lock().unwrap().push(peer_record.to_vec());
            Ok(B256::ZERO)
        }
    }

    fn descriptor() -> NetworkDescriptor {
        NetworkDescriptor {
            rbp_version: 1,
            registry: RegistryDescriptor {
                chain_id: 31337,
                address: Address::ZERO,
                deployment_block: 0,
                max_ttl_seconds: MAX_TTL_SECONDS,
            },
            namespace: Namespace::derive("announcer", 1),
            accepted_record_types: vec![RECORD_TYPE_LIBP2P],
        }
    }

    #[tokio::test]
    async fn eligible_announcer_signs_and_tracks_renewal() {
        let provider = Arc::new(Provider {
            published: StdMutex::new(Vec::new()),
        });
        let telemetry = Arc::new(RegistryTelemetry::default());
        let announcer = RegistryAnnouncer::new(
            provider.clone(),
            descriptor(),
            Keypair::generate_ed25519(),
            vec!["/ip4/127.0.0.1/tcp/4001".parse().unwrap()],
            true,
            Duration::from_secs(600),
            Arc::clone(&telemetry),
        );
        assert!(announcer.renewal_due());
        announcer.announce(Duration::from_secs(60)).await.unwrap();
        assert!(!announcer.renewal_due());
        assert_eq!(provider.published.lock().unwrap().len(), 1);
        assert_eq!(telemetry.snapshot().announcements, 1);
    }
}
