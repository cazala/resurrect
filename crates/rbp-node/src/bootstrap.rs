use async_trait::async_trait;
use futures::{StreamExt, stream};
use rbp_core::{Namespace, PeerCandidate};
use std::time::Duration;
use thiserror::Error;

/// Replaceable source of already-normalized candidate peers.
#[async_trait]
pub trait DiscoverySource: Send + Sync {
    /// Discovers candidates without dialing them.
    async fn discover(&self, namespace: Namespace) -> Result<Vec<PeerCandidate>, BootstrapError>;
}

/// Native discovery adapter that can retain verified RBP records.
#[async_trait]
pub trait NativeDiscovery: DiscoverySource {
    /// Feeds a cryptographically verified peer into the native peer store.
    async fn add_verified_peer(&self, peer: PeerCandidate) -> Result<(), BootstrapError>;
}

/// Bounded application transport dialer.
#[async_trait]
pub trait PeerConnector: Send + Sync {
    /// Number of currently connected application peers.
    async fn connected_peers(&self) -> usize;

    /// Dials one normalized candidate and completes after handshake success/failure.
    async fn connect(&self, peer: PeerCandidate, timeout: Duration) -> bool;
}

/// Gas-spending isolated-seed promotion adapter.
#[async_trait]
pub trait AnnouncementPublisher: Send + Sync {
    /// Whether this node is publicly reachable and configured to publish.
    fn eligible(&self) -> bool;

    /// Publishes the node's newest signed record.
    async fn announce(&self, ttl: Duration) -> Result<(), BootstrapError>;
}

/// Observable bootstrap state-machine states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapState {
    /// State machine entry.
    Start,
    /// Validating/dialing local cache entries.
    CacheDiscovery,
    /// Waiting for application-native discovery.
    NativeDiscovery,
    /// Scanning recent registry logs.
    RbpScan,
    /// Dialing a bounded candidate sample.
    Dialing,
    /// Every discovery source failed to produce enough connections.
    Isolated,
    /// Publishing a short-lived own record.
    AnnounceSelf,
    /// Waiting before the next native/RBP retry.
    Backoff,
    /// Minimum connectivity target reached.
    Connected,
}

/// One finite state-machine cycle result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapOutcome {
    /// Terminal state for this cycle.
    pub state: BootstrapState,
    /// Source that reached the connection target, if any.
    pub connected_via: Option<BootstrapState>,
    /// Whether isolated promotion published an announcement.
    pub announced: bool,
    /// Ordered observable transitions.
    pub transitions: Vec<BootstrapState>,
}

/// Application-specific connection and resource policy.
#[derive(Clone, Debug)]
pub struct BootstrapPolicy {
    /// Successful connection target.
    pub minimum_successful_peers: usize,
    /// Maximum simultaneous RBP/cache/native dials.
    pub maximum_parallel_dials: usize,
    /// Timeout for one dial and authenticated handshake.
    pub per_dial_timeout: Duration,
    /// Short isolated reboot announcement lifetime.
    pub reboot_ttl: Duration,
}

impl Default for BootstrapPolicy {
    fn default() -> Self {
        Self {
            minimum_successful_peers: 2,
            maximum_parallel_dials: 8,
            per_dial_timeout: Duration::from_secs(8),
            reboot_ttl: Duration::from_secs(7 * 24 * 60 * 60),
        }
    }
}

/// Interface-driven RBP startup controller.
#[derive(Debug)]
pub struct BootstrapController<'a, C, N, R, D, A>
where
    C: DiscoverySource + ?Sized,
    N: NativeDiscovery + ?Sized,
    R: DiscoverySource + ?Sized,
    D: PeerConnector + ?Sized,
    A: AnnouncementPublisher + ?Sized,
{
    cache: &'a C,
    native: &'a N,
    registry: &'a R,
    connector: &'a D,
    publisher: &'a A,
    policy: BootstrapPolicy,
}

