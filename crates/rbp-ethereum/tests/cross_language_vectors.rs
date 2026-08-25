//! Cross-language conformance tests for the canonical registry event ABI.

use alloy::{primitives::B256, sol_types::SolEvent};
use rbp_ethereum::RBPRegistryV1;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventVector {
    event_signature: String,
    topic0: String,
    namespace: String,
    record_type: u32,
    record_type_topic: String,
    valid_until: String,
    peer_record: String,
    data: String,
}

fn vector() -> EventVector {
    serde_json::from_str(include_str!(
        "../../../test-vectors/registry-events/peer-announced-v1.json"
    ))
    .unwrap()
}

#[test]
fn alloy_decodes_shared_peer_announced_event_vector() {
    let vector = vector();
    assert_eq!(
        vector.event_signature,
        RBPRegistryV1::PeerAnnounced::SIGNATURE
    );
    let topics = [
        B256::from_str(&vector.topic0).unwrap(),
        B256::from_str(&vector.namespace).unwrap(),
        B256::from_str(&vector.record_type_topic).unwrap(),
    ];
    assert_eq!(topics[0], RBPRegistryV1::PeerAnnounced::SIGNATURE_HASH);
    let data = hex::decode(vector.data.trim_start_matches("0x")).unwrap();
    let decoded = RBPRegistryV1::PeerAnnounced::decode_raw_log_validate(topics, &data).unwrap();

    assert_eq!(
        decoded.namespace,
        B256::from_str(&vector.namespace).unwrap()
    );
    assert_eq!(decoded.recordType, vector.record_type);
    assert_eq!(decoded.validUntil.to_string(), vector.valid_until);
    assert_eq!(
        decoded.peerRecord.as_ref(),
        hex::decode(vector.peer_record.trim_start_matches("0x"))
            .unwrap()
            .as_slice()
    );
}
