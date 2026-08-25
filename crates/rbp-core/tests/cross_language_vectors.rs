//! Cross-language conformance tests for descriptor normalization.

use rbp_core::{Namespace, NetworkDescriptor};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DescriptorVector {
    application: String,
    major_version: u64,
    derived_namespace: String,
    descriptor: serde_json::Value,
    canonical_json: String,
}

#[test]
fn rust_normalizes_shared_descriptor_vector() {
    let vector: DescriptorVector = serde_json::from_str(include_str!(
        "../../../test-vectors/descriptors/rbp-v1.json"
    ))
    .unwrap();
    let descriptor = NetworkDescriptor::from_json(&vector.descriptor.to_string()).unwrap();

    assert_eq!(
        Namespace::derive(&vector.application, vector.major_version),
        descriptor.namespace
    );
    assert_eq!(descriptor.namespace.to_string(), vector.derived_namespace);
    assert_eq!(descriptor.canonical_json().unwrap(), vector.canonical_json);
}
