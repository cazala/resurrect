//! Native command-line process for the RBP v1 reference node.

use alloy::signers::local::PrivateKeySigner;
use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use libp2p::Multiaddr;
use rbp_core::{
    CodecRegistry, DEFAULT_MAX_ENDPOINTS_PER_RECORD, DialContext, NetworkDescriptor,
    RECORD_TYPE_ENR, RECORD_TYPE_LIBP2P,
};
use rbp_ethereum::{AlloyRegistryProvider, RegistryProvider, ScannerConfig};
use rbp_libp2p::{EndpointPolicy, EnrCodec, Libp2pPeerRecordCodec};
use rbp_node::{
    BootstrapPolicy, HostConfig, Libp2pHost, NativePeerSource, RegistryAnnouncer,
    RegistryDiscovery, RegistryTelemetry, SqlitePeerCache, Supervisor, SupervisorPolicy,
    load_or_create_identity,
};
use std::{path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use tracing_subscriber::EnvFilter;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogFormat {
    Pretty,
    Json,
}

/// Native RBP v1 reference seed/light node.
#[derive(Debug, Parser)]
#[command(version, about)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Canonical RBP network descriptor JSON file.
    #[arg(long)]
    descriptor: PathBuf,

    /// Caller-selected Ethereum JSON-RPC endpoint.
    #[arg(long, env = "RBP_RPC_URL")]
    rpc_url: String,

    /// Persistent protobuf-encoded libp2p identity.
    #[arg(long, default_value = "rbp-identity.key")]
    identity: PathBuf,

    /// Disposable verified-peer `SQLite` cache.
    #[arg(long, default_value = "rbp-peers.sqlite3")]
    cache: PathBuf,

    /// Native libp2p listener; repeat for multiple transports.
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/4001")]
    listen: Vec<String>,

    /// Signed reachable endpoint; required for seed operation.
    #[arg(long)]
    advertise: Vec<String>,

    /// Operate as a publicly reachable self-announcing seed.
    #[arg(long)]
    seed: bool,

    /// Local Ethereum signing key, preferably supplied through the environment.
    #[arg(long, env = "RBP_ETHEREUM_PRIVATE_KEY", hide_env_values = true)]
    ethereum_private_key: Option<String>,

    /// Enable mDNS as the native post-bootstrap discovery mechanism.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    mdns: bool,

    /// Configured native peer multiaddr ending in `/p2p/<peer-id>`; repeatable.
    #[arg(long)]
    native_peer: Vec<String>,

    /// Permit private/loopback signed endpoints (testing/private overlays only).
    #[arg(long)]
    allow_private_endpoints: bool,

    /// Minimum authenticated peers that constitute healthy connectivity.
    #[arg(long, default_value_t = 2)]
    minimum_peers: usize,

    /// Maximum concurrent candidate dials.
    #[arg(long, default_value_t = 8)]
    maximum_parallel_dials: usize,

    /// Per-peer handshake timeout in seconds.
    #[arg(long, default_value_t = 8)]
    dial_timeout_seconds: u64,

    /// Native discovery observation window in milliseconds.
    #[arg(long, default_value_t = 1500)]
    native_observation_millis: u64,

    /// Confirmations used when safe/finalized tags are unavailable.
    #[arg(long, default_value_t = 12)]
    fallback_confirmations: u64,

    /// Use latest-minus-confirmations even if stagnant finality tags exist.
    #[arg(long)]
    allow_unfinalized: bool,

    /// Isolated seed announcement TTL in seconds.
    #[arg(long, default_value_t = 604800)]
    reboot_ttl_seconds: u64,

    /// Healthy seed announcement TTL in seconds.
    #[arg(long, default_value_t = 2592000)]
    maintenance_ttl_seconds: u64,

    /// Healthy seed renewal interval in seconds.
    #[arg(long, default_value_t = 1209600)]
    renewal_interval_seconds: u64,

    /// Initial retry backoff in milliseconds.
    #[arg(long, default_value_t = 1000)]
    initial_backoff_millis: u64,

    /// Maximum retry backoff in seconds.
    #[arg(long, default_value_t = 300)]
    maximum_backoff_seconds: u64,

    /// Optional atomically replaced JSON status file.
    #[arg(long)]
    status_file: Option<PathBuf>,

    /// Structured logging representation.
    #[arg(long, value_enum, default_value_t = LogFormat::Pretty)]
    log_format: LogFormat,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    initialize_logging(cli.log_format);
    run(cli).await
}

