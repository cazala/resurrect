use crate::{BootstrapError, DiscoverySource};
use async_trait::async_trait;
use resurrect_core::{
    Announcement, AnnouncementPolicy, CandidateStore, CandidateStoreConfig, CodecRegistry,
    DialContext, DiscoverySourceKind, Namespace, PeerCandidate,
};
use rusqlite::{Connection, params};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// SQLite-backed cache of previously verified signed peer records.
///
/// The database is deliberately disposable: every loaded row is
/// cryptographically revalidated before it becomes a dial candidate.
#[derive(Clone, Debug)]
pub struct SqlitePeerCache {
    path: PathBuf,
    codecs: Arc<CodecRegistry>,
    accepted_record_types: Arc<[u32]>,
    dial_context: DialContext,
    max_candidates: usize,
}

impl SqlitePeerCache {
    /// Opens or creates a cache and applies its idempotent schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory or `SQLite` database cannot be
    /// created.
    pub async fn open(
        path: impl AsRef<Path>,
        codecs: Arc<CodecRegistry>,
        accepted_record_types: Vec<u32>,
        dial_context: DialContext,
        max_candidates: usize,
    ) -> Result<Self, CacheError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let schema_path = path.clone();
        tokio::task::spawn_blocking(move || initialize(&schema_path)).await??;
        Ok(Self {
            path,
            codecs,
            accepted_record_types: accepted_record_types.into(),
            dial_context,
            max_candidates: max_candidates.max(1),
        })
    }

    /// Inserts only records that have already passed announcement validation.
    /// Newer peer-record sequence numbers replace older cached records.
    ///
    /// # Errors
    ///
    /// Returns an error on serialization, numeric overflow, or `SQLite` failure.
    pub async fn store_verified(
        &self,
        namespace: Namespace,
        candidates: &[PeerCandidate],
    ) -> Result<(), CacheError> {
        let path = self.path.clone();
        let namespace = *namespace.as_bytes();
        let candidates = candidates.to_vec();
        tokio::task::spawn_blocking(move || store(&path, namespace, &candidates)).await??;
        Ok(())
    }

    /// Removes expired records and returns the remaining row count.
    ///
    /// # Errors
    ///
    /// Returns an error if time conversion or `SQLite` cleanup fails.
    pub async fn prune_expired(&self) -> Result<usize, CacheError> {
        let path = self.path.clone();
        let now = unix_time()?;
        tokio::task::spawn_blocking(move || prune(&path, now)).await?
    }

    async fn load(&self, namespace: Namespace) -> Result<Vec<PeerCandidate>, CacheError> {
        let path = self.path.clone();
        let raw = *namespace.as_bytes();
        let rows = tokio::task::spawn_blocking(move || load_rows(&path, raw)).await??;
        let now = unix_time()?;
        let mut store = CandidateStore::new(CandidateStoreConfig {
            max_candidates: self.max_candidates,
            sampling_seed: namespace.as_b256(),
        });
        for row in rows {
            let announcement = Announcement {
                namespace,
                record_type: row.record_type,
                valid_until: row.expires_at,
                peer_record: row.raw_signed_record,
                block_number: row.block_number.unwrap_or_default(),
                log_index: row.log_index.unwrap_or_default(),
                block_hash: None,
            };
            let policy = AnnouncementPolicy {
                expected_namespace: namespace,
                accepted_record_types: &self.accepted_record_types,
                chain_time: now,
                max_record_bytes: resurrect_core::DEFAULT_MAX_RECORD_BYTES,
                max_endpoints: resurrect_core::DEFAULT_MAX_ENDPOINTS_PER_RECORD,
                dial_context: self.dial_context,
            };
            if let Ok(mut candidate) = self
                .codecs
                .validate_announcement(&announcement, policy)
                .await
            {
                candidate.source = DiscoverySourceKind::LocalCache;
                store.insert(candidate);
            }
        }
        Ok(store.ranked().into_iter().cloned().collect())
    }
}

