//! Ethereum registry access for Resurrect v1.
//!
//! [`RegistryProvider`] is deliberately provider-neutral so applications can
//! supply local nodes, authenticated transports, quorum providers, or test
//! doubles. [`AlloyRegistryProvider`] is the production HTTP implementation.

mod abi;
mod alloy_provider;
mod provider;
mod scanner;

pub use abi::ResurrectRegistryV1;
pub use alloy_provider::AlloyRegistryProvider;
pub use provider::{BlockInfo, BlockReference, ProviderError, RegistryConstants, RegistryProvider};
pub use scanner::{
    DEFAULT_INITIAL_CHUNK_SIZE, DEFAULT_MAX_LOGS_PER_SCAN, RegistryScanner, ScanCheckpoint,
    ScanError, ScanReport, ScannerConfig,
};
