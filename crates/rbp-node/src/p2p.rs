use crate::{BootstrapError, DiscoverySource, NativeDiscovery, PeerConnector};
use async_trait::async_trait;
use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder, identify, mdns,
    multiaddr::Protocol,
    noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent, behaviour::toggle::Toggle, dial_opts::DialOpts},
    tcp, yamux,
};
use rbp_core::{DiscoverySourceKind, Endpoint, Namespace, PeerCandidate, RECORD_TYPE_LIBP2P};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};

/// Native libp2p host settings.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Local listen multiaddrs.
    pub listen_addresses: Vec<Multiaddr>,
    /// Statically configured native peers, each ending in `/p2p/<peer-id>`.
    pub configured_peers: Vec<Multiaddr>,
    /// Enables local-network native discovery.
    pub enable_mdns: bool,
    /// Idle authenticated connection lifetime.
    pub idle_connection_timeout: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/4001"
                    .parse()
                    .expect("static multiaddr is valid"),
            ],
            configured_peers: Vec::new(),
            enable_mdns: true,
            idle_connection_timeout: Duration::from_secs(120),
        }
    }
}

/// Observable native host status.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// Local libp2p identity.
    pub peer_id: String,
    /// Active listener multiaddrs.
    pub listen_addresses: Vec<String>,
    /// Authenticated connected peer IDs.
    pub connected_peers: Vec<String>,
    /// Native-discovered peer count.
    pub native_discovered_peers: usize,
}

/// Running native host task and cloneable command handle.
#[derive(Debug)]
pub struct Libp2pHost {
    /// Command/discovery handle used by bootstrap adapters.
    pub handle: Libp2pHostHandle,
    task: tokio::task::JoinHandle<()>,
}

impl Libp2pHost {
    /// Starts the rust-libp2p event loop and requested listeners.
    ///
    /// # Errors
    ///
    /// Returns an error if behaviour construction or every listener fails.
    pub fn start(
        keypair: libp2p::identity::Keypair,
        config: HostConfig,
    ) -> Result<Self, HostError> {
        let configured_peers = config
            .configured_peers
            .into_iter()
            .map(split_configured_peer)
            .collect::<Result<Vec<_>, _>>()?;
        let local_peer = keypair.public().to_peer_id();
        let mdns_enabled = config.enable_mdns;
        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|error| HostError::Build(error.to_string()))?
            .with_dns()
            .map_err(|error| HostError::Build(error.to_string()))?
            .with_behaviour(move |key| {
                let mdns = if mdns_enabled {
                    Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?)
                } else {
                    None
                };
                Ok(Behaviour {
                    ping: ping::Behaviour::default(),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/rbp/1.0.0".to_owned(),
                        key.public(),
                    )),
                    mdns: Toggle::from(mdns),
                })
            })
            .map_err(|error| HostError::Build(error.to_string()))?
            .with_swarm_config(|configuration| {
                configuration.with_idle_connection_timeout(config.idle_connection_timeout)
            })
            .build();

        let mut listening = 0_usize;
        for address in config.listen_addresses {
            match swarm.listen_on(address) {
                Ok(_) => listening += 1,
                Err(error) => tracing::warn!(%error, "libp2p listener rejected"),
            }
        }
        if listening == 0 {
            return Err(HostError::NoListener);
        }

        let (commands, command_rx) = mpsc::channel(64);
        let initial = HostStatus {
            peer_id: local_peer.to_string(),
            ..HostStatus::default()
        };
        let (status_tx, status) = watch::channel(initial);
        let handle = Libp2pHostHandle { commands, status };
        let task =
            tokio::spawn(EventLoop::new(swarm, command_rx, status_tx, configured_peers).run());
        Ok(Self { handle, task })
    }

    /// Gracefully stops the host event loop.
    pub async fn shutdown(self) {
        self.handle.shutdown().await;
        let _ = self.task.await;
    }
}

/// Cloneable bootstrap/native-discovery adapter for a running host.
#[derive(Clone, Debug)]
pub struct Libp2pHostHandle {
    commands: mpsc::Sender<Command>,
    status: watch::Receiver<HostStatus>,
}

impl Libp2pHostHandle {
    /// Returns the latest lock-free host snapshot.
    #[must_use]
    pub fn status(&self) -> HostStatus {
        self.status.borrow().clone()
    }

    /// Waits until a listener is active or the timeout expires.
    ///
    /// # Errors
    ///
    /// Returns an error if the host stops or no listener event arrives in time.
    pub async fn wait_for_listener(&mut self, timeout: Duration) -> Result<HostStatus, HostError> {
        tokio::time::timeout(timeout, async {
            loop {
                let status = self.status();
                if !status.listen_addresses.is_empty() {
                    return Ok(status);
                }
                self.status
                    .changed()
                    .await
                    .map_err(|_| HostError::Stopped)?;
            }
        })
        .await
        .map_err(|_| HostError::ListenTimeout)?
    }

