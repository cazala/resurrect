use crate::{MAX_TTL_SECONDS, Namespace};
use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// The only protocol version implemented by this crate.
pub const RBP_VERSION: u32 = 1;

/// Location and immutable limits of an RBP registry deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryDescriptor {
    /// EIP-155 chain identifier containing the registry.
    pub chain_id: u64,
    /// Registry contract address.
    pub address: Address,
    /// Earliest block that can contain registry announcements.
    pub deployment_block: u64,
    /// Registry `MAX_TTL` in seconds.
    pub max_ttl_seconds: u32,
}

/// Complete application configuration required to discover RBP peers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkDescriptor {
    /// RBP protocol version. Must be `1`.
    pub rbp_version: u32,
    /// Registry deployment parameters.
    pub registry: RegistryDescriptor,
    /// Application/network isolation identifier.
    pub namespace: Namespace,
    /// Signed peer-record codec identifiers accepted by the application.
    pub accepted_record_types: Vec<u32>,
}

impl NetworkDescriptor {
    /// Parses and validates a descriptor from JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON, unknown fields, or an invalid v1
    /// descriptor invariant.
    pub fn from_json(json: &str) -> Result<Self, DescriptorError> {
        let descriptor: Self = serde_json::from_str(json)?;
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Validates all cross-field RBP v1 invariants.
    ///
    /// # Errors
    ///
    /// Returns the first version, TTL, or codec-list invariant violation.
    pub fn validate(&self) -> Result<(), DescriptorError> {
        if self.rbp_version != RBP_VERSION {
            return Err(DescriptorError::UnsupportedVersion(self.rbp_version));
        }
        if self.registry.max_ttl_seconds != MAX_TTL_SECONDS {
            return Err(DescriptorError::UnexpectedMaxTtl {
                expected: MAX_TTL_SECONDS,
                actual: self.registry.max_ttl_seconds,
            });
        }
        if self.accepted_record_types.is_empty() {
            return Err(DescriptorError::NoAcceptedRecordTypes);
        }

        let mut unique = HashSet::with_capacity(self.accepted_record_types.len());
        for record_type in &self.accepted_record_types {
            if !unique.insert(*record_type) {
                return Err(DescriptorError::DuplicateRecordType(*record_type));
            }
        }
        Ok(())
    }

    /// Fails closed if a provider is connected to a different chain.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::WrongChain`] when the chain IDs differ.
    pub fn verify_chain_id(&self, actual: u64) -> Result<(), DescriptorError> {
        if actual != self.registry.chain_id {
            return Err(DescriptorError::WrongChain {
                expected: self.registry.chain_id,
                actual,
            });
        }
        Ok(())
    }

    /// Returns deterministic minified JSON with sorted record types.
    ///
    /// # Errors
    ///
    /// Returns an error if the descriptor is invalid or serialization fails.
    pub fn canonical_json(&self) -> Result<String, DescriptorError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.accepted_record_types.sort_unstable();
        Ok(serde_json::to_string(&normalized)?)
    }
}

/// Network descriptor errors.
#[derive(Debug, Error)]
pub enum DescriptorError {
    /// The JSON representation is malformed or contains unknown fields.
    #[error("invalid network descriptor JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Only version 1 is supported.
    #[error("unsupported RBP version {0}; expected 1")]
    UnsupportedVersion(u32),
    /// The embedded limit does not match the canonical v1 contract.
    #[error("descriptor max TTL is {actual}, expected {expected}")]
    UnexpectedMaxTtl {
        /// Required protocol value.
        expected: u32,
        /// Descriptor value.
        actual: u32,
    },
    /// At least one cryptographically verified codec is required.
    #[error("acceptedRecordTypes must contain at least one codec")]
    NoAcceptedRecordTypes,
    /// Duplicate codec IDs make normalization ambiguous.
    #[error("record type {0} appears more than once")]
    DuplicateRecordType(u32),
    /// The provider is connected to a different chain.
    #[error("provider chain ID {actual} does not match descriptor chain ID {expected}")]
    WrongChain {
        /// Configured chain ID.
        expected: u64,
        /// Provider chain ID.
        actual: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn descriptor() -> NetworkDescriptor {
        NetworkDescriptor {
            rbp_version: 1,
            registry: RegistryDescriptor {
                chain_id: 1,
                address: Address::from_str("0x1111111111111111111111111111111111111111").unwrap(),
                deployment_block: 42,
                max_ttl_seconds: MAX_TTL_SECONDS,
            },
            namespace: Namespace::derive("test", 1),
            accepted_record_types: vec![2, 1],
        }
    }

    #[test]
    fn descriptor_round_trip_and_normalization() {
        let descriptor = descriptor();
        let json = descriptor.canonical_json().unwrap();
        assert!(json.contains("\"acceptedRecordTypes\":[1,2]"));
        assert_eq!(
            NetworkDescriptor::from_json(&json)
                .unwrap()
                .registry
                .chain_id,
            1
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = descriptor().canonical_json().unwrap();
        let invalid = json.replacen('{', "{\"rpcUrl\":\"https://central.invalid\",", 1);
        assert!(matches!(
            NetworkDescriptor::from_json(&invalid),
            Err(DescriptorError::Json(_))
        ));
    }

    #[test]
    fn validates_version_ttl_codecs_and_chain() {
        let mut value = descriptor();
        value.rbp_version = 2;
        assert!(matches!(
            value.validate(),
            Err(DescriptorError::UnsupportedVersion(2))
        ));

        value = descriptor();
        value.registry.max_ttl_seconds = 1;
        assert!(matches!(
            value.validate(),
            Err(DescriptorError::UnexpectedMaxTtl { .. })
        ));

        value = descriptor();
        value.accepted_record_types = vec![2, 2];
        assert!(matches!(
            value.validate(),
            Err(DescriptorError::DuplicateRecordType(2))
        ));

        assert!(matches!(
            descriptor().verify_chain_id(10),
            Err(DescriptorError::WrongChain { .. })
        ));
    }
}
