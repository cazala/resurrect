use libp2p::identity::Keypair;
use std::{fs, io, path::Path};
use thiserror::Error;

/// Loads a protobuf-encoded libp2p identity or atomically creates one.
///
/// The identity file is created with owner-only permissions on Unix. An absent
/// parent directory is created automatically.
///
/// # Errors
///
/// Returns an error for filesystem failures or malformed identity bytes.
pub fn load_or_create_identity(path: &Path) -> Result<Keypair, IdentityError> {
    match fs::read(path) {
        Ok(bytes) => return Keypair::from_protobuf_encoding(&bytes).map_err(IdentityError::Decode),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let keypair = Keypair::generate_ed25519();
    let encoded = keypair
        .to_protobuf_encoding()
        .map_err(IdentityError::Decode)?;
    write_private_file(path, &encoded)?;
    Ok(keypair)
}

#[cfg(unix)]
fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    let mut file = options.open(path)?;
    io::Write::write_all(&mut file, bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path)?;
    io::Write::write_all(&mut file, bytes)?;
    file.sync_all()
}

/// Persistent identity failures.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// Filesystem failure.
    #[error("identity file error: {0}")]
    Io(#[from] io::Error),
    /// Invalid protobuf key encoding.
    #[error("invalid libp2p identity: {0}")]
    Decode(libp2p::identity::DecodingError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_stable_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity.key");
        let first = load_or_create_identity(&path).unwrap();
        let second = load_or_create_identity(&path).unwrap();
        assert_eq!(first.public().to_peer_id(), second.public().to_peer_id());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn rejects_corrupt_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity.key");
        fs::write(&path, b"not a key").unwrap();
        assert!(matches!(
            load_or_create_identity(&path),
            Err(IdentityError::Decode(_))
        ));
    }
}