impl<'a, C, N, R, D, A> BootstrapController<'a, C, N, R, D, A>
where
    C: DiscoverySource + ?Sized,
    N: NativeDiscovery + ?Sized,
    R: DiscoverySource + ?Sized,
    D: PeerConnector + ?Sized,
    A: AnnouncementPublisher + ?Sized,
{
    /// Creates a state machine over caller-owned replaceable interfaces.
    pub const fn new(
        cache: &'a C,
        native: &'a N,
        registry: &'a R,
        connector: &'a D,
        publisher: &'a A,
        policy: BootstrapPolicy,
    ) -> Self {
        Self {
            cache,
            native,
            registry,
            connector,
            publisher,
            policy,
        }
    }

    /// Executes cache → native → RBP → isolated promotion exactly once.
    ///
    /// Discovery-source errors are non-fatal hints and do not prevent later
    /// sources from running. Publication errors are returned because they can
    /// indicate a signer, balance, or chain configuration problem.
    ///
    /// # Errors
    ///
    /// Returns an error only when an eligible isolated node cannot publish.
    pub async fn run_cycle(
        &self,
        namespace: Namespace,
    ) -> Result<BootstrapOutcome, BootstrapError> {
        let mut transitions = vec![BootstrapState::Start];
        if self.connector.connected_peers().await >= self.connection_target() {
            transitions.push(BootstrapState::Connected);
            return Ok(connected_outcome(transitions, BootstrapState::Start));
        }

        transitions.push(BootstrapState::CacheDiscovery);
        if let Ok(peers) = self.cache.discover(namespace).await
            && self
                .dial_candidates(peers, &mut transitions, BootstrapState::CacheDiscovery)
                .await
        {
            return Ok(connected_outcome(
                transitions,
                BootstrapState::CacheDiscovery,
            ));
        }

        transitions.push(BootstrapState::NativeDiscovery);
        if let Ok(peers) = self.native.discover(namespace).await
            && self
                .dial_candidates(peers, &mut transitions, BootstrapState::NativeDiscovery)
                .await
        {
            return Ok(connected_outcome(
                transitions,
                BootstrapState::NativeDiscovery,
            ));
        }

        transitions.push(BootstrapState::RbpScan);
        if let Ok(peers) = self.registry.discover(namespace).await {
            for peer in &peers {
                let _ = self.native.add_verified_peer(peer.clone()).await;
            }
            if self
                .dial_candidates(peers, &mut transitions, BootstrapState::RbpScan)
                .await
            {
                return Ok(connected_outcome(transitions, BootstrapState::RbpScan));
            }
        }

        transitions.push(BootstrapState::Isolated);
        let mut announced = false;
        if self.publisher.eligible() {
            transitions.push(BootstrapState::AnnounceSelf);
            self.publisher.announce(self.policy.reboot_ttl).await?;
            announced = true;
        }
        transitions.push(BootstrapState::Backoff);
        Ok(BootstrapOutcome {
            state: BootstrapState::Backoff,
            connected_via: None,
            announced,
            transitions,
        })
    }

    async fn dial_candidates(
        &self,
        peers: Vec<PeerCandidate>,
        transitions: &mut Vec<BootstrapState>,
        source: BootstrapState,
    ) -> bool {
        if peers.is_empty() {
            return false;
        }
        transitions.push(BootstrapState::Dialing);
        let parallel = self.policy.maximum_parallel_dials.max(1);
        let timeout = self.policy.per_dial_timeout;
        let connector = self.connector;
        let mut dials = stream::iter(
            peers
                .into_iter()
                .map(move |peer| async move { connector.connect(peer, timeout).await }),
        )
        .buffer_unordered(parallel);

        while dials.next().await.is_some() {
            if self.connector.connected_peers().await >= self.connection_target() {
                transitions.push(source);
                transitions.push(BootstrapState::Connected);
                return true;
            }
        }
        false
    }

    const fn connection_target(&self) -> usize {
        if self.policy.minimum_successful_peers == 0 {
            1
        } else {
            self.policy.minimum_successful_peers
        }
    }
}

fn connected_outcome(
    transitions: Vec<BootstrapState>,
    connected_via: BootstrapState,
) -> BootstrapOutcome {
    BootstrapOutcome {
        state: BootstrapState::Connected,
        connected_via: Some(connected_via),
        announced: false,
        transitions,
    }
}

