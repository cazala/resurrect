use alloy_primitives::{B256, keccak256};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// A 32-byte application/network isolation identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Namespace(B256);

impl Namespace {
    /// Derives the recommended namespace for an application protocol version.
    ///
    /// This is exactly `keccak256("rbp:" + application + ":" + major)`.
    #[must_use]
    pub fn derive(application_identifier: &str, major_protocol_version: u64) -> Self {
        let input = format!("rbp:{application_identifier}:{major_protocol_version}");
        Self(keccak256(input.as_bytes()))
    }

    /// Constructs a namespace from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(B256::new(bytes))
    }

    /// Returns the raw namespace bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_ref()
    }

    /// Returns the Alloy fixed-byte representation.
    #[must_use]
    pub const fn as_b256(self) -> B256 {
        self.0
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}", self.0)
    }
}

impl FromStr for Namespace {
    type Err = NamespaceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = B256::from_str(value).map_err(|_| NamespaceError::InvalidHex)?;
        Ok(Self(parsed))
    }
}

impl Serialize for Namespace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Namespace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

/// Namespace parsing errors.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NamespaceError {
    /// The value is not exactly 32 hex-encoded bytes.
    #[error("namespace must be a 0x-prefixed 32-byte hex value")]
    InvalidHex,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_stable_namespace() {
        assert_eq!(
            Namespace::derive("example-network", 1).to_string(),
            "0xf90b28e5c2deb8854a5a0cda7584edcca25b73bc5a45f456aaa33c1de303646e"
        );
    }

    #[test]
    fn serde_round_trip_uses_canonical_hex() {
        let namespace = Namespace::derive("rbp-test", 1);
        let json = serde_json::to_string(&namespace).unwrap();
        let decoded: Namespace = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, namespace);
        assert!(json.starts_with("\"0x"));
    }

    #[test]
    fn rejects_short_namespace() {
        assert_eq!("0x01".parse::<Namespace>(), Err(NamespaceError::InvalidHex));
    }
}
