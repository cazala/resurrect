use crate::{
    AnnouncementPublisher, BootstrapController, BootstrapPolicy, BootstrapState, DiscoverySource,
    NativeDiscovery, PeerConnector, RegistryTelemetry,
};
use rbp_core::Namespace;
use serde::Serialize;
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Long-running retry, renewal, and status policy.
#[derive(Clone, Debug)]
pub struct SupervisorPolicy {
    /// Bootstrap dial and isolated-promotion policy.
    pub bootstrap: BootstrapPolicy,
    /// Poll interval while healthy; no registry scan occurs during this wait.
    pub connected_poll_interval: Duration,
    /// Initial isolated retry delay.
    pub initial_backoff: Duration,
    /// Maximum isolated retry delay.
    pub maximum_backoff: Duration,
    /// Normal connected-seed announcement lifetime.
    pub maintenance_ttl: Duration,
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        Self {
            bootstrap: BootstrapPolicy::default(),
            connected_poll_interval: Duration::from_secs(15),
            initial_backoff: Duration::from_secs(5),
            maximum_backoff: Duration::from_secs(5 * 60),
            maintenance_ttl: Duration::from_secs(30 * 24 * 60 * 60),
        }
    }
}

/// Atomic JSON status-file representation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    /// Stable local libp2p peer identity.
    pub peer_id: String,
    /// Latest state-machine terminal state.
    pub state: BootstrapState,
    /// Current authenticated connection count.
    pub connected_peers: usize,
    /// Source responsible for the latest successful bootstrap.
    pub connected_via: Option<BootstrapState>,
    /// Ordered transitions from the latest cycle.
    pub transitions: Vec<BootstrapState>,
    /// Registry activity counters.
    pub registry: crate::TelemetrySnapshot,
    /// Most recent recoverable supervisor error.
    pub last_error: Option<String>,
    /// Unix timestamp of this snapshot.
    pub updated_at: u64,
}

/// Interface-driven long-running node supervisor.
#[derive(Debug)]
pub struct Supervisor<'a, C, N, R, D, A>
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
    namespace: Namespace,
    peer_id: String,
    policy: SupervisorPolicy,
    telemetry: Arc<RegistryTelemetry>,
    status_file: Option<PathBuf>,
}