/// Bootstrap adapter failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BootstrapError {
    /// Discovery transport or validation failure.
    #[error("discovery failed: {0}")]
    Discovery(String),
    /// Self-announcement transaction failure.
    #[error("self-announcement failed: {0}")]
    Announcement(String),
    /// Native peer-store update failure.
    #[error("native peer-store update failed: {0}")]
    NativePeerStore(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rbp_core::{DiscoverySourceKind, Endpoint};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    #[derive(Default)]
    struct Source {
        peers: Vec<PeerCandidate>,
        fail: bool,
        added: Mutex<Vec<Vec<u8>>>,
    }

    #[async_trait]
    impl DiscoverySource for Source {
        async fn discover(
            &self,
            _namespace: Namespace,
        ) -> Result<Vec<PeerCandidate>, BootstrapError> {
            if self.fail {
                Err(BootstrapError::Discovery("expected".to_owned()))
            } else {
                Ok(self.peers.clone())
            }
        }
    }

    #[async_trait]
    impl NativeDiscovery for Source {
        async fn add_verified_peer(&self, peer: PeerCandidate) -> Result<(), BootstrapError> {
            self.added.lock().unwrap().push(peer.peer_id);
            Ok(())
        }
    }

    struct Connector {
        connected: AtomicUsize,
        succeed: bool,
    }

    #[async_trait]
    impl PeerConnector for Connector {
        async fn connected_peers(&self) -> usize {
            self.connected.load(Ordering::SeqCst)
        }

        async fn connect(&self, _peer: PeerCandidate, _timeout: Duration) -> bool {
            if self.succeed {
                self.connected.fetch_add(1, Ordering::SeqCst);
            }
            self.succeed
        }
    }

    struct Publisher {
        eligible: bool,
        announced: Arc<AtomicBool>,
    }

    #[async_trait]
    impl AnnouncementPublisher for Publisher {
        fn eligible(&self) -> bool {
            self.eligible
        }

        async fn announce(&self, _ttl: Duration) -> Result<(), BootstrapError> {
            self.announced.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn peer(id: u8) -> PeerCandidate {
        PeerCandidate {
            record_type: 2,
            peer_id: vec![id],
            sequence: 1,
            endpoints: vec![Endpoint {
                address: "/ip4/127.0.0.1/tcp/4001".to_owned(),
            }],
            raw_signed_record: vec![id],
            expires_at: u64::MAX,
            source: DiscoverySourceKind::RbpRegistry,
            announcement_block: None,
            announcement_log_index: None,
        }
    }

    fn policy() -> BootstrapPolicy {
        BootstrapPolicy {
            minimum_successful_peers: 1,
            per_dial_timeout: Duration::from_millis(10),
            ..BootstrapPolicy::default()
        }
    }

    #[tokio::test]
    async fn follows_cache_native_registry_order_and_stops_connected() {
        let cache = Source {
            fail: true,
            ..Source::default()
        };
        let native = Source::default();
        let registry = Source {
            peers: vec![peer(1)],
            ..Source::default()
        };
        let connector = Connector {
            connected: AtomicUsize::new(0),
            succeed: true,
        };
        let publisher = Publisher {
            eligible: true,
            announced: Arc::new(AtomicBool::new(false)),
        };
        let outcome =
            BootstrapController::new(&cache, &native, &registry, &connector, &publisher, policy())
                .run_cycle(Namespace::default())
                .await
                .unwrap();
        assert_eq!(outcome.state, BootstrapState::Connected);
        assert_eq!(outcome.connected_via, Some(BootstrapState::RbpScan));
        assert!(!outcome.announced);
        assert_eq!(native.added.lock().unwrap().as_slice(), &[vec![1]]);
    }

    #[tokio::test]
    async fn isolated_eligible_node_announces_and_backs_off() {
        let empty = Source::default();
        let native = Source::default();
        let connector = Connector {
            connected: AtomicUsize::new(0),
            succeed: false,
        };
        let announced = Arc::new(AtomicBool::new(false));
        let publisher = Publisher {
            eligible: true,
            announced: Arc::clone(&announced),
        };
        let outcome =
            BootstrapController::new(&empty, &native, &empty, &connector, &publisher, policy())
                .run_cycle(Namespace::default())
                .await
                .unwrap();
        assert_eq!(outcome.state, BootstrapState::Backoff);
        assert!(outcome.announced);
        assert!(announced.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn light_node_never_announces_when_isolated() {
        let empty = Source::default();
        let native = Source::default();
        let connector = Connector {
            connected: AtomicUsize::new(0),
            succeed: false,
        };
        let announced = Arc::new(AtomicBool::new(false));
        let publisher = Publisher {
            eligible: false,
            announced: Arc::clone(&announced),
        };
        let outcome =
            BootstrapController::new(&empty, &native, &empty, &connector, &publisher, policy())
                .run_cycle(Namespace::default())
                .await
                .unwrap();
        assert!(!outcome.announced);
        assert!(!announced.load(Ordering::SeqCst));
    }
}