    /// Adds a verified record to the host's native peer store.
    ///
    /// # Errors
    ///
    /// Returns an error if the host event loop has stopped.
    pub async fn add_verified(&self, peer: PeerCandidate) -> Result<(), HostError> {
        self.commands
            .send(Command::AddVerified(peer))
            .await
            .map_err(|_| HostError::Stopped)
    }

    /// Returns the latest native-discovered candidates.
    ///
    /// # Errors
    ///
    /// Returns an error if the host event loop has stopped.
    pub async fn native_candidates(&self) -> Result<Vec<PeerCandidate>, HostError> {
        let (sender, receiver) = oneshot::channel();
        self.commands
            .send(Command::NativeCandidates(sender))
            .await
            .map_err(|_| HostError::Stopped)?;
        receiver.await.map_err(|_| HostError::Stopped)
    }

    async fn shutdown(&self) {
        let (sender, receiver) = oneshot::channel();
        if self.commands.send(Command::Shutdown(sender)).await.is_ok() {
            let _ = receiver.await;
        }
    }
}

#[async_trait]
impl PeerConnector for Libp2pHostHandle {
    async fn connected_peers(&self) -> usize {
        self.status().connected_peers.len()
    }

    async fn connect(&self, peer: PeerCandidate, timeout: Duration) -> bool {
        let (sender, receiver) = oneshot::channel();
        if self
            .commands
            .send(Command::Dial { peer, sender })
            .await
            .is_err()
        {
            return false;
        }
        if let Ok(dial_result) = tokio::time::timeout(timeout, receiver).await {
            dial_result.is_ok_and(|dial| dial.is_ok())
        } else {
            // The receiver has been dropped by the timeout. Promptly prune its
            // sender so a stalled transport cannot accumulate retry waiters.
            let _ = self.commands.send(Command::PruneDialWaiters).await;
            false
        }
    }
}

/// Native discovery view with a bounded observation interval.
#[derive(Clone, Debug)]
pub struct NativePeerSource {
    host: Libp2pHostHandle,
    observation_timeout: Duration,
}

impl NativePeerSource {
    /// Creates a source that lets mDNS/identify run for the given interval.
    #[must_use]
    pub const fn new(host: Libp2pHostHandle, observation_timeout: Duration) -> Self {
        Self {
            host,
            observation_timeout,
        }
    }
}

#[async_trait]
impl DiscoverySource for NativePeerSource {
    async fn discover(&self, _namespace: Namespace) -> Result<Vec<PeerCandidate>, BootstrapError> {
        tokio::time::sleep(self.observation_timeout).await;
        self.host
            .native_candidates()
            .await
            .map_err(|error| BootstrapError::Discovery(error.to_string()))
    }
}