#[async_trait]
impl DiscoverySource for SqlitePeerCache {
    async fn discover(&self, namespace: Namespace) -> Result<Vec<PeerCandidate>, BootstrapError> {
        self.load(namespace)
            .await
            .map_err(|error| BootstrapError::Discovery(error.to_string()))
    }
}

#[derive(Debug)]
struct CachedRow {
    record_type: u32,
    raw_signed_record: Vec<u8>,
    expires_at: u64,
    block_number: Option<u64>,
    log_index: Option<u64>,
}

fn initialize(path: &Path) -> Result<(), CacheError> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS verified_peers (
           namespace BLOB NOT NULL CHECK(length(namespace) = 32),
           record_type INTEGER NOT NULL,
           peer_id BLOB NOT NULL,
           sequence INTEGER NOT NULL,
           raw_signed_record BLOB NOT NULL,
           expires_at INTEGER NOT NULL,
           announcement_block INTEGER,
           announcement_log_index INTEGER,
           PRIMARY KEY(namespace, record_type, peer_id)
         );
         CREATE INDEX IF NOT EXISTS verified_peers_expiry
           ON verified_peers(expires_at);",
    )?;
    Ok(())
}

fn store(path: &Path, namespace: [u8; 32], candidates: &[PeerCandidate]) -> Result<(), CacheError> {
    let mut connection = Connection::open(path)?;
    let transaction = connection.transaction()?;
    for candidate in candidates {
        let sequence = to_i64(candidate.sequence, "sequence")?;
        let expires_at = to_i64(candidate.expires_at, "expires_at")?;
        let block = candidate
            .announcement_block
            .map(|value| to_i64(value, "announcement_block"))
            .transpose()?;
        let log_index = candidate
            .announcement_log_index
            .map(|value| to_i64(value, "announcement_log_index"))
            .transpose()?;
        transaction.execute(
            "INSERT INTO verified_peers (
               namespace, record_type, peer_id, sequence, raw_signed_record,
               expires_at, announcement_block, announcement_log_index
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(namespace, record_type, peer_id) DO UPDATE SET
               sequence = excluded.sequence,
               raw_signed_record = excluded.raw_signed_record,
               expires_at = excluded.expires_at,
               announcement_block = excluded.announcement_block,
               announcement_log_index = excluded.announcement_log_index
             WHERE excluded.sequence > verified_peers.sequence
                OR (excluded.sequence = verified_peers.sequence
                    AND COALESCE(excluded.announcement_block, 0) >=
                        COALESCE(verified_peers.announcement_block, 0))",
            params![
                namespace.as_slice(),
                i64::from(candidate.record_type),
                &candidate.peer_id,
                sequence,
                &candidate.raw_signed_record,
                expires_at,
                block,
                log_index,
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn load_rows(path: &Path, namespace: [u8; 32]) -> Result<Vec<CachedRow>, CacheError> {
    let connection = Connection::open(path)?;
    let mut statement = connection.prepare(
        "SELECT record_type, raw_signed_record, expires_at,
                announcement_block, announcement_log_index
         FROM verified_peers
         WHERE namespace = ?1
         ORDER BY sequence DESC, announcement_block DESC",
    )?;
    let rows = statement.query_map([namespace.as_slice()], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (record_type, raw_signed_record, expires_at, block_number, log_index) = row?;
        Ok(CachedRow {
            record_type: u32::try_from(record_type)
                .map_err(|_| CacheError::InvalidRow("record_type"))?,
            raw_signed_record,
            expires_at: u64::try_from(expires_at)
                .map_err(|_| CacheError::InvalidRow("expires_at"))?,
            block_number: optional_u64(block_number, "announcement_block")?,
            log_index: optional_u64(log_index, "announcement_log_index")?,
        })
    })
    .collect()
}

fn prune(path: &Path, now: u64) -> Result<usize, CacheError> {
    let connection = Connection::open(path)?;
    connection.execute(
        "DELETE FROM verified_peers WHERE expires_at <= ?1",
        [to_i64(now, "time")?],
    )?;
    let count: i64 =
        connection.query_row("SELECT COUNT(*) FROM verified_peers", [], |row| row.get(0))?;
    count
        .try_into()
        .map_err(|_| CacheError::InvalidRow("count"))
}

fn unix_time() -> Result<u64, CacheError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CacheError::Clock)?
        .as_secs())
}