impl<'a, C, N, R, D, A> Supervisor<'a, C, N, R, D, A>
where
    C: DiscoverySource + ?Sized,
    N: NativeDiscovery + ?Sized,
    R: DiscoverySource + ?Sized,
    D: PeerConnector + ?Sized,
    A: AnnouncementPublisher + ?Sized,
{
    /// Creates a supervisor over caller-owned bootstrap interfaces.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        cache: &'a C,
        native: &'a N,
        registry: &'a R,
        connector: &'a D,
        publisher: &'a A,
        namespace: Namespace,
        peer_id: String,
        policy: SupervisorPolicy,
        telemetry: Arc<RegistryTelemetry>,
        status_file: Option<PathBuf>,
    ) -> Self {
        Self {
            cache,
            native,
            registry,
            connector,
            publisher,
            namespace,
            peer_id,
            policy,
            telemetry,
            status_file,
        }
    }

    /// Runs until the supplied shutdown future resolves.
    ///
    /// # Errors
    ///
    /// Returns an error only when a configured status file cannot be written.
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> Result<(), SupervisorError> {
        tokio::pin!(shutdown);
        let mut attempts = 0_u32;
        let mut last_connected_via = None;
        loop {
            let connected = self.connector.connected_peers().await;
            if connected >= connection_target(&self.policy.bootstrap) {
                attempts = 0;
                let mut last_error = None;
                if self.publisher.renewal_due()
                    && let Err(error) = self.publisher.announce(self.policy.maintenance_ttl).await
                {
                    tracing::warn!(%error, "connected seed renewal failed");
                    last_error = Some(error.to_string());
                }
                self.publish_status(NodeStatus {
                    peer_id: self.peer_id.clone(),
                    state: BootstrapState::Connected,
                    connected_peers: connected,
                    connected_via: last_connected_via,
                    transitions: vec![BootstrapState::Connected],
                    registry: self.telemetry.snapshot(),
                    last_error,
                    updated_at: unix_time(),
                })
                .await?;
                if wait_or_shutdown(self.policy.connected_poll_interval, &mut shutdown).await {
                    return Ok(());
                }
                continue;
            }

            let controller = BootstrapController::new(
                self.cache,
                self.native,
                self.registry,
                self.connector,
                self.publisher,
                self.policy.bootstrap.clone(),
            );
            let (state, connected_via, transitions, last_error) =
                match controller.run_cycle(self.namespace).await {
                    Ok(outcome) => {
                        tracing::info!(
                            state = ?outcome.state,
                            connected_via = ?outcome.connected_via,
                            announced = outcome.announced,
                            transitions = ?outcome.transitions,
                            "bootstrap cycle completed"
                        );
                        if outcome.connected_via.is_some() {
                            last_connected_via = outcome.connected_via;
                        }
                        (
                            outcome.state,
                            outcome.connected_via,
                            outcome.transitions,
                            None,
                        )
                    }
                    Err(error) => {
                        tracing::warn!(%error, "bootstrap cycle failed and will retry");
                        (
                            BootstrapState::Backoff,
                            None,
                            vec![BootstrapState::Backoff],
                            Some(error.to_string()),
                        )
                    }
                };
            let connected = self.connector.connected_peers().await;
            self.publish_status(NodeStatus {
                peer_id: self.peer_id.clone(),
                state,
                connected_peers: connected,
                connected_via,
                transitions,
                registry: self.telemetry.snapshot(),
                last_error,
                updated_at: unix_time(),
            })
            .await?;
            if state == BootstrapState::Connected {
                attempts = 0;
                continue;
            }
            let delay = exponential_backoff(
                self.policy.initial_backoff,
                self.policy.maximum_backoff,
                attempts,
            );
            attempts = attempts.saturating_add(1);
            if wait_or_shutdown(delay, &mut shutdown).await {
                return Ok(());
            }
        }
    }

    async fn publish_status(&self, status: NodeStatus) -> Result<(), SupervisorError> {
        let Some(path) = &self.status_file else {
            return Ok(());
        };
        write_atomic_json(path, &status).await
    }
}

fn connection_target(policy: &BootstrapPolicy) -> usize {
    policy.minimum_successful_peers.max(1)
}

fn exponential_backoff(initial: Duration, maximum: Duration, attempt: u32) -> Duration {
    let factor = 1_u32.checked_shl(attempt.min(20)).unwrap_or(u32::MAX);
    initial.saturating_mul(factor).min(maximum)
}

async fn wait_or_shutdown<F>(duration: Duration, shutdown: &mut std::pin::Pin<&mut F>) -> bool
where
    F: Future<Output = ()>,
{
    tokio::select! {
        () = shutdown => true,
        () = tokio::time::sleep(duration) => false,
    }
}

async fn write_atomic_json(path: &Path, status: &NodeStatus) -> Result<(), SupervisorError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = path.with_extension("tmp");
    let mut bytes = serde_json::to_vec_pretty(status)?;
    bytes.push(b'\n');
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, path).await?;
    Ok(())
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Supervisor status persistence failure.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// Status filesystem operation failed.
    #[error("status file operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// Status serialization failed.
    #[error("status serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_and_capped() {
        let initial = Duration::from_secs(2);
        let maximum = Duration::from_secs(9);
        assert_eq!(exponential_backoff(initial, maximum, 0), initial);
        assert_eq!(
            exponential_backoff(initial, maximum, 2),
            Duration::from_secs(8)
        );
        assert_eq!(exponential_backoff(initial, maximum, 3), maximum);
        assert_eq!(exponential_backoff(initial, maximum, u32::MAX), maximum);
    }
}