#[async_trait]
impl NativeDiscovery for NativePeerSource {
    async fn add_verified_peer(&self, peer: PeerCandidate) -> Result<(), BootstrapError> {
        self.host
            .add_verified(peer)
            .await
            .map_err(|error| BootstrapError::NativePeerStore(error.to_string()))
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    mdns: Toggle<mdns::tokio::Behaviour>,
}

enum Command {
    Dial {
        peer: PeerCandidate,
        sender: oneshot::Sender<Result<(), HostError>>,
    },
    AddVerified(PeerCandidate),
    NativeCandidates(oneshot::Sender<Vec<PeerCandidate>>),
    PruneDialWaiters,
    Shutdown(oneshot::Sender<()>),
}

struct EventLoop {
    swarm: Swarm<Behaviour>,
    commands: mpsc::Receiver<Command>,
    status_tx: watch::Sender<HostStatus>,
    connected: HashSet<PeerId>,
    native: HashMap<PeerId, HashSet<Multiaddr>>,
    verified: HashMap<PeerId, PeerCandidate>,
    pending: HashMap<PeerId, Vec<oneshot::Sender<Result<(), HostError>>>>,
    listen_addresses: HashSet<Multiaddr>,
}

impl EventLoop {
    fn new(
        swarm: Swarm<Behaviour>,
        commands: mpsc::Receiver<Command>,
        status_tx: watch::Sender<HostStatus>,
        configured_peers: Vec<(PeerId, Multiaddr)>,
    ) -> Self {
        let mut native = HashMap::<PeerId, HashSet<Multiaddr>>::new();
        for (peer_id, address) in configured_peers {
            native.entry(peer_id).or_default().insert(address);
        }
        Self {
            swarm,
            commands,
            status_tx,
            connected: HashSet::new(),
            native,
            verified: HashMap::new(),
            pending: HashMap::new(),
            listen_addresses: HashSet::new(),
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                swarm_event = self.swarm.select_next_some() => self.handle_swarm(swarm_event),
                command = self.commands.recv() => match command {
                    Some(Command::Shutdown(sender)) => {
                        let _ = sender.send(());
                        break;
                    }
                    Some(command) => self.handle_command(command),
                    None => break,
                }
            }
        }
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::Dial { peer, sender } => self.dial(&peer, sender),
            Command::AddVerified(peer) => {
                if let Ok(peer_id) = PeerId::from_bytes(&peer.peer_id) {
                    self.verified.insert(peer_id, peer);
                }
                self.publish_status();
            }
            Command::NativeCandidates(sender) => {
                let _ = sender.send(self.native_candidates());
            }
            Command::PruneDialWaiters => {
                self.pending.retain(|_, waiters| {
                    waiters.retain(|waiter| !waiter.is_closed());
                    !waiters.is_empty()
                });
            }
            Command::Shutdown(_) => unreachable!("shutdown is handled in the run loop"),
        }
    }

    fn dial(&mut self, peer: &PeerCandidate, sender: oneshot::Sender<Result<(), HostError>>) {
        if peer.record_type != RECORD_TYPE_LIBP2P {
            let _ = sender.send(Err(HostError::UnsupportedRecordType(peer.record_type)));
            return;
        }
        let Ok(peer_id) = PeerId::from_bytes(&peer.peer_id) else {
            let _ = sender.send(Err(HostError::InvalidPeerId));
            return;
        };
        if self.connected.contains(&peer_id) {
            let _ = sender.send(Ok(()));
            return;
        }
        let addresses: Vec<_> = peer
            .endpoints
            .iter()
            .filter_map(|endpoint| endpoint.address.parse().ok())
            .collect();
        if addresses.is_empty() {
            let _ = sender.send(Err(HostError::NoDialableEndpoint));
            return;
        }
        let options = DialOpts::peer_id(peer_id).addresses(addresses).build();
        match self.swarm.dial(options) {
            Ok(()) => self.pending.entry(peer_id).or_default().push(sender),
            Err(error) => {
                let _ = sender.send(Err(HostError::Dial(error.to_string())));
            }
        }
    }

    fn handle_swarm(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                self.listen_addresses.insert(address);
                self.publish_status();
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                self.listen_addresses.remove(&address);
                self.publish_status();
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.connected.insert(peer_id);
                // Publish connectivity before completing dial waiters. The
                // bootstrap controller reads this snapshot immediately after a
                // successful dial to attribute the connection to its source.
                self.publish_status();
                if let Some(waiters) = self.pending.remove(&peer_id) {
                    for waiter in waiters {
                        let _ = waiter.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established: 0,
                ..
            } => {
                self.connected.remove(&peer_id);
                self.publish_status();
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer_id),
                error,
                ..
            } => {
                if let Some(waiters) = self.pending.remove(&peer_id) {
                    let message = error.to_string();
                    for waiter in waiters {
                        let _ = waiter.send(Err(HostError::Dial(message.clone())));
                    }
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                for (peer, address) in peers {
                    self.native.entry(peer).or_default().insert(address);
                }
                self.publish_status();
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                for (peer, address) in peers {
                    if let Some(addresses) = self.native.get_mut(&peer) {
                        addresses.remove(&address);
                        if addresses.is_empty() {
                            self.native.remove(&peer);
                        }
                    }
                }
                self.publish_status();
            }
            SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                self.native
                    .entry(peer_id)
                    .or_default()
                    .extend(info.listen_addrs);
                self.publish_status();
            }
            _ => {}
        }
    }

    fn native_candidates(&self) -> Vec<PeerCandidate> {
        self.native
            .iter()
            .map(|(peer, addresses)| PeerCandidate {
                record_type: RECORD_TYPE_LIBP2P,
                peer_id: peer.to_bytes(),
                sequence: 0,
                endpoints: addresses
                    .iter()
                    .map(|address| Endpoint {
                        address: address.to_string(),
                    })
                    .collect(),
                raw_signed_record: Vec::new(),
                expires_at: u64::MAX,
                source: DiscoverySourceKind::Native,
                announcement_block: None,
                announcement_log_index: None,
            })
            .collect()
    }

    fn publish_status(&self) {
        let mut connected_peers: Vec<_> = self.connected.iter().map(ToString::to_string).collect();
        connected_peers.sort();
        let mut listen_addresses: Vec<_> = self
            .listen_addresses
            .iter()
            .map(ToString::to_string)
            .collect();
        listen_addresses.sort();
        self.status_tx.send_replace(HostStatus {
            peer_id: self.swarm.local_peer_id().to_string(),
            listen_addresses,
            connected_peers,
            native_discovered_peers: self.native.len(),
        });
    }
}