#[allow(clippy::too_many_lines)]
async fn run(cli: Cli) -> Result<()> {
    let descriptor_json = tokio::fs::read_to_string(&cli.descriptor)
        .await
        .with_context(|| format!("could not read descriptor {}", cli.descriptor.display()))?;
    let descriptor = NetworkDescriptor::from_json(&descriptor_json)
        .context("network descriptor is not conforming RBP v1 JSON")?;
    validate_cli_policy(&cli, descriptor.registry.max_ttl_seconds)?;
    let endpoint_policy = if cli.allow_private_endpoints {
        EndpointPolicy::local_testing()
    } else {
        EndpointPolicy::default()
    };
    let codecs = build_codecs(&descriptor, &endpoint_policy)?;
    let identity = load_or_create_identity(&cli.identity)?;
    let peer_id = identity.public().to_peer_id();

    let provider: Arc<dyn RegistryProvider> = match cli.ethereum_private_key.as_deref() {
        Some(secret) => {
            let signer = PrivateKeySigner::from_str(secret)
                .context("RBP Ethereum signing key is invalid")?;
            Arc::new(AlloyRegistryProvider::with_signer(&cli.rpc_url, signer)?)
        }
        None => Arc::new(AlloyRegistryProvider::read_only(&cli.rpc_url)?),
    };
    if cli.seed && cli.ethereum_private_key.is_none() {
        bail!("--seed requires RBP_ETHEREUM_PRIVATE_KEY");
    }
    if cli.seed && cli.advertise.is_empty() {
        bail!("--seed requires at least one explicit --advertise endpoint");
    }

    let listen_addresses = parse_multiaddrs(&cli.listen, "listen")?;
    let configured_peers = parse_multiaddrs(&cli.native_peer, "native-peer")?;
    let advertised_addresses = parse_multiaddrs(&cli.advertise, "advertise")?;
    if advertised_addresses.len() > DEFAULT_MAX_ENDPOINTS_PER_RECORD {
        bail!("--advertise exceeds the signed peer-record endpoint cap");
    }
    for address in &advertised_addresses {
        if !endpoint_policy.accepts(address, Some(peer_id), DialContext::NativeServer) {
            bail!("advertised endpoint is rejected by the selected endpoint policy: {address}");
        }
    }
    let mut host = Libp2pHost::start(
        identity.clone(),
        HostConfig {
            listen_addresses,
            configured_peers,
            enable_mdns: cli.mdns,
            ..HostConfig::default()
        },
    )?;
    let listener_status = host
        .handle
        .wait_for_listener(Duration::from_secs(10))
        .await?;
    tracing::info!(
        peer_id = %peer_id,
        listeners = ?listener_status.listen_addresses,
        namespace = %descriptor.namespace,
        seed = cli.seed,
        "RBP native node started"
    );

    let cache = SqlitePeerCache::open(
        &cli.cache,
        Arc::clone(&codecs),
        descriptor.accepted_record_types.clone(),
        DialContext::NativeServer,
        rbp_core::DEFAULT_MAX_ACTIVE_CANDIDATES,
    )
    .await?;
    cache.prune_expired().await?;
    let native = NativePeerSource::new(
        host.handle.clone(),
        Duration::from_millis(cli.native_observation_millis),
    );
    let telemetry = Arc::new(RegistryTelemetry::default());
    let registry = RegistryDiscovery::new(
        Arc::clone(&provider),
        descriptor.clone(),
        Arc::clone(&codecs),
        ScannerConfig {
            fallback_confirmations: cli.fallback_confirmations,
            use_finality_tags: !cli.allow_unfinalized,
            dial_context: DialContext::NativeServer,
            ..ScannerConfig::default()
        },
        peer_id.to_bytes(),
        Some(cache.clone()),
        Arc::clone(&telemetry),
    );
    let announcer = RegistryAnnouncer::new(
        provider,
        descriptor.clone(),
        identity,
        advertised_addresses,
        cli.seed,
        Duration::from_secs(cli.renewal_interval_seconds),
        Arc::clone(&telemetry),
    );
    let supervisor = Supervisor::new(
        &cache,
        &native,
        &registry,
        &host.handle,
        &announcer,
        descriptor.namespace,
        peer_id.to_string(),
        SupervisorPolicy {
            bootstrap: BootstrapPolicy {
                minimum_successful_peers: cli.minimum_peers,
                maximum_parallel_dials: cli.maximum_parallel_dials,
                per_dial_timeout: Duration::from_secs(cli.dial_timeout_seconds),
                reboot_ttl: Duration::from_secs(cli.reboot_ttl_seconds),
            },
            initial_backoff: Duration::from_millis(cli.initial_backoff_millis),
            maximum_backoff: Duration::from_secs(cli.maximum_backoff_seconds),
            maintenance_ttl: Duration::from_secs(cli.maintenance_ttl_seconds),
            ..SupervisorPolicy::default()
        },
        telemetry,
        cli.status_file,
    );
    supervisor
        .run(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    host.shutdown().await;
    Ok(())
}

fn validate_cli_policy(cli: &Cli, maximum_ttl: u32) -> Result<()> {
    if cli.minimum_peers == 0 {
        bail!("--minimum-peers must be positive");
    }
    if cli.maximum_parallel_dials == 0 {
        bail!("--maximum-parallel-dials must be positive");
    }
    if cli.dial_timeout_seconds == 0 {
        bail!("--dial-timeout-seconds must be positive");
    }
    if cli.native_observation_millis == 0 {
        bail!("--native-observation-millis must be positive");
    }
    if cli.initial_backoff_millis == 0 || cli.maximum_backoff_seconds == 0 {
        bail!("retry backoff values must be positive");
    }
    if cli.renewal_interval_seconds == 0 {
        bail!("--renewal-interval-seconds must be positive");
    }
    for (name, ttl) in [
        ("--reboot-ttl-seconds", cli.reboot_ttl_seconds),
        ("--maintenance-ttl-seconds", cli.maintenance_ttl_seconds),
    ] {
        if ttl == 0 || ttl > u64::from(maximum_ttl) {
            bail!("{name} must be between 1 and the descriptor maximum TTL");
        }
    }
    Ok(())
}

fn build_codecs(
    descriptor: &NetworkDescriptor,
    endpoint_policy: &EndpointPolicy,
) -> Result<Arc<CodecRegistry>> {
    let mut codecs = CodecRegistry::new();
    for record_type in &descriptor.accepted_record_types {
        match *record_type {
            RECORD_TYPE_ENR => codecs.register(Arc::new(EnrCodec::new(endpoint_policy.clone()))),
            RECORD_TYPE_LIBP2P => codecs.register(Arc::new(Libp2pPeerRecordCodec::new(
                endpoint_policy.clone(),
                rbp_core::DEFAULT_MAX_ENDPOINTS_PER_RECORD,
            ))),
            unsupported => bail!("descriptor requests unsupported record type {unsupported}"),
        }
    }
    Ok(Arc::new(codecs))
}

fn parse_multiaddrs(values: &[String], field: &str) -> Result<Vec<Multiaddr>> {
    values
        .iter()
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("invalid {field} multiaddr: {value}"))
        })
        .collect()
}

fn initialize_logging(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match format {
        LogFormat::Pretty => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
    }
}
