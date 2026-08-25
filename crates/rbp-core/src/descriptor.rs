use crate::{MAX_TTL_SECONDS, Namespace};
use alloy_primitives::{Address, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use std::{collections::HashSet, fmt};
use thiserror::Error;

/// The only protocol version implemented by this crate.
pub const RBP_VERSION: u32 = 1;

/// Location and immutable limits of an RBP registry deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryDescriptor {
    /// EIP-155 chain identifier containing the registry.
    #[serde(with = "chain_id_serde")]
    pub chain_id: U256,
    /// Registry contract address.
    pub address: Address,
    /// Earliest block that can contain registry announcements.
    #[serde(with = "deployment_block_serde")]
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
    pub fn verify_chain_id(&self, actual: U256) -> Result<(), DescriptorError> {
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
        expected: U256,
        /// Provider chain ID.
        actual: U256,
    },
}

mod chain_id_serde {
    use super::{Deserializer, Serializer, U256, Visitor, fmt};

    const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

    pub fn serialize<S: Serializer>(chain_id: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        if let Ok(value) = u64::try_from(*chain_id)
            && value <= MAX_SAFE_JSON_INTEGER
        {
            return serializer.serialize_u64(value);
        }
        serializer.serialize_str(&chain_id.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<U256, D::Error> {
        deserializer.deserialize_any(ChainIdVisitor)
    }

    struct ChainIdVisitor;

    impl Visitor<'_> for ChainIdVisitor {
        type Value = U256;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an unsigned uint256 number or canonical decimal string")
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(U256::from(value))
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty()
                || (value.len() > 1 && value.starts_with('0'))
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(E::custom(
                    "chainId must be a canonical unsigned decimal integer",
                ));
            }
            value.parse::<U256>().map_err(E::custom)
        }
    }
}

mod deployment_block_serde {
    use super::{Deserializer, Serializer, Visitor, fmt};

    const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

    #[allow(clippy::trivially_copy_pass_by_ref)] // serde `with` requires a shared reference.
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        if *value <= MAX_SAFE_JSON_INTEGER {
            serializer.serialize_u64(*value)
        } else {
            serializer.serialize_str(&value.to_string())
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        deserializer.deserialize_any(DeploymentBlockVisitor)
    }

    struct DeploymentBlockVisitor;

    impl Visitor<'_> for DeploymentBlockVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an unsigned uint64 number or canonical decimal string")
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty()
                || (value.len() > 1 && value.starts_with('0'))
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(E::custom(
                    "deploymentBlock must be a canonical unsigned decimal integer",
                ));
            }
            value.parse::<u64>().map_err(E::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn descriptor() -> NetworkDescriptor {
        NetworkDescriptor {
            rbp_version: 1,
            registry: RegistryDescriptor {
                chain_id: U256::from(1),
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
            U256::from(1)
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
            descriptor().verify_chain_id(U256::from(10)),
            Err(DescriptorError::WrongChain { .. })
        ));
    }

    #[test]
    fn supports_full_width_chain_ids_without_lossy_json_numbers() {
        let mut value = descriptor();
        value.registry.chain_id = U256::MAX;
        let json = value.canonical_json().unwrap();
        assert!(json.contains(&format!("\"chainId\":\"{}\"", U256::MAX)));
        assert_eq!(
            NetworkDescriptor::from_json(&json)
                .unwrap()
                .registry
                .chain_id,
            U256::MAX
        );
        assert!(NetworkDescriptor::from_json(&json.replace(&U256::MAX.to_string(), "01")).is_err());
    }

    #[test]
    fn supports_full_width_deployment_blocks_without_lossy_json_numbers() {
        let mut value = descriptor();
        value.registry.deployment_block = u64::MAX;
        let json = value.canonical_json().unwrap();
        assert!(json.contains(&format!("\"deploymentBlock\":\"{}\"", u64::MAX)));
        assert_eq!(
            NetworkDescriptor::from_json(&json)
                .unwrap()
                .registry
                .deployment_block,
            u64::MAX
        );
        assert!(NetworkDescriptor::from_json(&json.replace(&u64::MAX.to_string(), "01")).is_err());
    }
}