fn split_configured_peer(mut address: Multiaddr) -> Result<(PeerId, Multiaddr), HostError> {
    let original = address.clone();
    match address.pop() {
        Some(Protocol::P2p(peer_id)) if !address.is_empty() => Ok((peer_id, address)),
        _ => Err(HostError::InvalidConfiguredPeer(original.to_string())),
    }
}

/// Native host construction and command failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostError {
    /// Transport/behaviour builder failure.
    #[error("failed to build libp2p host: {0}")]
    Build(String),
    /// No configured listener could be started.
    #[error("no libp2p listener could be started")]
    NoListener,
    /// A configured native peer omitted its terminal authenticated peer ID.
    #[error("configured peer must end in /p2p/<peer-id>: {0}")]
    InvalidConfiguredPeer(String),
    /// Listener event did not arrive in time.
    #[error("timed out waiting for libp2p listener")]
    ListenTimeout,
    /// Event loop stopped.
    #[error("libp2p host stopped")]
    Stopped,
    /// Candidate codec cannot be mapped to a libp2p peer ID.
    #[error("record type {0} cannot be dialed by the libp2p host")]
    UnsupportedRecordType(u32),
    /// Candidate contains malformed peer ID bytes.
    #[error("candidate contains an invalid libp2p peer ID")]
    InvalidPeerId,
    /// No endpoint was syntactically dialable.
    #[error("candidate has no dialable libp2p endpoint")]
    NoDialableEndpoint,
    /// Dial or authenticated handshake failure.
    #[error("libp2p dial failed: {0}")]
    Dial(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    fn local_config() -> HostConfig {
        HostConfig {
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
            configured_peers: Vec::new(),
            enable_mdns: false,
            idle_connection_timeout: Duration::from_secs(30),
        }
    }

    #[tokio::test]
    async fn two_hosts_complete_authenticated_dial() {
        let key_a = Keypair::generate_ed25519();
        let peer_a = key_a.public().to_peer_id();
        let mut host_a = Libp2pHost::start(key_a, local_config()).unwrap();
        let status_a = host_a
            .handle
            .wait_for_listener(Duration::from_secs(5))
            .await
            .unwrap();

        let key_b = Keypair::generate_ed25519();
        let mut host_b = Libp2pHost::start(key_b, local_config()).unwrap();
        host_b
            .handle
            .wait_for_listener(Duration::from_secs(5))
            .await
            .unwrap();

        let candidate = PeerCandidate {
            record_type: RECORD_TYPE_LIBP2P,
            peer_id: peer_a.to_bytes(),
            sequence: 1,
            endpoints: vec![Endpoint {
                address: status_a.listen_addresses[0].clone(),
            }],
            raw_signed_record: vec![1],
            expires_at: u64::MAX,
            source: DiscoverySourceKind::RbpRegistry,
            announcement_block: None,
            announcement_log_index: None,
        };
        assert!(
            host_b
                .handle
                .connect(candidate, Duration::from_secs(5))
                .await
        );
        assert_eq!(host_b.handle.status().connected_peers.len(), 1);

        host_b.shutdown().await;
        host_a.shutdown().await;
    }

    #[tokio::test]
    async fn configured_peer_is_a_native_candidate() {
        let key_a = Keypair::generate_ed25519();
        let peer_a = key_a.public().to_peer_id();
        let mut host_a = Libp2pHost::start(key_a, local_config()).unwrap();
        let status_a = host_a
            .handle
            .wait_for_listener(Duration::from_secs(5))
            .await
            .unwrap();

        let mut config_b = local_config();
        config_b.configured_peers.push(
            status_a.listen_addresses[0]
                .parse::<Multiaddr>()
                .unwrap()
                .with(Protocol::P2p(peer_a)),
        );
        let key_b = Keypair::generate_ed25519();
        let mut host_b = Libp2pHost::start(key_b, config_b).unwrap();
        host_b
            .handle
            .wait_for_listener(Duration::from_secs(5))
            .await
            .unwrap();

        let candidates = host_b.handle.native_candidates().await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].peer_id, peer_a.to_bytes());
        assert_eq!(candidates[0].source, DiscoverySourceKind::Native);
        assert!(
            host_b
                .handle
                .connect(candidates[0].clone(), Duration::from_secs(5))
                .await
        );

        host_b.shutdown().await;
        host_a.shutdown().await;
    }

    #[test]
    fn configured_peer_requires_terminal_peer_id() {
        let error = split_configured_peer("/ip4/127.0.0.1/tcp/4001".parse().unwrap())
            .expect_err("peer ID is required");
        assert!(matches!(error, HostError::InvalidConfiguredPeer(_)));
    }
}