fn to_i64(value: u64, field: &'static str) -> Result<i64, CacheError> {
    i64::try_from(value).map_err(|_| CacheError::NumericOverflow(field))
}

fn optional_u64(value: Option<i64>, field: &'static str) -> Result<Option<u64>, CacheError> {
    value
        .map(|value| u64::try_from(value).map_err(|_| CacheError::InvalidRow(field)))
        .transpose()
}

/// Persistent-cache failures.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Filesystem operation failed.
    #[error("cache filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// `SQLite` operation failed.
    #[error("cache SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Blocking database task could not complete.
    #[error("cache task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    /// System clock predates the Unix epoch.
    #[error("system clock predates the Unix epoch")]
    Clock,
    /// A protocol number cannot fit `SQLite`'s signed integer type.
    #[error("cache field {0} exceeds SQLite integer range")]
    NumericOverflow(&'static str),
    /// A stored row violates protocol numeric bounds.
    #[error("cache row contains invalid {0}")]
    InvalidRow(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use resurrect_core::{Endpoint, RECORD_TYPE_LIBP2P};
    use resurrect_libp2p::{
        EndpointPolicy, Keypair, Libp2pPeerRecordCodec, Multiaddr, sign_peer_record,
    };

    fn codecs() -> Arc<CodecRegistry> {
        let mut codecs = CodecRegistry::new();
        codecs.register(Arc::new(Libp2pPeerRecordCodec::new(
            EndpointPolicy::local_testing(),
            8,
        )));
        Arc::new(codecs)
    }

    #[tokio::test]
    async fn persists_and_revalidates_signed_records() {
        let directory = tempfile::tempdir().unwrap();
        let cache = SqlitePeerCache::open(
            directory.path().join("peers.sqlite3"),
            codecs(),
            vec![RECORD_TYPE_LIBP2P],
            DialContext::NativeServer,
            16,
        )
        .await
        .unwrap();
        let key = Keypair::generate_ed25519();
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();
        let signed = sign_peer_record(&key, 7, std::slice::from_ref(&address)).unwrap();
        let now = unix_time().unwrap();
        let candidate = PeerCandidate {
            record_type: RECORD_TYPE_LIBP2P,
            peer_id: key.public().to_peer_id().to_bytes(),
            sequence: 7,
            endpoints: vec![Endpoint {
                address: address.to_string(),
            }],
            raw_signed_record: signed,
            expires_at: now + 600,
            source: DiscoverySourceKind::ResurrectRegistry,
            announcement_block: Some(12),
            announcement_log_index: Some(1),
        };
        let namespace = Namespace::derive("cache-test", 1);
        cache.store_verified(namespace, &[candidate]).await.unwrap();

        let loaded = cache.discover(namespace).await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].sequence, 7);
        assert_eq!(loaded[0].source, DiscoverySourceKind::LocalCache);
    }

    #[tokio::test]
    async fn rejects_tampered_and_expired_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("peers.sqlite3");
        let cache = SqlitePeerCache::open(
            &path,
            codecs(),
            vec![RECORD_TYPE_LIBP2P],
            DialContext::NativeServer,
            16,
        )
        .await
        .unwrap();
        let namespace = Namespace::derive("cache-test", 1);
        let connection = Connection::open(path).unwrap();
        connection
            .execute(
                "INSERT INTO verified_peers VALUES (?1, 2, x'01', 1, x'00', 1, 1, 1)",
                [namespace.as_bytes().as_slice()],
            )
            .unwrap();
        assert!(cache.discover(namespace).await.unwrap().is_empty());
        assert_eq!(cache.prune_expired().await.unwrap(), 0);
    }
}
